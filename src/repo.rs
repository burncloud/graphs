use crate::config::RepoSpec;
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub command: String,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CommandResult {
    pub fn ok(&self) -> bool {
        self.status == 0
    }
}

#[derive(Debug, Clone)]
pub struct RepoWorkspace {
    pub root: PathBuf,
}

impl RepoWorkspace {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn ensure_repo(
        &self,
        name: &str,
        spec: &RepoSpec,
        existing: Option<&Path>,
    ) -> Result<PathBuf> {
        if let Some(existing) = existing {
            let path = existing
                .canonicalize()
                .with_context(|| format!("cannot resolve repo {}", existing.display()))?;
            if !path.join(".git").exists() {
                bail!("not a git repository: {}", path.display());
            }
            return Ok(path);
        }

        let path = self.root.join(name);
        if !path.join(".git").exists() {
            let args = vec![
                "clone".to_string(),
                "--filter=blob:none".to_string(),
                spec.url.clone(),
                path.display().to_string(),
            ];
            self.git_in(&self.root, &args)?;
        }
        self.git_in(
            &path,
            &[
                "fetch".into(),
                "origin".into(),
                spec.r#ref.clone(),
                "--depth=1".into(),
            ],
        )?;
        self.git_in(
            &path,
            &["checkout".into(), "--detach".into(), "FETCH_HEAD".into()],
        )?;
        Ok(path)
    }

    pub fn prepare_target_branch(&self, target: &Path, run_id: &str) -> Result<String> {
        let branch = format!("graphs/ui-migration-{run_id}");
        self.git_in(target, &["checkout".into(), "-B".into(), branch.clone()])?;
        Ok(branch)
    }

    pub fn changed_files(&self, target: &Path) -> Result<Vec<String>> {
        let result = self.git_in(target, &["status".into(), "--porcelain".into()])?;
        Ok(result
            .stdout
            .lines()
            .filter_map(|line| line.get(3..).map(str::trim))
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect())
    }

    pub fn working_files(&self, target: &Path) -> Result<Vec<String>> {
        let modified = self.git_in(target, &["diff".into(), "--name-only".into()])?;
        let untracked = self.git_in(
            target,
            &[
                "ls-files".into(),
                "--others".into(),
                "--exclude-standard".into(),
            ],
        )?;
        let mut files = BTreeSet::new();
        for line in modified.stdout.lines().chain(untracked.stdout.lines()) {
            let line = line.trim();
            if !line.is_empty() {
                files.insert(line.to_string());
            }
        }
        Ok(files.into_iter().collect())
    }

    pub fn working_text(&self, target: &Path) -> Result<String> {
        let mut text = String::new();
        for rel in self.working_files(target)? {
            let path = target.join(&rel);
            if let Ok(content) = fs::read_to_string(path) {
                text.push_str(&content);
                text.push('\n');
            }
        }
        Ok(text)
    }

    pub fn stage_working(&self, target: &Path) -> Result<()> {
        self.git_in(target, &["add".into(), "-A".into()])?;
        Ok(())
    }

    pub fn commit_staged(&self, target: &Path, message: &str) -> Result<String> {
        let args = vec![
            "-c".into(),
            "user.name=BurnCloud Graphs".into(),
            "-c".into(),
            "user.email=graphs@burncloud.local".into(),
            "commit".into(),
            "-m".into(),
            message.into(),
        ];
        let result = self.git_in(target, &args)?;
        if !result.ok() {
            bail!("graph commit failed: {}", result.stderr);
        }
        Ok(self
            .git_in(target, &["rev-parse".into(), "HEAD".into()])?
            .stdout
            .trim()
            .to_string())
    }

    pub fn git_in(&self, cwd: &Path, args: &[String]) -> Result<CommandResult> {
        self.run_argv(cwd, "git", args, None)
    }

    pub fn run_command_line(
        &self,
        cwd: &Path,
        command_line: &str,
        stdin: Option<&str>,
    ) -> Result<CommandResult> {
        let argv = shell_words::split(command_line)
            .with_context(|| format!("cannot parse command: {command_line}"))?;
        let (program, args) = argv.split_first().context("empty command")?;
        self.run_argv(cwd, program, args, stdin)
    }

    pub fn run_argv(
        &self,
        cwd: &Path,
        program: &str,
        args: &[String],
        stdin: Option<&str>,
    ) -> Result<CommandResult> {
        let mut cmd = portable_command(program, args)?;
        cmd.current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if stdin.is_some() {
            cmd.stdin(Stdio::piped());
        }
        let rendered = std::iter::once(program.to_string())
            .chain(args.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        let mut child = cmd
            .spawn()
            .with_context(|| format!("cannot start command: {rendered}"))?;
        if let Some(input) = stdin {
            if let Some(mut child_stdin) = child.stdin.take() {
                child_stdin.write_all(input.as_bytes())?;
            }
        }
        let output: Output = child.wait_with_output()?;
        Ok(CommandResult {
            command: rendered,
            status: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

fn portable_command(program: &str, args: &[String]) -> Result<Command> {
    #[cfg(windows)]
    {
        let resolved = resolve_windows_program(program).unwrap_or_else(|| PathBuf::from(program));
        let ext = resolved
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "cmd" || ext == "bat" {
            let mut line = quote_windows(&resolved.display().to_string());
            for arg in args {
                line.push(' ');
                line.push_str(&quote_windows(arg));
            }
            let mut cmd = Command::new("cmd.exe");
            cmd.args(["/D", "/S", "/C", &line]);
            return Ok(cmd);
        }
        let mut cmd = Command::new(resolved);
        cmd.args(args);
        return Ok(cmd);
    }
    #[cfg(not(windows))]
    {
        let mut cmd = Command::new(program);
        cmd.args(args);
        Ok(cmd)
    }
}

#[cfg(windows)]
fn resolve_windows_program(program: &str) -> Option<PathBuf> {
    if Path::new(program).extension().is_some() {
        return Some(PathBuf::from(program));
    }
    let output = Command::new("where.exe").arg(program).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|line| PathBuf::from(line.trim()))
}

#[cfg(windows)]
fn quote_windows(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".into();
    }
    if !value.chars().any(|c| c.is_whitespace() || c == '"') {
        return value.into();
    }
    format!("\"{}\"", value.replace('"', "\\\""))
}
