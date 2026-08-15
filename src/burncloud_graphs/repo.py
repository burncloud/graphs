from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import shlex
import subprocess

from .config import RepoSpec


@dataclass
class CommandResult:
    command: str
    returncode: int
    stdout: str
    stderr: str

    @property
    def ok(self) -> bool:
        return self.returncode == 0


class RepoWorkspace:
    def __init__(self, root: Path):
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)

    def ensure_repo(self, name: str, spec: RepoSpec, existing: str | None = None) -> Path:
        if existing:
            path = Path(existing).resolve()
            if not (path / ".git").exists():
                raise RuntimeError(f"Not a git repository: {path}")
            return path

        path = self.root / name
        if not (path / ".git").exists():
            self._run(["git", "clone", "--filter=blob:none", spec.url, str(path)], self.root)
        self._run(["git", "fetch", "origin", spec.ref, "--depth=1"], path)
        self._run(["git", "checkout", "--detach", "FETCH_HEAD"], path)
        return path

    def prepare_target_branch(self, target: Path, run_id: str) -> str:
        branch = f"graphs/ui-migration-{run_id}"
        self._run(["git", "checkout", "-B", branch], target)
        return branch

    def run_shell(self, command: str, cwd: Path, timeout: int = 1800) -> CommandResult:
        proc = subprocess.run(
            command,
            cwd=cwd,
            shell=True,
            text=True,
            capture_output=True,
            timeout=timeout,
        )
        return CommandResult(command, proc.returncode, proc.stdout, proc.stderr)

    def run_argv(
        self,
        argv: list[str],
        cwd: Path,
        stdin: str = "",
        timeout: int = 1800,
    ) -> CommandResult:
        rendered = " ".join(shlex.quote(x) for x in argv)
        proc = subprocess.run(
            argv,
            cwd=cwd,
            input=stdin,
            text=True,
            capture_output=True,
            timeout=timeout,
        )
        return CommandResult(rendered, proc.returncode, proc.stdout, proc.stderr)

    def changed_files(self, target: Path) -> list[str]:
        result = self.run_argv(["git", "status", "--porcelain"], target)
        if not result.ok:
            return []
        files: list[str] = []
        for line in result.stdout.splitlines():
            if len(line) >= 4:
                files.append(line[3:].strip())
        return sorted(set(files))

    def working_files(self, target: Path) -> list[str]:
        modified = self.run_argv(["git", "diff", "--name-only"], target).stdout.splitlines()
        untracked = self.run_argv(
            ["git", "ls-files", "--others", "--exclude-standard"], target
        ).stdout.splitlines()
        return sorted(set(x.strip() for x in modified + untracked if x.strip()))

    def working_text(self, target: Path) -> str:
        chunks: list[str] = []
        for rel in self.working_files(target):
            path = target / rel
            if not path.is_file():
                continue
            try:
                chunks.append(path.read_text(encoding="utf-8"))
            except UnicodeDecodeError:
                continue
        return "\n".join(chunks)

    def added_diff(self, target: Path) -> str:
        diff = self.run_argv(["git", "diff", "--unified=0"], target).stdout
        added = [
            line[1:]
            for line in diff.splitlines()
            if line.startswith("+") and not line.startswith("+++")
        ]
        tracked = set(
            self.run_argv(["git", "diff", "--name-only"], target).stdout.splitlines()
        )
        for rel in self.working_files(target):
            if rel in tracked:
                continue
            path = target / rel
            if not path.is_file():
                continue
            try:
                added.append(path.read_text(encoding="utf-8"))
            except UnicodeDecodeError:
                pass
        return "\n".join(added)

    def stage_working(self, target: Path) -> None:
        result = self.run_argv(["git", "add", "-A"], target)
        if not result.ok:
            raise RuntimeError(result.stderr)

    def commit_staged(self, target: Path, message: str) -> str:
        result = self.run_argv(["git", "commit", "-m", message], target)
        if not result.ok:
            raise RuntimeError(f"Could not commit graph result: {result.stderr}")
        return self.run_argv(["git", "rev-parse", "HEAD"], target).stdout.strip()

    def _run(self, argv: list[str], cwd: Path) -> None:
        result = self.run_argv(argv, cwd)
        if not result.ok:
            raise RuntimeError(f"Command failed: {result.command}\n{result.stderr}")
