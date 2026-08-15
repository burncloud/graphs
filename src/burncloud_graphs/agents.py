from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import os
import shlex

from .config import AgentSpec, PageSpec, Workload
from .repo import CommandResult, RepoWorkspace
from .state import Finding


@dataclass
class AgentContext:
    source_dir: Path
    target_dir: Path
    visual_contract: str
    truth_contract: str


class PromptBuilder:
    @staticmethod
    def implementation(workload: Workload, page: PageSpec, ctx: AgentContext) -> str:
        visual = Path(ctx.visual_contract).read_text(encoding="utf-8")
        truth = Path(ctx.truth_contract).read_text(encoding="utf-8")
        target_hints = "\n".join(f"- {item}" for item in page.target_search) or "- inspect target ownership"
        required = "\n".join(f"- {item}" for item in page.required) or "- preserve page responsibility"
        forbidden = "\n".join(f"- {item}" for item in page.forbidden) or "- no invented product claims"

        return f"""Role: BurnCloud UI Graph Engineer

You are implementing a frozen page migration. You are NOT the product designer.

WORKLOAD: {workload.name}
PAGE: {page.name}
SOURCE REPOSITORY: {ctx.source_dir}
SOURCE PAGE: {ctx.source_dir / page.source}
TARGET REPOSITORY: {ctx.target_dir}

PAGE QUESTION
{page.question}

TARGET SEARCH HINTS
{target_hints}

REQUIRED OUTCOMES
{required}

FORBIDDEN OUTCOMES
{forbidden}

PAGE NOTES
{page.notes or 'None'}

VISUAL CONTRACT
{visual}

DATA TRUTH CONTRACT
{truth}

EXECUTION RULES
1. Inspect the real source page and real target implementation before editing.
2. Preserve the visual language of rustburn/burncloud-ui where it does not conflict with truth or target architecture.
3. Translate React/Tailwind interaction semantics into idiomatic Rust/Dioxus; do not mechanically transliterate JSX.
4. Reuse existing BurnCloud components and backend APIs before inventing new ones.
5. Do not fabricate metrics, health, verification, billing, security, or runtime state.
6. UNKNOWN, loading, empty, error, configured, available and verified are distinct states.
7. Keep this page inside its ownership boundary. Link to other domains instead of duplicating their controls.
8. Only touch files necessary for this page. Do not rewrite unrelated pages.
9. Run relevant formatting/checks if possible before returning.
10. Finish the implementation in the target working tree. Do not merely describe code.
11. Do not stage or commit changes; the graph owns checkpoints and commits.
"""

    @staticmethod
    def fix(
        workload: Workload,
        page: PageSpec,
        ctx: AgentContext,
        findings: list[Finding],
    ) -> str:
        base = PromptBuilder.implementation(workload, page, ctx)
        issues = "\n".join(
            f"- [{finding.gate}] {finding.message}"
            + (f" ({finding.path})" if finding.path else "")
            for finding in findings
        )
        return base + f"""

THIS IS A TARGETED FIX PASS.
The previous implementation failed these gates:
{issues}

Fix every listed failure without broadening scope. Re-run the relevant checks and leave the working tree in the corrected state.
"""


class AgentRunner:
    def __init__(
        self,
        spec: AgentSpec,
        workspace: RepoWorkspace,
        override: tuple[str, ...] = (),
    ):
        command = override or spec.command
        env_override = os.getenv("BURNCLOUD_GRAPHS_AGENT")
        if env_override:
            command = tuple(shlex.split(env_override))
        self.command = command
        self.timeout = spec.timeout_seconds
        self.workspace = workspace

    @property
    def configured(self) -> bool:
        return bool(self.command)

    def run(self, prompt: str, ctx: AgentContext, page: PageSpec) -> CommandResult:
        if not self.command:
            return CommandResult(
                "<agent-not-configured>",
                64,
                "",
                "No agent command configured. Set [agent].command or BURNCLOUD_GRAPHS_AGENT.",
            )

        replacements = {
            "{source_dir}": str(ctx.source_dir),
            "{target_dir}": str(ctx.target_dir),
            "{page}": page.name,
        }
        argv: list[str] = []
        for raw_arg in self.command:
            arg = raw_arg
            for key, value in replacements.items():
                arg = arg.replace(key, value)
            argv.append(arg)

        return self.workspace.run_argv(
            argv,
            ctx.target_dir,
            stdin=prompt,
            timeout=self.timeout,
        )
