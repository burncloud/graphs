use crate::{
    config::{AgentSpec, PageSpec, Workload},
    repo::{CommandResult, RepoWorkspace},
    state::Finding,
};
use anyhow::{Context, Result};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct AgentContext {
    pub source_dir: PathBuf,
    pub target_dir: PathBuf,
    pub visual_contract: PathBuf,
    pub truth_contract: PathBuf,
}

pub struct PromptBuilder;

impl PromptBuilder {
    pub fn implementation(
        workload: &Workload,
        page: &PageSpec,
        ctx: &AgentContext,
    ) -> Result<String> {
        let visual = fs::read_to_string(&ctx.visual_contract)
            .with_context(|| format!("cannot read {}", ctx.visual_contract.display()))?;
        let truth = fs::read_to_string(&ctx.truth_contract)
            .with_context(|| format!("cannot read {}", ctx.truth_contract.display()))?;
        Ok(format!(
            r#"Role: BurnCloud UI Graph Engineer

You are implementing a frozen page migration. You are NOT the product designer.

WORKLOAD: {workload}
PAGE: {page}
SOURCE REPOSITORY: {source}
SOURCE PAGE: {source_page}
TARGET REPOSITORY: {target}

PAGE QUESTION
{question}

TARGET SEARCH HINTS
{hints}

REQUIRED OUTCOMES
{required}

FORBIDDEN OUTCOMES
{forbidden}

PAGE NOTES
{notes}

VISUAL CONTRACT
{visual}

DATA TRUTH CONTRACT
{truth}

EXECUTION RULES
1. Inspect the real source page and target implementation before editing.
2. Preserve the reference visual language only where it does not conflict with real target behavior.
3. Translate React/Tailwind semantics into idiomatic Rust/Dioxus; never transliterate JSX mechanically.
4. Reuse existing BurnCloud components and APIs before inventing new ones.
5. Do not fabricate metrics, health, verification, billing, security, or runtime state.
6. UNKNOWN, loading, empty, error, configured, available, observed, and verified are distinct states.
7. Keep this page inside its ownership boundary.
8. Touch only files necessary for this page.
9. Finish the implementation in the target working tree; do not merely describe it.
10. Leave changes unstaged and uncommitted. The graph owns checkpoints and commits.
"#,
            workload = workload.name,
            page = page.name,
            source = ctx.source_dir.display(),
            source_page = ctx.source_dir.join(&page.source).display(),
            target = ctx.target_dir.display(),
            question = page.question,
            hints = bullets(&page.target_search),
            required = bullets(&page.required),
            forbidden = bullets(&page.forbidden),
            notes = if page.notes.is_empty() {
                "None"
            } else {
                &page.notes
            },
            visual = visual,
            truth = truth,
        ))
    }

    pub fn fix(
        workload: &Workload,
        page: &PageSpec,
        ctx: &AgentContext,
        findings: &[Finding],
    ) -> Result<String> {
        let base = Self::implementation(workload, page, ctx)?;
        let issues = findings
            .iter()
            .map(|f| {
                let path = f
                    .path
                    .as_deref()
                    .map(|p| format!(" ({p})"))
                    .unwrap_or_default();
                format!("- [{}] {}{}", f.gate, f.message, path)
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(format!(
            "{base}\nTHIS IS A TARGETED FIX PASS.\nThe previous implementation failed these gates:\n{issues}\n\nFix every listed failure without broadening scope.\n"
        ))
    }
}

fn bullets(items: &[String]) -> String {
    if items.is_empty() {
        return "- none".into();
    }
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone)]
pub struct AgentRunner {
    command: Vec<String>,
    #[allow(dead_code)]
    timeout_seconds: u64,
}

impl AgentRunner {
    pub fn new(spec: &AgentSpec) -> Result<Self> {
        let command = match env::var("BURNCLOUD_GRAPHS_AGENT") {
            Ok(value) if !value.trim().is_empty() => {
                shell_words::split(&value).context("invalid BURNCLOUD_GRAPHS_AGENT")?
            }
            _ => spec.command.clone(),
        };
        Ok(Self {
            command,
            timeout_seconds: spec.timeout_seconds,
        })
    }

    pub fn configured(&self) -> bool {
        !self.command.is_empty()
    }

    pub fn run(
        &self,
        prompt: &str,
        ctx: &AgentContext,
        page: &PageSpec,
        workspace: &RepoWorkspace,
    ) -> Result<CommandResult> {
        let (program, raw_args) = self
            .command
            .split_first()
            .context("implementation agent is not configured")?;
        let source = ctx.source_dir.display().to_string();
        let target = ctx.target_dir.display().to_string();
        let args = raw_args
            .iter()
            .map(|arg| {
                arg.replace("{source_dir}", &source)
                    .replace("{target_dir}", &target)
                    .replace("{page}", &page.name)
            })
            .collect::<Vec<_>>();
        workspace.run_argv(Path::new(&ctx.target_dir), program, &args, Some(prompt))
    }
}
