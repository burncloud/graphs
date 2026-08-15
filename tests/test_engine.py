import tempfile
import unittest
from pathlib import Path

from burncloud_graphs.engine import (
    AgentSpec,
    PageSpec,
    RepoSpec,
    RepoWorkspace,
    TruthVerifier,
    VerifySpec,
    VisualVerifier,
    Workload,
    load_workload,
)


ROOT = Path(__file__).resolve().parents[1]


class EngineTests(unittest.TestCase):
    def _init_repo(self, root: Path) -> RepoWorkspace:
        ws = RepoWorkspace(root.parent / "work")
        ws.run_argv(["git", "init"], root)
        ws.run_argv(["git", "config", "user.email", "test@example.com"], root)
        ws.run_argv(["git", "config", "user.name", "Test"], root)
        (root / "page.rs").write_text("fn page() {}\n", encoding="utf-8")
        ws.run_argv(["git", "add", "page.rs"], root)
        ws.run_argv(["git", "commit", "-m", "seed"], root)
        return ws

    def test_workload_loads_all_reference_pages(self):
        workload = load_workload(ROOT / "workloads/burncloud-ui-to-burncloud.toml")
        self.assertEqual(workload.pages[0].name, "public-home")
        self.assertGreaterEqual(len(workload.pages), 16)
        self.assertEqual(workload.pages[-1].name, "settings")

    def test_truth_gate_rejects_unconditional_claim(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            ws = self._init_repo(repo)
            (repo / "page.rs").write_text(
                'fn page() { let _ = "All routes verified"; }\n',
                encoding="utf-8",
            )
            workload = Workload(
                "test",
                RepoSpec("x"),
                RepoSpec("y"),
                AgentSpec(),
                VerifySpec(),
                (PageSpec("overview", "x", "q"),),
                "",
                "",
            )
            result = TruthVerifier().run(
                workload=workload,
                target_dir=repo,
                workspace=ws,
            )
            self.assertFalse(result.passed)

    def test_visual_gate_rejects_mechanical_react_copy(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            ws = self._init_repo(repo)
            (repo / "page.rs").write_text(
                'fn page() { let _ = "className=\\\"rounded-xl\\\""; }\n',
                encoding="utf-8",
            )
            result = VisualVerifier().run(
                page=PageSpec("overview", "source.tsx", "q"),
                target_dir=repo,
                workspace=ws,
            )
            self.assertFalse(result.passed)

    def test_staged_page_is_not_visible_to_next_page_gate(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            ws = self._init_repo(repo)
            (repo / "page.rs").write_text('fn page() { let _ = "Overview"; }\n')
            ws.stage_working(repo)
            self.assertEqual(ws.working_files(repo), [])

            (repo / "next.rs").write_text('fn next() { let _ = "Providers"; }\n')
            self.assertEqual(ws.working_files(repo), ["next.rs"])
            self.assertNotIn("Overview", ws.working_text(repo))
            self.assertIn("Providers", ws.working_text(repo))


if __name__ == "__main__":
    unittest.main()
