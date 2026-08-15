use crate::{
    config::{PageSpec, Workload},
    repo::RepoWorkspace,
    state::{Finding, GateResult},
};
use anyhow::{Context, Result};
use globset::{Glob, GlobSetBuilder};
use std::{fs, path::Path};

pub fn source_gate(page: &PageSpec, source_dir: &Path) -> GateResult {
    let path = source_dir.join(&page.source);
    if path.is_file() {
        GateResult::pass_output("source", path.display().to_string())
    } else {
        GateResult::fail(
            "source",
            vec![
                Finding::error("source", "source page does not exist")
                    .with_path(path.display().to_string()),
            ],
        )
    }
}

pub fn scope_gate(
    page: &PageSpec,
    target_dir: &Path,
    workspace: &RepoWorkspace,
) -> Result<GateResult> {
    let changed = workspace.working_files(target_dir)?;
    if changed.is_empty() {
        return Ok(GateResult::fail(
            "scope",
            vec![Finding::error("scope", "agent produced no target changes")],
        ));
    }
    if page.allowed_paths.is_empty() {
        return Ok(GateResult::pass_output("scope", changed.join("\n")));
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in &page.allowed_paths {
        builder.add(Glob::new(pattern)?);
    }
    let set = builder.build()?;
    let findings = changed
        .iter()
        .filter(|path| !set.is_match(path.as_str()))
        .map(|path| {
            Finding::error("scope", "changed file is outside this page's allowed scope")
                .with_path(path.clone())
        })
        .collect::<Vec<_>>();
    if findings.is_empty() {
        Ok(GateResult::pass_output("scope", changed.join("\n")))
    } else {
        Ok(GateResult::fail("scope", findings))
    }
}

pub fn product_gate(
    page: &PageSpec,
    target_dir: &Path,
    workspace: &RepoWorkspace,
) -> Result<GateResult> {
    let text = workspace.working_text(target_dir)?;
    let lower = text.to_ascii_lowercase();
    let mut findings = Vec::new();
    for required in &page.required {
        if !lower.contains(&required.to_ascii_lowercase()) {
            findings.push(Finding::error(
                "product",
                format!("required outcome not evidenced in page changes: {required}"),
            ));
        }
    }
    for forbidden in &page.forbidden {
        if lower.contains(&forbidden.to_ascii_lowercase()) {
            findings.push(Finding::error(
                "product",
                format!("forbidden page content remains or was introduced: {forbidden}"),
            ));
        }
    }
    Ok(if findings.is_empty() {
        GateResult::pass("product")
    } else {
        GateResult::fail("product", findings)
    })
}

pub fn truth_gate(
    workload: &Workload,
    target_dir: &Path,
    workspace: &RepoWorkspace,
) -> Result<GateResult> {
    const DEFAULT_CLAIMS: &[&str] = &[
        "100% traceable",
        "100% authentic",
        "all routes verified",
        "fully traceable",
        "verified silicon",
        "zero proxy tampering",
        "hardware attested",
        "silicon attestation active",
    ];
    let text = workspace.working_text(target_dir)?;
    let lower = text.to_ascii_lowercase();
    let mut findings = Vec::new();
    for claim in DEFAULT_CLAIMS
        .iter()
        .map(|s| s.to_string())
        .chain(workload.verify.truth_forbidden_claims.iter().cloned())
    {
        if lower.contains(&claim.to_ascii_lowercase()) {
            findings.push(Finding::error(
                "truth",
                format!(
                    "unconditional trust claim found without machine-verifiable evidence: {claim}"
                ),
            ));
        }
    }
    for suspicious in [
        "unwrap_or(0)",
        "unwrap_or(0.0)",
        "unwrap_or(false)",
        "unwrap_or(\"\")",
    ] {
        if text.contains(suspicious) {
            findings.push(Finding::warning(
                "truth",
                format!("potential UNKNOWN→default coercion found: {suspicious}"),
            ));
        }
    }
    let has_errors = findings.iter().any(|f| f.severity == "error");
    Ok(if has_errors {
        GateResult::fail("truth", findings)
    } else {
        GateResult::pass_output(
            "truth",
            findings
                .iter()
                .map(|f| f.message.clone())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    })
}

pub fn visual_gate(
    page: &PageSpec,
    target_dir: &Path,
    workspace: &RepoWorkspace,
) -> Result<GateResult> {
    const REACT_COPY: &[&str] = &[
        "className=",
        "motion/react",
        "lucide-react",
        "React.useState",
        "useState(",
        "@tailwindcss",
    ];
    let text = workspace.working_text(target_dir)?;
    let lower = text.to_ascii_lowercase();
    let mut findings = Vec::new();
    for token in REACT_COPY {
        if lower.contains(&token.to_ascii_lowercase()) {
            findings.push(Finding::error(
                "visual",
                format!("mechanical React/Tailwind copy detected in Dioxus migration: {token}"),
            ));
        }
    }
    for token in &page.visual_required {
        if !lower.contains(&token.to_ascii_lowercase()) {
            findings.push(Finding::error(
                "visual",
                format!("required visual-language token missing: {token}"),
            ));
        }
    }
    for token in &page.visual_forbidden {
        if lower.contains(&token.to_ascii_lowercase()) {
            findings.push(Finding::error(
                "visual",
                format!("forbidden visual-language pattern introduced: {token}"),
            ));
        }
    }
    Ok(if findings.is_empty() {
        GateResult::pass("visual")
    } else {
        GateResult::fail("visual", findings)
    })
}

pub fn command_gate(
    name: &str,
    commands: &[String],
    target_dir: &Path,
    workspace: &RepoWorkspace,
) -> Result<GateResult> {
    let mut output = String::new();
    for command in commands {
        let result = workspace.run_command_line(target_dir, command, None)?;
        output.push_str(&format!(
            "$ {}\n{}\n{}\n",
            result.command, result.stdout, result.stderr
        ));
        if !result.ok() {
            let finding = Finding::error(
                name,
                format!("command failed ({}): {command}", result.status),
            )
            .with_detail(tail(&result.stderr, 4000));
            return Ok(GateResult::fail_output(
                name,
                vec![finding],
                tail(&output, 20_000),
            ));
        }
    }
    Ok(GateResult::pass_output(name, tail(&output, 20_000)))
}

pub fn builtin_gate(name: &str, target_dir: &Path) -> Result<GateResult> {
    match name {
        "burncloud_functional_wiring" => burncloud_functional_wiring(target_dir),
        "burncloud_product_ux" => burncloud_product_ux(target_dir),
        other => Ok(GateResult::fail(
            "builtin",
            vec![Finding::error(
                "builtin",
                format!("unknown built-in check: {other}"),
            )],
        )),
    }
}

fn read(target: &Path, rel: &str) -> Result<String> {
    fs::read_to_string(target.join(rel)).with_context(|| format!("cannot read target file {rel}"))
}

fn require(
    target: &Path,
    needle: &str,
    file: &str,
    findings: &mut Vec<Finding>,
    gate: &str,
) -> Result<()> {
    if !read(target, file)?.contains(needle) {
        findings.push(
            Finding::error(gate, format!("missing contract: '{needle}' in {file}")).with_path(file),
        );
    }
    Ok(())
}

fn forbid(
    target: &Path,
    needle: &str,
    file: &str,
    findings: &mut Vec<Finding>,
    gate: &str,
) -> Result<()> {
    if read(target, file)?.contains(needle) {
        findings.push(
            Finding::error(
                gate,
                format!("forbidden contract content: '{needle}' in {file}"),
            )
            .with_path(file),
        );
    }
    Ok(())
}

fn line_of(target: &Path, needle: &str, file: &str) -> Result<Option<usize>> {
    Ok(read(target, file)?
        .lines()
        .position(|line| line.contains(needle))
        .map(|i| i + 1))
}

fn burncloud_functional_wiring(target: &Path) -> Result<GateResult> {
    let gate = "burncloud_functional_wiring";
    let mut f = Vec::new();
    for (needle, file) in [
        ("functional_pages::{", "crates/client/src/app.rs"),
        ("auth_gate::AuthGate", "crates/client/src/app.rs"),
        ("FunctionalConsoleLayout", "crates/client/src/auth_gate.rs"),
        ("auth.clear();", "crates/client/src/functional_layout.rs"),
        (
            "pub use providers::Providers;",
            "crates/client/src/functional_pages/mod.rs",
        ),
        (
            "pub use catalog::{Models, Routes};",
            "crates/client/src/functional_pages/mod.rs",
        ),
        (
            "pub use logs_full::Logs;",
            "crates/client/src/functional_pages/mod.rs",
        ),
        (
            "pub use analytics_full::Evaluation;",
            "crates/client/src/functional_pages/mod.rs",
        ),
        (
            "pub use api_keys_live::APIKeys;",
            "crates/client/src/functional_pages/mod.rs",
        ),
        (
            "pub use access_live::Team;",
            "crates/client/src/functional_pages/mod.rs",
        ),
        ("/api/auth/login", "crates/client/src/backend.rs"),
        ("/api/auth/register", "crates/client/src/backend.rs"),
        ("/api/auth/forgot-password", "crates/client/src/backend.rs"),
        ("Authorization", "crates/client/src/backend.rs"),
        ("/console/api/list_users", "crates/client/src/backend.rs"),
        ("/console/api/user/register", "crates/client/src/backend.rs"),
        ("/console/api/user/topup", "crates/client/src/backend.rs"),
        (
            "/console/api/channel?limit=",
            "crates/client/src/backend.rs",
        ),
        ("/console/api/tokens", "crates/client/src/backend.rs"),
        ("/console/api/usage/", "crates/client/src/backend.rs"),
        ("/api/billing/summary", "crates/client/src/backend.rs"),
        ("/console/api/monitor", "crates/client/src/backend.rs"),
        ("/v1/chat/completions", "crates/client/src/backend.rs"),
        (
            "/console/api/logs?page=1&page_size=",
            "crates/client/src/observability.rs",
        ),
        (
            "/console/api/monitor/security/filters",
            "crates/client/src/functional_api.rs",
        ),
        (
            "/console/api/monitor/security/events",
            "crates/client/src/functional_api.rs",
        ),
        (
            "/console/api/monitor/security/emergency-circuit-break",
            "crates/client/src/functional_api.rs",
        ),
        (
            "/console/api/cache/stats",
            "crates/client/src/functional_api.rs",
        ),
        (
            "/console/api/cache/clear",
            "crates/client/src/functional_api.rs",
        ),
        (
            "AuthService::login",
            "crates/client/src/critical_pages/auth.rs",
        ),
        (
            "AuthService::register",
            "crates/client/src/critical_pages/auth.rs",
        ),
        (
            "UserService::list",
            "crates/client/src/critical_pages/customers_portable.rs",
        ),
        (
            "UserService::topup",
            "crates/client/src/critical_pages/customers_portable.rs",
        ),
        (
            "TokenService::create",
            "crates/client/src/functional_pages/api_keys_live.rs",
        ),
        (
            "TokenService::rotate",
            "crates/client/src/functional_pages/api_keys_live.rs",
        ),
        (
            "TokenService::delete",
            "crates/client/src/functional_pages/api_keys_live.rs",
        ),
        (
            "ChannelService::create",
            "crates/client/src/functional_pages/providers.rs",
        ),
        (
            "ChannelService::list",
            "crates/client/src/functional_pages/catalog.rs",
        ),
        (
            "full_logs",
            "crates/client/src/functional_pages/logs_full.rs",
        ),
        (
            "full_logs",
            "crates/client/src/functional_pages/analytics_full.rs",
        ),
        (
            "chat_completion",
            "crates/client/src/functional_pages/playground_live.rs",
        ),
        (
            "save_security_filters",
            "crates/client/src/functional_pages/guardrails_live.rs",
        ),
        (
            "clear_cache",
            "crates/client/src/functional_pages/settings.rs",
        ),
        (
            "search_route(&query)",
            "crates/client/src/functional_layout.rs",
        ),
        (
            "div { class:\"env-chip\"",
            "crates/client/src/functional_layout.rs",
        ),
    ] {
        require(target, needle, file, &mut f, gate)?;
    }
    forbid(
        target,
        "Suspend Account",
        "crates/client/src/critical_pages/customers_portable.rs",
        &mut f,
        gate,
    )?;
    forbid(
        target,
        "Prompt Snippet",
        "crates/client/src/functional_pages/logs_full.rs",
        &mut f,
        gate,
    )?;
    Ok(if f.is_empty() {
        GateResult::pass(gate)
    } else {
        GateResult::fail(gate, f)
    })
}

fn burncloud_product_ux(target: &Path) -> Result<GateResult> {
    let gate = "burncloud_product_ux";
    let mut f = Vec::new();
    let nav = "crates/client/src/functional_layout.rs";
    let p = line_of(target, "NavItem { to:Route::Providers", nav)?;
    let m = line_of(target, "NavItem { to:Route::Models", nav)?;
    let r = line_of(target, "NavItem { to:Route::Routes", nav)?;
    let pg = line_of(target, "NavItem { to:Route::Playground", nav)?;
    if !matches!((p,m,r,pg), (Some(a),Some(b),Some(c),Some(d)) if a < b && b < c && c < d) {
        f.push(
            Finding::error(
                gate,
                "Traffic Setup navigation must remain Providers -> Models -> Routes -> Playground",
            )
            .with_path(nav),
        );
    }
    let l = line_of(target, "NavItem { to:Route::Logs", nav)?;
    let e = line_of(target, "NavItem { to:Route::Evaluation", nav)?;
    let b = line_of(target, "NavItem { to:Route::Billing", nav)?;
    if !matches!((l,e,b), (Some(a),Some(c),Some(d)) if a < c && c < d) {
        f.push(
            Finding::error(
                gate,
                "Observe navigation must remain Logs -> Evaluation -> Billing",
            )
            .with_path(nav),
        );
    }
    for (needle, file) in [
        (
            "Password recovery",
            "crates/client/src/critical_pages/auth.rs",
        ),
        ("Account email", "crates/client/src/critical_pages/auth.rs"),
        (
            "PROVIDER_TYPES",
            "crates/client/src/functional_pages/providers.rs",
        ),
        (
            "Provider type",
            "crates/client/src/functional_pages/providers.rs",
        ),
        (
            "Advanced routing & capacity",
            "crates/client/src/functional_pages/providers.rs",
        ),
        (
            "pending_delete",
            "crates/client/src/functional_pages/providers.rs",
        ),
        (
            "Single upstream",
            "crates/client/src/functional_pages/catalog.rs",
        ),
        ("Redundant", "crates/client/src/functional_pages/catalog.rs"),
        (
            "Unavailable",
            "crates/client/src/functional_pages/catalog.rs",
        ),
        (
            "No failover redundancy",
            "crates/client/src/functional_pages/catalog.rs",
        ),
        (
            "Playground is not ready yet",
            "crates/client/src/functional_pages/playground_live.rs",
        ),
        (
            "Connect an active provider first",
            "crates/client/src/functional_pages/playground_live.rs",
        ),
        (
            "Create an API key for the test",
            "crates/client/src/functional_pages/playground_live.rs",
        ),
        (
            "Send Test Request",
            "crates/client/src/functional_pages/playground_live.rs",
        ),
        (
            "Manage business accounts",
            "crates/client/src/critical_pages/customers_portable.rs",
        ),
        (
            "Environment operators",
            "crates/client/src/functional_pages/access_live.rs",
        ),
        (
            "Opaque management reference",
            "crates/client/src/functional_pages/api_keys_live.rs",
        ),
        (
            "One-time bearer secret",
            "crates/client/src/functional_pages/api_keys_live.rs",
        ),
        (
            "Rotate API Key",
            "crates/client/src/functional_pages/api_keys_live.rs",
        ),
        (
            "Delete API Key",
            "crates/client/src/functional_pages/api_keys_live.rs",
        ),
        (
            "Failures",
            "crates/client/src/functional_pages/logs_full.rs",
        ),
        ("Outcome", "crates/client/src/functional_pages/logs_full.rs"),
        (
            "Operational attention",
            "crates/client/src/functional_pages/analytics_full.rs",
        ),
        (
            "Spend by model",
            "crates/client/src/functional_pages/analytics.rs",
        ),
        (
            "Request Health",
            "crates/client/src/functional_pages/guardrails_live.rs",
        ),
        (
            "not a threat-intelligence feed",
            "crates/client/src/functional_pages/guardrails_live.rs",
        ),
        (
            "DANGER ZONE",
            "crates/client/src/functional_pages/guardrails_live.rs",
        ),
        (
            "MAINTENANCE",
            "crates/client/src/functional_pages/settings.rs",
        ),
        (
            "Server Configured",
            "crates/client/src/functional_layout.rs",
        ),
    ] {
        require(target, needle, file, &mut f, gate)?;
    }
    for (needle, file) in [
        (
            "Onboarding Account Preference",
            "crates/client/src/critical_pages/auth.rs",
        ),
        ("TierButton", "crates/client/src/critical_pages/auth.rs"),
        (
            "Provider Type ID",
            "crates/client/src/functional_pages/providers.rs",
        ),
        (
            "Security Score",
            "crates/client/src/functional_pages/guardrails_live.rs",
        ),
        (
            "Threat Sources",
            "crates/client/src/functional_pages/guardrails_live.rs",
        ),
        ("Server Connected", "crates/client/src/functional_layout.rs"),
    ] {
        forbid(target, needle, file, &mut f, gate)?;
    }
    Ok(if f.is_empty() {
        GateResult::pass(gate)
    } else {
        GateResult::fail(gate, f)
    })
}

fn tail(value: &str, max: usize) -> String {
    if value.len() <= max {
        return value.to_string();
    }
    value[value.len() - max..].to_string()
}
