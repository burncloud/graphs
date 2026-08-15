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
    Workload,
    load_workload,
)


ROOT = Path(__file__).resolve().parents[1]


class EngineTests(unittest.TestCase):
    def test_workload_loads_all_reference_pages(self):
        workload = load_workload(ROOT / "workloads/burncloud-ui-to-burncloud.toml")
        self.assertEqual(workload.pages[0].name, "public-home")
        self.assertGreaterEqual(len(workload.pages), 16)
        self.assertEqual(workload.pages[-1].name, "settings")

    def test_truth_gate_rejects_unconditional_claim(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            ws = RepoWorkspace(repo.parent / "work")
            ws.run_argv(["git", "init"], repo)
            ws.run_argv(["git", "config", "user.email", "test@example.com"], repo)
            ws.run_argv(["git", "config", "user.name", "Test"], repo)
            (repo / "page.rs").write_text("fn page() {}\n", encoding="utf-8")
            ws.run_argv(["git", "add", "page.rs"], repo)
            ws.run_argv(["git", "commit", "-m", "seed"], repo)
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


if __name__ == "__main__":
    unittest.main()
