use crate::{
    agent::{AgentContext, AgentRunner, PromptBuilder},
    config::{PageSpec, Workload},
    repo::RepoWorkspace,
    state::{Finding, GateResult, GraphState, PageRun, Status},
    verifier,
};
use anyhow::Result;
use std::path::PathBuf;

pub struct UiGraph {
    pub workload: Workload,
    pub workspace: RepoWorkspace,
    pub source_dir: PathBuf,
    pub target_dir: PathBuf,
    pub agent: AgentRunner,
    pub state_file: PathBuf,
    pub state: GraphState,
}

impl UiGraph {
    pub fn run(mut self) -> Result<GraphState> {
        self.state.event("graph", "started");
        self.checkpoint()?;

        for page_name in self.state.selected_pages.clone() {
            if !self.run_page(&page_name)? {
                self.state
                    .event("graph", format!("stopped at failed page {page_name}"));
                self.checkpoint()?;
                return Ok(self.state);
            }
        }

        self.run_final_gates()?;
        if self.state.final_gates.iter().all(GateResult::passed) {
            let sha = self.workspace.commit_staged(
                &self.target_dir,
                &format!("Migrate UI via {}", self.workload.name),
            )?;
            self.state.commit_sha = Some(sha.clone());
            self.state.event("git", format!("committed {sha}"));
        }
        self.state.event("graph", "finished");
        self.checkpoint()?;
        Ok(self.state)
    }

    fn run_page(&mut self, page_name: &str) -> Result<bool> {
        let page = self.workload.page(page_name)?.clone();
        let source = verifier::source_gate(&page, &self.source_dir);
        if !source.passed() {
            self.state.pages.insert(
                page.name.clone(),
                PageRun {
                    name: page.name,
                    status: Status::Failed,
                    attempt: 0,
                    gates: vec![source],
                    changed_files: vec![],
                },
            );
            self.checkpoint()?;
            return Ok(false);
        }

        let agent_ctx = AgentContext {
            source_dir: self.source_dir.clone(),
            target_dir: self.target_dir.clone(),
            visual_contract: self.workload.visual_contract.clone(),
            truth_contract: self.workload.truth_contract.clone(),
        };
        let mut findings: Vec<Finding> = Vec::new();

        for attempt in 1..=self.workload.max_attempts {
            let node = if attempt == 1 { "implement" } else { "fix" };
            self.state
                .event(node, format!("{} attempt {attempt}", page.name));
            self.state.pages.insert(
                page.name.clone(),
                PageRun {
                    name: page.name.clone(),
                    status: Status::Running,
                    attempt,
                    gates: vec![source.clone()],
                    changed_files: vec![],
                },
            );
            self.checkpoint()?;

            let prompt = if attempt == 1 {
                PromptBuilder::implementation(&self.workload, &page, &agent_ctx)?
            } else {
                PromptBuilder::fix(&self.workload, &page, &agent_ctx, &findings)?
            };
            let agent = match self
                .agent
                .run(&prompt, &agent_ctx, &page, &self.workspace)
            {
                Ok(agent) => agent,
                Err(error) => {
                    let gate = GateResult::fail(
                        "agent",
                        vec![
                            Finding::error("agent", "implementation agent could not be started")
                                .with_detail(format!("{error:#}")),
                        ],
                    );
                    self.state.pages.insert(
                        page.name.clone(),
                        PageRun {
                            name: page.name.clone(),
                            status: Status::Failed,
                            attempt,
                            gates: vec![source.clone(), gate],
                            changed_files: self.workspace.working_files(&self.target_dir)?,
                        },
                    );
                    self.state.event(
                        "agent",
                        format!("{} attempt {attempt} could not start", page.name),
                    );
                    self.checkpoint()?;
                    return Ok(false);
                }
            };
            if !agent.ok() {
                let gate = GateResult::fail_output(
                    "agent",
                    vec![
                        Finding::error("agent", "implementation agent failed")
                            .with_detail(tail(&agent.stderr, 4000)),
                    ],
                    tail(&(agent.stdout + "\n" + &agent.stderr), 20_000),
                );
                self.state.pages.insert(
                    page.name.clone(),
                    PageRun {
                        name: page.name.clone(),
                        status: Status::Failed,
                        attempt,
                        gates: vec![source.clone(), gate],
                        changed_files: self.workspace.working_files(&self.target_dir)?,
                    },
                );
                self.checkpoint()?;
                return Ok(false);
            }

            let mut gates = vec![source.clone()];
            gates.extend(self.verification_gates(&page)?);
            let changed_files = self.workspace.working_files(&self.target_dir)?;
            findings = gates
                .iter()
                .filter(|gate| !gate.passed())
                .flat_map(|gate| gate.findings.clone())
                .collect();

            if findings.is_empty() {
                self.workspace.stage_working(&self.target_dir)?;
                self.state.pages.insert(
                    page.name.clone(),
                    PageRun {
                        name: page.name.clone(),
                        status: Status::Passed,
                        attempt,
                        gates,
                        changed_files,
                    },
                );
                self.state.event(
                    "page",
                    format!("{} passed all gates and was staged", page.name),
                );
                self.checkpoint()?;
                return Ok(true);
            }

            self.state.pages.insert(
                page.name.clone(),
                PageRun {
                    name: page.name.clone(),
                    status: Status::Running,
                    attempt,
                    gates,
                    changed_files,
                },
            );
            self.checkpoint()?;
        }

        if let Some(run) = self.state.pages.get_mut(&page.name) {
            run.status = Status::Failed;
        }
        self.state.event(
            "page",
            format!(
                "{} exhausted {} attempts",
                page.name, self.workload.max_attempts
            ),
        );
        self.checkpoint()?;
        Ok(false)
    }

    fn verification_gates(&self, page: &PageSpec) -> Result<Vec<GateResult>> {
        let mut gates = vec![
            verifier::scope_gate(page, &self.target_dir, &self.workspace)?,
            verifier::product_gate(page, &self.target_dir, &self.workspace)?,
            verifier::truth_gate(&self.workload, &self.target_dir, &self.workspace)?,
            verifier::visual_gate(page, &self.target_dir, &self.workspace)?,
        ];
        if !self.workload.verify.page_commands.is_empty() {
            gates.push(verifier::command_gate(
                "build",
                &self.workload.verify.page_commands,
                &self.target_dir,
                &self.workspace,
            )?);
        }
        Ok(gates)
    }

    fn run_final_gates(&mut self) -> Result<()> {
        let mut gates = Vec::new();
        for check in &self.workload.verify.builtin_checks {
            gates.push(verifier::builtin_gate(check, &self.target_dir)?);
        }
        if !self.workload.verify.cross_page_commands.is_empty() {
            gates.push(verifier::command_gate(
                "cross-page",
                &self.workload.verify.cross_page_commands,
                &self.target_dir,
                &self.workspace,
            )?);
        }
        if !self.workload.verify.final_commands.is_empty() {
            gates.push(verifier::command_gate(
                "final-build",
                &self.workload.verify.final_commands,
                &self.target_dir,
                &self.workspace,
            )?);
        }
        self.state.final_gates = gates;
        self.checkpoint()
    }

    fn checkpoint(&self) -> Result<()> {
        self.state.save(&self.state_file)
    }
}

fn tail(value: &str, max: usize) -> String {
    if value.len() <= max {
        return value.to_string();
    }
    value[value.len() - max..].to_string()
}
