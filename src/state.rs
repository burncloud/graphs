use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Pending,
    Running,
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub gate: String,
    pub message: String,
    #[serde(default = "default_severity")]
    pub severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

fn default_severity() -> String {
    "error".to_string()
}

impl Finding {
    pub fn error(gate: &str, message: impl Into<String>) -> Self {
        Self {
            gate: gate.into(),
            message: message.into(),
            severity: "error".into(),
            path: None,
            detail: None,
        }
    }

    pub fn warning(gate: &str, message: impl Into<String>) -> Self {
        Self {
            gate: gate.into(),
            message: message.into(),
            severity: "warning".into(),
            path: None,
            detail: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub name: String,
    pub status: Status,
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub output: String,
}

impl GateResult {
    pub fn pass(name: &str) -> Self {
        Self {
            name: name.into(),
            status: Status::Passed,
            findings: vec![],
            output: String::new(),
        }
    }
    pub fn pass_output(name: &str, output: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: Status::Passed,
            findings: vec![],
            output: output.into(),
        }
    }
    pub fn fail(name: &str, findings: Vec<Finding>) -> Self {
        Self {
            name: name.into(),
            status: Status::Failed,
            findings,
            output: String::new(),
        }
    }
    pub fn fail_output(name: &str, findings: Vec<Finding>, output: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: Status::Failed,
            findings,
            output: output.into(),
        }
    }
    pub fn passed(&self) -> bool {
        self.status == Status::Passed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PageRun {
    pub name: String,
    pub status: Status,
    pub attempt: u32,
    #[serde(default)]
    pub gates: Vec<GateResult>,
    #[serde(default)]
    pub changed_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub at: DateTime<Utc>,
    pub node: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphState {
    pub workload: String,
    pub source_dir: String,
    pub target_dir: String,
    pub selected_pages: Vec<String>,
    pub run_id: String,
    pub started_at: DateTime<Utc>,
    #[serde(default)]
    pub pages: BTreeMap<String, PageRun>,
    #[serde(default)]
    pub final_gates: Vec<GateResult>,
    #[serde(default)]
    pub events: Vec<Event>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_sha: Option<String>,
}

impl GraphState {
    pub fn new(
        workload: String,
        source_dir: String,
        target_dir: String,
        selected_pages: Vec<String>,
    ) -> Self {
        Self {
            workload,
            source_dir,
            target_dir,
            selected_pages,
            run_id: Uuid::new_v4().simple().to_string()[..12].to_string(),
            started_at: Utc::now(),
            pages: BTreeMap::new(),
            final_gates: vec![],
            events: vec![],
            branch: None,
            commit_sha: None,
        }
    }

    pub fn event(&mut self, node: &str, message: impl Into<String>) {
        self.events.push(Event {
            at: Utc::now(),
            node: node.into(),
            message: message.into(),
        });
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)
            .with_context(|| format!("cannot write graph state {}", path.display()))
    }
}
