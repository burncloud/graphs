use anyhow::{Context, Result, bail};
use std::{
    env,
    io::{self, Read},
    process::Command,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut prompt = String::new();
    io::stdin().read_to_string(&mut prompt)?;
    if !prompt.contains("PAGE: overview") {
        bail!("overview-patch-agent only supports the overview runtime proof");
    }
    let patch = env::var("BURNCLOUD_GRAPHS_PATCH_FILE")
        .context("BURNCLOUD_GRAPHS_PATCH_FILE is required")?;
    run_cmd("git", &["apply", "--check", &patch])?;
    run_cmd("git", &["apply", &patch])?;
    run_cmd("cargo", &["fmt", "-p", "burncloud-client"])?;
    Ok(())
}

fn run_cmd(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("cannot start {program}"))?;
    if !status.success() {
        bail!("{program} {} failed with {status}", args.join(" "));
    }
    Ok(())
}
