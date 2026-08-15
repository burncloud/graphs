from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from .agents import AgentContext, AgentRunner, PromptBuilder
from .config import Workload
from .repo import RepoWorkspace
from .state import Finding, GateResult, GraphState, Status
from .verifiers import (
    CommandVerifier,
    ContractVerifier,
    ScopeVerifier,
    SourceVerifier,
    TruthVerifier,
    VisualVerifier,
)


@dataclass
class RunContext:
    workload: Workload
    workspace: RepoWorkspace
    source_dir: Path
    target_dir: Path
    agent: AgentRunner
    state_file: Path


class UIGraph:
    """Deterministic evaluator-optimizer loop for page-by-page UI migration."""

    def __init__(self, ctx: RunContext, state: GraphState):
        self.ctx = ctx
        self.state = state

    def run(self) -> GraphState:
        self.state.event("graph", "started")
        self._checkpoint()

        for page_name in self.state.selected_pages:
            self._run_page(page_name)
            if self.state.page(page_name).status == Status.FAILED:
                self.state.event("graph", f"stopped at failed page {page_name}")
                self._checkpoint()
                return self.state

        self._run_final_gates()
        if all(gate.passed for gate in self.state.final_gates):
            self.state.commit_sha = self.ctx.workspace.commit_staged(
                self.ctx.target_dir,
                f"Migrate UI via {self.ctx.workload.name}",
            )
            self.state.event("git", f"committed {self.state.commit_sha}")

        self.state.event("graph", "finished")
        self._checkpoint()
        return self.state

    def _run_page(self, page_name: str) -> None:
        page = self.ctx.workload.page(page_name)
        page_run = self.state.page(page_name)
        source_gate = SourceVerifier().run(page=page, source_dir=self.ctx.source_dir)
        page_run.gates = [source_gate]

        if not source_gate.passed:
            page_run.status = Status.FAILED
            self._checkpoint()
            return

        agent_ctx = AgentContext(
            source_dir=self.ctx.source_dir,
            target_dir=self.ctx.target_dir,
            visual_contract=self.ctx.workload.visual_contract,
            truth_contract=self.ctx.workload.truth_contract,
        )

        findings: list[Finding] = []
        for attempt in range(1, self.ctx.workload.max_attempts + 1):
            page_run.attempt = attempt
            page_run.status = Status.RUNNING
            node = "implement" if attempt == 1 else "fix"
            self.state.event(node, f"{page.name} attempt {attempt}")

            prompt = (
                PromptBuilder.implementation(self.ctx.workload, page, agent_ctx)
                if attempt == 1
                else PromptBuilder.fix(self.ctx.workload, page, agent_ctx, findings)
            )
            agent_result = self.ctx.agent.run(prompt, agent_ctx, page)
            if not agent_result.ok:
                page_run.gates.append(
                    GateResult(
                        name="agent",
                        status=Status.FAILED,
                        findings=[
                            Finding(
                                "agent",
                                "Implementation agent failed",
                                detail=agent_result.stderr[-3000:],
                            )
                        ],
                        output=agent_result.stdout + "\n" + agent_result.stderr,
                    )
                )
                page_run.status = Status.FAILED
                self._checkpoint()
                return

            page_run.gates = [source_gate] + self._verification_gates(page)
            page_run.changed_files = self.ctx.workspace.working_files(self.ctx.target_dir)
            findings = [
                finding
                for gate in page_run.gates
                if not gate.passed
                for finding in gate.findings
            ]
            self._checkpoint()

            if not findings:
                page_run.status = Status.PASSED
                self.ctx.workspace.stage_working(self.ctx.target_dir)
                self.state.event(
                    "page",
                    f"{page.name} passed all gates and was staged",
                )
                self._checkpoint()
                return

        page_run.status = Status.FAILED
        self.state.event(
            "page",
            f"{page.name} exhausted {self.ctx.workload.max_attempts} attempts",
        )
        self._checkpoint()

    def _verification_gates(self, page) -> list[GateResult]:
        kwargs = dict(
            workload=self.ctx.workload,
            page=page,
            source_dir=self.ctx.source_dir,
            target_dir=self.ctx.target_dir,
            workspace=self.ctx.workspace,
        )
        gates = [
            ScopeVerifier().run(**kwargs),
            ContractVerifier().run(**kwargs),
            TruthVerifier().run(**kwargs),
            VisualVerifier().run(**kwargs),
        ]
        if self.ctx.workload.verify.page_commands:
            gates.append(
                CommandVerifier(
                    "build",
                    self.ctx.workload.verify.page_commands,
                ).run(**kwargs)
            )
        return gates

    def _run_final_gates(self) -> None:
        kwargs = dict(
            target_dir=self.ctx.target_dir,
            workspace=self.ctx.workspace,
        )
        gates: list[GateResult] = []
        if self.ctx.workload.verify.cross_page_commands:
            gates.append(
                CommandVerifier(
                    "cross-page",
                    self.ctx.workload.verify.cross_page_commands,
                ).run(**kwargs)
            )
        if self.ctx.workload.verify.final_commands:
            gates.append(
                CommandVerifier(
                    "final-build",
                    self.ctx.workload.verify.final_commands,
                ).run(**kwargs)
            )
        self.state.final_gates = gates

    def _checkpoint(self) -> None:
        self.state.save(self.ctx.state_file)
