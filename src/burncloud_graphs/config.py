from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import tomllib


@dataclass(frozen=True)
class RepoSpec:
    url: str
    ref: str = "main"


@dataclass(frozen=True)
class AgentSpec:
    command: tuple[str, ...] = ()
    timeout_seconds: int = 1800


@dataclass(frozen=True)
class PageSpec:
    name: str
    source: str
    question: str
    target_search: tuple[str, ...] = ()
    required: tuple[str, ...] = ()
    forbidden: tuple[str, ...] = ()
    visual_required: tuple[str, ...] = ()
    visual_forbidden: tuple[str, ...] = ()
    allowed_paths: tuple[str, ...] = ()
    notes: str = ""


@dataclass(frozen=True)
class VerifySpec:
    page_commands: tuple[str, ...] = ()
    final_commands: tuple[str, ...] = ()
    cross_page_commands: tuple[str, ...] = ()
    truth_forbidden_claims: tuple[str, ...] = ()


@dataclass(frozen=True)
class Workload:
    name: str
    source: RepoSpec
    target: RepoSpec
    agent: AgentSpec
    verify: VerifySpec
    pages: tuple[PageSpec, ...]
    visual_contract: str
    truth_contract: str
    max_attempts: int = 3

    def page(self, name: str) -> PageSpec:
        for page in self.pages:
            if page.name == name:
                return page
        raise KeyError(f"Unknown page: {name}")


def _tuple(value) -> tuple[str, ...]:
    return tuple(value or ())


def load_workload(path: str | Path) -> Workload:
    config_path = Path(path).resolve()
    data = tomllib.loads(config_path.read_text(encoding="utf-8"))
    repo_root = config_path.parent.parent

    pages = tuple(
        PageSpec(
            name=p["name"],
            source=p["source"],
            question=p["question"],
            target_search=_tuple(p.get("target_search")),
            required=_tuple(p.get("required")),
            forbidden=_tuple(p.get("forbidden")),
            visual_required=_tuple(p.get("visual_required")),
            visual_forbidden=_tuple(p.get("visual_forbidden")),
            allowed_paths=_tuple(p.get("allowed_paths")),
            notes=p.get("notes", ""),
        )
        for p in data.get("pages", [])
    )
    verify_data = data.get("verify", {})
    agent_data = data.get("agent", {})

    return Workload(
        name=data["workload"]["name"],
        source=RepoSpec(**data["source"]),
        target=RepoSpec(**data["target"]),
        agent=AgentSpec(
            command=tuple(agent_data.get("command", [])),
            timeout_seconds=int(agent_data.get("timeout_seconds", 1800)),
        ),
        verify=VerifySpec(
            page_commands=_tuple(verify_data.get("page_commands")),
            final_commands=_tuple(verify_data.get("final_commands")),
            cross_page_commands=_tuple(verify_data.get("cross_page_commands")),
            truth_forbidden_claims=_tuple(verify_data.get("truth_forbidden_claims")),
        ),
        pages=pages,
        visual_contract=str((repo_root / data["contracts"]["visual"]).resolve()),
        truth_contract=str((repo_root / data["contracts"]["truth"]).resolve()),
        max_attempts=int(data["workload"].get("max_attempts", 3)),
    )
