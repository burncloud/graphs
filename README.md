# BurnCloud Graphs

Rust-native Graph Engineering runtime for migrating `rustburn/burncloud-ui` into `burncloud/burncloud` without requiring Python or WSL.

The runtime is being migrated from the original Python prototype to a native Rust CLI. Contracts and workloads stay external and declarative; orchestration, state, agent execution, Git checkpoints, verification gates, retries, and run evidence are implemented in Rust.

```text
reference repo
    ↓
page contract
    ↓
AgentRunner
    ↓
scope / product / truth / visual / build
    ↓
FAIL → targeted fix → retry
    ↓
PASS → stage → next page
    ↓
cross-page / final build
    ↓
commit + state.json
```

## Windows-native target

Required tools:

- Rust stable
- Git
- a coding agent executable such as Codex CLI, Claude Code, or another command that accepts prompts on stdin

No Python runtime is required. No WSL dependency is required by BurnCloud Graphs itself.

## Quick start

```powershell
cargo build --release
.\target\release\burncloud-graphs.exe plan workloads\burncloud-ui-to-burncloud.toml
```

Run one page with already-cloned repositories:

```powershell
$env:BURNCLOUD_GRAPHS_AGENT='codex exec --sandbox workspace-write -C {target_dir} -'

.\target\release\burncloud-graphs.exe run workloads\burncloud-ui-to-burncloud.toml `
  --source-dir ..\burncloud-ui `
  --target-dir ..\burncloud `
  --workspace ..\graph-work `
  --page overview
```

Every run writes machine-readable evidence under `<workspace>/runs/<run-id>/state.json`.

## Runtime model

The executable owns the definition of done. Coding agents only modify the target worktree. Each page passes through built-in source/scope/product/truth/visual gates and configured platform-neutral command gates. Failed findings are fed back to the agent for a bounded retry loop. A page is staged only after every page gate passes, and a final commit is created only after cross-page and final checks pass.
