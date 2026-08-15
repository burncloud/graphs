from __future__ import annotations

import argparse
from pathlib import Path
import sys

from .agents import AgentRunner
from .config import load_workload
from .graph import RunContext, UIGraph
from .repo import RepoWorkspace
from .state import GraphState, Status


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="burncloud-graphs")
    sub = parser.add_subparsers(dest="command", required=True)

    for name in ("run", "plan"):
        command = sub.add_parser(name)
        command.add_argument("workload")
        command.add_argument(
            "--page",
            action="append",
            dest="pages",
            help="Run only one or more named page contracts",
        )
        command.add_argument("--workspace", default=".graphs/work")
        command.add_argument("--source-dir")
        command.add_argument("--target-dir")
        command.add_argument(
            "--no-branch",
            action="store_true",
            help="Do not create a graphs/* target branch",
        )

    return parser


def _select_pages(workload, requested):
    names = [page.name for page in workload.pages]
    if not requested:
        return names
    unknown = sorted(set(requested) - set(names))
    if unknown:
        raise SystemExit(f"Unknown pages: {', '.join(unknown)}")
    return requested


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    workload = load_workload(args.workload)
    pages = _select_pages(workload, args.pages)

    if args.command == "plan":
        print(f"Workload: {workload.name}")
        for index, name in enumerate(pages, 1):
            page = workload.page(name)
            print(f"{index:02d}. {page.name}: {page.question} <- {page.source}")
        return 0

    root = Path(args.workspace).resolve()
    repo_workspace = RepoWorkspace(root / "repos")
    source = repo_workspace.ensure_repo("source", workload.source, args.source_dir)
    target = repo_workspace.ensure_repo("target", workload.target, args.target_dir)

    dirty = repo_workspace.changed_files(target)
    if dirty:
        print(
            "Target repository must be clean before a graph run: " + ", ".join(dirty),
            file=sys.stderr,
        )
        return 65

    state = GraphState(
        workload=workload.name,
        source_dir=str(source),
        target_dir=str(target),
        selected_pages=pages,
    )

    if not args.no_branch:
        state.branch = repo_workspace.prepare_target_branch(target, state.run_id)
        state.event("git", f"prepared target branch {state.branch}")

    state_file = root / "runs" / state.run_id / "state.json"
    agent = AgentRunner(workload.agent, repo_workspace)
    if not agent.configured:
        print(
            "No implementation agent configured. Set BURNCLOUD_GRAPHS_AGENT or [agent].command.",
            file=sys.stderr,
        )
        return 64

    graph = UIGraph(
        RunContext(workload, repo_workspace, source, target, agent, state_file),
        state,
    )
    result = graph.run()

    failed_pages = [
        page.name for page in result.pages.values() if page.status == Status.FAILED
    ]
    failed_final = [gate.name for gate in result.final_gates if not gate.passed]

    print(f"Run state: {state_file}")
    if result.branch:
        print(f"Target branch: {result.branch}")
    if result.commit_sha:
        print(f"Commit: {result.commit_sha}")

    if failed_pages or failed_final:
        print("FAILED")
        if failed_pages:
            print("Pages:", ", ".join(failed_pages))
        if failed_final:
            print("Final gates:", ", ".join(failed_final))
        return 1

    print("PASSED")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
