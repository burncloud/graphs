# BurnCloud Graphs

Deterministic **Graph Engineering** harness for migrating UI/product behavior from a reference repository into BurnCloud without letting an implementation agent silently redesign the product or invent data.

The first built-in workload covers all 16 routed page components in `rustburn/burncloud-ui` (`Home` owns both `/home` and `/landing`):

```text
rustburn/burncloud-ui (React/Vite visual reference)
                ↓
        page contract + truth rules
                ↓
       implementation / fix agent
                ↓
 scope → product → truth → visual → build
                ↓
           failure loops back
                ↓
 burncloud/burncloud (Rust/Dioxus target)
```

## Why this exists

A green build is not the same as a completed UI migration. The graph makes page completion explicit and machine-checkable:

1. the exact source page must exist;
2. the implementation agent may edit only page-owned target paths;
3. page contract gates must pass;
4. fake/simulated claims are rejected by the Truth Gate;
5. visual-language constraints are checked;
6. Rust formatting/build checks must pass;
7. accepted pages are staged as graph checkpoints, so later pages are verified only against their own new delta;
8. cross-page wiring/product checks must pass before a final commit is created;
9. a failed page feeds structured gate findings back into a targeted fix pass, up to the configured attempt limit.

The graph stops instead of declaring success when a gate cannot prove completion.

## Quick start

Python 3.11+ is enough; the harness itself has no runtime dependencies.

```bash
python -m pip install -e .
burncloud-graphs plan workloads/burncloud-ui-to-burncloud.toml
```

Configure an implementation agent. For example, if your environment exposes a Codex CLI that accepts prompts on stdin:

```bash
export BURNCLOUD_GRAPHS_AGENT='codex exec --full-auto -C {target_dir} -'
```

The agent command is intentionally not hard-coded because the graph owns acceptance, while the implementation backend can be Codex, Claude, or another coding agent.

Migrate one page:

```bash
burncloud-graphs run workloads/burncloud-ui-to-burncloud.toml --page overview
```

Execute the complete ordered workload:

```bash
burncloud-graphs run workloads/burncloud-ui-to-burncloud.toml
```

Use already-cloned repos while iterating:

```bash
burncloud-graphs run workloads/burncloud-ui-to-burncloud.toml \
  --source-dir ../burncloud-ui \
  --target-dir ../burncloud \
  --page providers
```

The target worktree must be clean before a run. By default the graph creates `graphs/ui-migration-<run-id>` in the target repository. Every run checkpoints machine-readable state under `.graphs/work/runs/<run-id>/state.json`.

## Execution graph

```text
SOURCE AUDIT
    │
    ▼
IMPLEMENT ───────────────────────────────┐
    │                                    │
    ▼                                    │
SCOPE                                    │
    │                                    │
PRODUCT CONTRACT                         │
    │                                    │
TRUTH                                    │
    │                                    │
VISUAL                                   │
    │                                    │
BUILD                                    │
    │                                    │
  PASS? ── no ──> TARGETED FIX ──────────┘
    │
   yes
    ▼
STAGE PAGE CHECKPOINT
    │
    ▼
NEXT PAGE
    │
    ▼
CROSS-PAGE WIRING
    │
FINAL BUILD
    │
    ▼
FINAL COMMIT
```

Staging is deliberate. Previously accepted pages live in the Git index while the current page remains unstaged. Product/Truth/Visual/Scope gates therefore inspect only the current page's new delta rather than accidentally passing because an earlier page contained the same keyword.

## Contracts

`contracts/visual-language.md` keeps the refined `rustburn/burncloud-ui` aesthetic while explicitly forbidding mechanical React→Dioxus transliteration.

`contracts/data-truth.md` is more important: configured ≠ available ≠ verified; UNKNOWN ≠ zero; a signed receipt ≠ runtime attestation. The harness rejects unconditional claims such as `All routes verified`, `100% traceable`, `verified silicon`, and `hardware attested` when they remain in a migrated page without an evidence-backed implementation.

`workloads/burncloud-ui-to-burncloud.toml` contains:

- source and target repositories;
- all 16 page mappings;
- the question each page must answer;
- target search hints;
- required outcomes;
- forbidden mock/simulated content;
- allowed file scope;
- per-page Rust checks;
- final cross-page BurnCloud checks.

The built-in final checks call BurnCloud's existing `check-functional-wiring.sh` and `check-product-ux.sh` plus a web-feature `cargo check` for `burncloud-client`.

## Agent contract

The implementation agent is deliberately pluggable. The configured command must:

- accept the generated prompt on stdin;
- have read access to the source repo;
- have write access to the target repo;
- leave changes unstaged and uncommitted;
- exit non-zero when it cannot complete the requested edit.

The implementation prompt explicitly says the agent is **not the product designer**. The graph, not the agent, owns the definition of done.

## Tests

```bash
PYTHONPATH=src python -m unittest discover -s tests -v
python -m compileall -q src tests
PYTHONPATH=src python -m burncloud_graphs.cli plan workloads/burncloud-ui-to-burncloud.toml
```

The repository CI runs the same self-checks on every pull request.
