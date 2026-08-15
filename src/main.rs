mod agent;
mod config;
mod graph;
mod repo;
mod state;
mod verifier;

use agent::AgentRunner;
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use config::Workload;
use graph::UiGraph;
use repo::RepoWorkspace;
use state::{GraphState, Status};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "burncloud-graphs", version, about = "Rust-native BurnCloud Graph Engineering runtime")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Plan {
        workload: PathBuf,
        #[arg(long = "page")]
        pages: Vec<String>,
    },
    Run {
        workload: PathBuf,
        #[arg(long = "page")]
        pages: Vec<String>,
        #[arg(long, default_value = ".graphs/work")]
        workspace: PathBuf,
        #[arg(long)]
        source_dir: Option<PathBuf>,
        #[arg(long)]
        target_dir: Option<PathBuf>,
        #[arg(long)]
        no_branch: bool,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ERROR: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Plan { workload, pages } => plan(workload, pages),
        Commands::Run { workload, pages, workspace, source_dir, target_dir, no_branch } => {
            execute(workload, pages, workspace, source_dir, target_dir, no_branch)
        }
    }
}

fn select_pages(workload: &Workload, requested: Vec<String>) -> Result<Vec<String>> {
    if requested.is_empty() { return Ok(workload.page_names()); }
    for page in &requested { workload.page(page)?; }
    Ok(requested)
}

fn plan(path: PathBuf, requested: Vec<String>) -> Result<()> {
    let workload = Workload::load(path)?;
    let pages = select_pages(&workload, requested)?;
    println!("Workload: {}", workload.name);
    for (index, name) in pages.iter().enumerate() {
        let page = workload.page(name)?;
        println!("{:02}. {}: {} <- {}", index + 1, page.name, page.question, page.source);
    }
    Ok(())
}

fn execute(
    path: PathBuf,
    requested: Vec<String>,
    workspace_root: PathBuf,
    source_dir: Option<PathBuf>,
    target_dir: Option<PathBuf>,
    no_branch: bool,
) -> Result<()> {
    let workload = Workload::load(path)?;
    let selected_pages = select_pages(&workload, requested)?;
    let workspace_root = absolute(workspace_root)?;
    let repos = RepoWorkspace::new(workspace_root.join("repos"))?;
    let source = repos.ensure_repo("source", &workload.source, source_dir.as_deref())?;
    let target = repos.ensure_repo("target", &workload.target, target_dir.as_deref())?;

    let dirty = repos.changed_files(&target)?;
    if !dirty.is_empty() {
        bail!("target repository must be clean before a graph run: {}", dirty.join(", "));
    }

    let mut state = GraphState::new(
        workload.name.clone(),
        source.display().to_string(),
        target.display().to_string(),
        selected_pages,
    );
    if !no_branch {
        let branch = repos.prepare_target_branch(&target, &state.run_id)?;
        state.branch = Some(branch.clone());
        state.event("git", format!("prepared target branch {branch}"));
    }

    let state_file = workspace_root.join("runs").join(&state.run_id).join("state.json");
    state.save(&state_file)?;
    let agent = AgentRunner::new(&workload.agent)?;
    if !agent.configured() {
        bail!("no implementation agent configured; set BURNCLOUD_GRAPHS_AGENT or [agent].command");
    }

    let result = UiGraph {
        workload,
        workspace: repos,
        source_dir: source,
        target_dir: target,
        agent,
        state_file: state_file.clone(),
        state,
    }.run()?;

    println!("Run state: {}", state_file.display());
    if let Some(branch) = &result.branch { println!("Target branch: {branch}"); }
    if let Some(commit) = &result.commit_sha { println!("Commit: {commit}"); }

    let failed_pages = result.pages.values().filter(|run| run.status == Status::Failed).map(|run| run.name.clone()).collect::<Vec<_>>();
    let failed_final = result.final_gates.iter().filter(|gate| !gate.passed()).map(|gate| gate.name.clone()).collect::<Vec<_>>();
    if !failed_pages.is_empty() || !failed_final.is_empty() {
        println!("FAILED");
        if !failed_pages.is_empty() { println!("Pages: {}", failed_pages.join(", ")); }
        if !failed_final.is_empty() { println!("Final gates: {}", failed_final.join(", ")); }
        bail!("graph run failed");
    }
    println!("PASSED");
    Ok(())
}

fn absolute(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() { return Ok(path); }
    Ok(std::env::current_dir().context("cannot determine current directory")?.join(path))
}
