from __future__ import annotations

from pathlib import Path
import fnmatch
import re

from .config import PageSpec, Workload
from .repo import RepoWorkspace
from .state import Finding, GateResult, Status


def _pass(name: str, output: str = "") -> GateResult:
    return GateResult(name=name, status=Status.PASSED, output=output)


def _fail(name: str, findings: list[Finding], output: str = "") -> GateResult:
    return GateResult(name=name, status=Status.FAILED, findings=findings, output=output)


class SourceVerifier:
    name = "source"

    def run(self, page: PageSpec, source_dir: Path, **_) -> GateResult:
        path = source_dir / page.source
        if path.is_file():
            return _pass(self.name, str(path))
        return _fail(
            self.name,
            [Finding(self.name, "Source page does not exist", path=str(path))],
        )


class ScopeVerifier:
    name = "scope"

    def run(
        self,
        page: PageSpec,
        target_dir: Path,
        workspace: RepoWorkspace,
        **_,
    ) -> GateResult:
        changed = workspace.working_files(target_dir)
        if not changed:
            return _fail(self.name, [Finding(self.name, "Agent produced no target changes")])

        if not page.allowed_paths:
            return _pass(self.name, "\n".join(changed))

        violations = [
            path
            for path in changed
            if not any(fnmatch.fnmatch(path, pattern) for pattern in page.allowed_paths)
        ]
        if violations:
            return _fail(
                self.name,
                [
                    Finding(
                        self.name,
                        "Changed file is outside this page's allowed scope",
                        path=path,
                    )
                    for path in violations
                ],
            )
        return _pass(self.name, "\n".join(changed))


class ContractVerifier:
    name = "product"

    def run(
        self,
        page: PageSpec,
        target_dir: Path,
        workspace: RepoWorkspace,
        **_,
    ) -> GateResult:
        text = workspace.working_text(target_dir)
        findings: list[Finding] = []

        for pattern in page.required:
            if pattern.startswith("re:"):
                matched = re.search(pattern[3:], text, re.I | re.M) is not None
            else:
                matched = pattern.lower() in text.lower()
            if not matched:
                findings.append(
                    Finding(
                        self.name,
                        f"Required outcome not evidenced in page changes: {pattern}",
                    )
                )

        for pattern in page.forbidden:
            if pattern.startswith("re:"):
                matched = re.search(pattern[3:], text, re.I | re.M) is not None
            else:
                matched = pattern.lower() in text.lower()
            if matched:
                findings.append(
                    Finding(self.name, f"Forbidden page content remains or was introduced: {pattern}")
                )

        return _fail(self.name, findings) if findings else _pass(self.name)


class VisualVerifier:
    name = "visual"

    REACT_COPY_PATTERNS = (
        "className=",
        "motion/react",
        "lucide-react",
        "React.useState",
        "useState(",
        "@tailwindcss",
    )

    def run(
        self,
        page: PageSpec,
        target_dir: Path,
        workspace: RepoWorkspace,
        **_,
    ) -> GateResult:
        text = workspace.working_text(target_dir)
        findings: list[Finding] = []

        for token in self.REACT_COPY_PATTERNS:
            if token.lower() in text.lower():
                findings.append(
                    Finding(
                        self.name,
                        f"Mechanical React/Tailwind copy detected in Dioxus migration: {token}",
                    )
                )

        for token in page.visual_required:
            if token.lower() not in text.lower():
                findings.append(
                    Finding(self.name, f"Required visual-language token missing: {token}")
                )
        for token in page.visual_forbidden:
            if token.lower() in text.lower():
                findings.append(
                    Finding(self.name, f"Forbidden visual-language pattern introduced: {token}")
                )

        return _fail(self.name, findings) if findings else _pass(self.name)


class TruthVerifier:
    name = "truth"

    DEFAULT_CLAIMS = (
        "100% traceable",
        "100% authentic",
        "all routes verified",
        "fully traceable",
        "verified silicon",
        "zero proxy tampering",
        "hardware attested",
        "silicon attestation active",
    )

    def run(
        self,
        workload: Workload,
        target_dir: Path,
        workspace: RepoWorkspace,
        **_,
    ) -> GateResult:
        text = workspace.working_text(target_dir)
        findings: list[Finding] = []
        claims = self.DEFAULT_CLAIMS + workload.verify.truth_forbidden_claims

        for claim in dict.fromkeys(claims):
            if claim.lower() in text.lower():
                findings.append(
                    Finding(
                        self.name,
                        "Unconditional trust claim found without a machine-verifiable evidence gate: "
                        + claim,
                    )
                )

        suspicious_zero = re.compile(
            r"unwrap_or\(\s*(?:0(?:\.0)?|false|\"\"|String::new\(\))\s*\)"
        )
        if suspicious_zero.search(text):
            findings.append(
                Finding(
                    self.name,
                    "Potential UNKNOWN→default coercion found in page changes; preserve Unknown/Error unless the default is authoritative.",
                    severity="warning",
                )
            )

        errors = [finding for finding in findings if finding.severity == "error"]
        return _fail(self.name, findings) if errors else _pass(
            self.name,
            "\n".join(finding.message for finding in findings),
        )


class CommandVerifier:
    def __init__(self, name: str, commands: tuple[str, ...]):
        self.name = name
        self.commands = commands

    def run(
        self,
        target_dir: Path,
        workspace: RepoWorkspace,
        **_,
    ) -> GateResult:
        output: list[str] = []
        findings: list[Finding] = []

        for command in self.commands:
            result = workspace.run_shell(command, target_dir)
            output.append(f"$ {command}\n{result.stdout}\n{result.stderr}")
            if not result.ok:
                findings.append(
                    Finding(
                        self.name,
                        f"Command failed ({result.returncode}): {command}",
                        detail=result.stderr[-3000:],
                    )
                )
                break

        joined = "\n".join(output)
        return _fail(self.name, findings, joined) if findings else _pass(self.name, joined)
