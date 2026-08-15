use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize)]
pub struct RepoSpec {
    pub url: String,
    #[serde(default = "default_ref")]
    pub r#ref: String,
}

fn default_ref() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AgentSpec {
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    1800
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContractSpec {
    pub visual: String,
    pub truth: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VerifySpec {
    #[serde(default)]
    pub page_commands: Vec<String>,
    #[serde(default)]
    pub cross_page_commands: Vec<String>,
    #[serde(default)]
    pub final_commands: Vec<String>,
    #[serde(default)]
    pub builtin_checks: Vec<String>,
    #[serde(default)]
    pub truth_forbidden_claims: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PageSpec {
    pub name: String,
    pub source: String,
    pub question: String,
    #[serde(default)]
    pub target_search: Vec<String>,
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub forbidden: Vec<String>,
    #[serde(default)]
    pub visual_required: Vec<String>,
    #[serde(default)]
    pub visual_forbidden: Vec<String>,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize)]
struct WorkloadMeta {
    pub name: String,
    #[serde(default = "default_attempts")]
    pub max_attempts: u32,
}

fn default_attempts() -> u32 {
    3
}

#[derive(Debug, Clone, Deserialize)]
struct RawWorkload {
    workload: WorkloadMeta,
    source: RepoSpec,
    target: RepoSpec,
    #[serde(default)]
    agent: AgentSpec,
    contracts: ContractSpec,
    #[serde(default)]
    verify: VerifySpec,
    #[serde(default)]
    pages: Vec<PageSpec>,
}

#[derive(Debug, Clone)]
pub struct Workload {
    pub name: String,
    pub max_attempts: u32,
    pub source: RepoSpec,
    pub target: RepoSpec,
    pub agent: AgentSpec,
    pub verify: VerifySpec,
    pub pages: Vec<PageSpec>,
    pub visual_contract: PathBuf,
    pub truth_contract: PathBuf,
}

impl Workload {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path
            .as_ref()
            .canonicalize()
            .with_context(|| format!("cannot resolve workload {}", path.as_ref().display()))?;
        let text = fs::read_to_string(&path)
            .with_context(|| format!("cannot read workload {}", path.display()))?;
        let raw: RawWorkload = toml::from_str(&text)
            .with_context(|| format!("invalid workload TOML {}", path.display()))?;
        let root = path
            .parent()
            .and_then(Path::parent)
            .context("workload must live under a repository subdirectory such as workloads/")?;
        let workload = Self {
            name: raw.workload.name,
            max_attempts: raw.workload.max_attempts,
            source: raw.source,
            target: raw.target,
            agent: raw.agent,
            verify: raw.verify,
            pages: raw.pages,
            visual_contract: root.join(raw.contracts.visual),
            truth_contract: root.join(raw.contracts.truth),
        };
        if workload.pages.is_empty() {
            bail!("workload has no pages");
        }
        Ok(workload)
    }

    pub fn page(&self, name: &str) -> Result<&PageSpec> {
        self.pages
            .iter()
            .find(|p| p.name == name)
            .with_context(|| format!("unknown page: {name}"))
    }

    pub fn page_names(&self) -> Vec<String> {
        self.pages.iter().map(|p| p.name.clone()).collect()
    }
}
