from __future__ import annotations

from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from enum import Enum
from pathlib import Path
import json
import uuid


class Status(str, Enum):
    PENDING = "pending"
    RUNNING = "running"
    PASSED = "passed"
    FAILED = "failed"
    SKIPPED = "skipped"


@dataclass
class Finding:
    gate: str
    message: str
    severity: str = "error"
    path: str | None = None
    detail: str | None = None


@dataclass
class GateResult:
    name: str
    status: Status
    findings: list[Finding] = field(default_factory=list)
    output: str = ""

    @property
    def passed(self) -> bool:
        return self.status == Status.PASSED


@dataclass
class PageRun:
    name: str
    status: Status = Status.PENDING
    attempt: int = 0
    gates: list[GateResult] = field(default_factory=list)
    changed_files: list[str] = field(default_factory=list)

    @property
    def findings(self) -> list[Finding]:
        return [finding for gate in self.gates for finding in gate.findings]


@dataclass
class GraphState:
    workload: str
    source_dir: str
    target_dir: str
    selected_pages: list[str]
    run_id: str = field(default_factory=lambda: uuid.uuid4().hex[:12])
    started_at: str = field(default_factory=lambda: datetime.now(timezone.utc).isoformat())
    pages: dict[str, PageRun] = field(default_factory=dict)
    final_gates: list[GateResult] = field(default_factory=list)
    events: list[dict[str, str]] = field(default_factory=list)
    branch: str | None = None
    commit_sha: str | None = None

    def page(self, name: str) -> PageRun:
        return self.pages.setdefault(name, PageRun(name=name))

    def event(self, node: str, message: str) -> None:
        self.events.append({
            "at": datetime.now(timezone.utc).isoformat(),
            "node": node,
            "message": message,
        })

    def save(self, path: Path) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            json.dumps(asdict(self), indent=2, ensure_ascii=False),
            encoding="utf-8",
        )
