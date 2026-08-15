use burncloud_graphs::{
    config::Workload,
    repo::RepoWorkspace,
    verifier::{truth_gate, visual_gate},
};
use std::{fs, path::PathBuf};
use tempfile::TempDir;

fn workload() -> Workload {
    Workload::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("workloads")
            .join("burncloud-ui-to-burncloud.toml"),
    )
    .expect("load built-in workload")
}

fn init_repo(temp: &TempDir) -> (RepoWorkspace, PathBuf) {
    let target = temp.path().join("target");
    fs::create_dir_all(&target).expect("create target");
    let workspace = RepoWorkspace::new(temp.path().join("workspace")).expect("create workspace");

    workspace
        .run_argv(&target, "git", &["init".into()], None)
        .expect("git init");
    fs::write(target.join("page.rs"), "fn page() {}\n").expect("write seed");
    workspace
        .run_argv(&target, "git", &["add".into(), "page.rs".into()], None)
        .expect("git add");
    workspace
        .run_argv(
            &target,
            "git",
            &[
                "-c".into(),
                "user.name=Graph Test".into(),
                "-c".into(),
                "user.email=graph-test@example.invalid".into(),
                "commit".into(),
                "-m".into(),
                "seed".into(),
            ],
            None,
        )
        .expect("git commit");
    (workspace, target)
}

#[test]
fn workload_loads_all_reference_pages() {
    let workload = workload();
    assert_eq!(workload.pages.len(), 16);
    assert_eq!(workload.pages.first().unwrap().name, "public-home");
    assert_eq!(workload.pages.last().unwrap().name, "settings");
    assert_eq!(
        workload.page("overview").unwrap().source,
        "src/pages/Overview.tsx"
    );
}

#[test]
fn truth_gate_rejects_unconditional_verification_claim() {
    let temp = TempDir::new().unwrap();
    let (workspace, target) = init_repo(&temp);
    fs::write(
        target.join("page.rs"),
        "fn page() { let _claim = \"All routes verified\"; }\n",
    )
    .unwrap();

    let result = truth_gate(&workload(), &target, &workspace).unwrap();
    assert!(!result.passed());
    assert!(
        result
            .findings
            .iter()
            .any(|finding| finding.message.contains("All routes verified"))
    );
}

#[test]
fn visual_gate_rejects_mechanical_react_copy() {
    let temp = TempDir::new().unwrap();
    let (workspace, target) = init_repo(&temp);
    fs::write(
        target.join("page.rs"),
        "fn page() { let _copied = \"className=\\\"rounded-xl\\\"\"; }\n",
    )
    .unwrap();

    let workload = workload();
    let page = workload.page("overview").unwrap();
    let result = visual_gate(page, &target, &workspace).unwrap();
    assert!(!result.passed());
}

#[test]
fn staged_page_is_hidden_from_next_page_delta() {
    let temp = TempDir::new().unwrap();
    let (workspace, target) = init_repo(&temp);
    fs::write(
        target.join("page.rs"),
        "fn page() { let _page = \"Overview\"; }\n",
    )
    .unwrap();
    workspace.stage_working(&target).unwrap();
    assert!(workspace.working_files(&target).unwrap().is_empty());

    fs::write(
        target.join("next.rs"),
        "fn next() { let _page = \"Providers\"; }\n",
    )
    .unwrap();
    assert_eq!(workspace.working_files(&target).unwrap(), vec!["next.rs"]);
    let text = workspace.working_text(&target).unwrap();
    assert!(text.contains("Providers"));
    assert!(!text.contains("Overview"));
}

#[cfg(windows)]
#[test]
fn windows_cmd_agent_runs_without_wsl() {
    let temp = TempDir::new().unwrap();
    let workspace = RepoWorkspace::new(temp.path().join("workspace")).unwrap();
    let agent = temp.path().join("agent.cmd");
    fs::write(
        &agent,
        "@echo off\r\nset /p graph_prompt=\r\necho AGENT_OK:%graph_prompt%\r\n",
    )
    .unwrap();

    let result = workspace
        .run_argv(
            temp.path(),
            agent.to_str().unwrap(),
            &[],
            Some("hello-from-graph\n"),
        )
        .unwrap();
    assert!(result.ok(), "{}", result.stderr);
    assert!(result.stdout.contains("AGENT_OK:hello-from-graph"));
}
