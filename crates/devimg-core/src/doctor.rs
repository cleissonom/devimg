use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::check::{check_with_options, CheckOptions};
use crate::config::{project_relative, resolve_project_path_checked, Config};
use crate::manifest::{
    manifest_export_to_json, manifest_export_to_typescript, read_manifest, ManifestExportOptions,
};
use crate::pipeline::{path_to_string, CheckIssue};
use crate::{build_plan, scan_sources, DevimgError, Result};

#[derive(Debug, Clone, Default)]
pub struct DoctorOptions {
    pub manifest_export: Option<DoctorManifestExportOptions>,
}

#[derive(Debug, Clone)]
pub struct DoctorManifestExportOptions {
    pub output: PathBuf,
    pub format: DoctorManifestExportFormat,
    pub strip_prefix: Option<String>,
    pub url_prefix: String,
}

#[derive(Debug, Clone, Copy)]
pub enum DoctorManifestExportFormat {
    Json,
    Typescript,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub status: String,
    pub config_path: String,
    pub project_root: String,
    pub manifest_path: String,
    pub report_path: String,
    pub source_image_count: usize,
    pub planned_variant_count: usize,
    pub generated_variant_count: usize,
    pub source_bytes: u64,
    pub output_bytes: u64,
    pub budget: DoctorBudget,
    pub checks: Vec<DoctorCheck>,
    pub warnings: Vec<DoctorDiagnostic>,
    pub issues: Vec<DoctorDiagnostic>,
    pub next_command: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorBudget {
    pub status: String,
    pub max_total_bytes: Option<u64>,
    pub max_file_bytes: Option<u64>,
    pub total_output_bytes: u64,
    pub total_headroom_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorCheck {
    pub name: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorDiagnostic {
    pub code: String,
    pub path: String,
    pub message: String,
    pub hint: String,
}

impl DoctorReport {
    pub fn passed(&self) -> bool {
        self.issues.is_empty()
    }
}

pub fn doctor(config: &Config, options: DoctorOptions) -> Result<DoctorReport> {
    let config_path = path_to_string(&config.path);
    let project_root = display_path(&config.project.root);
    let manifest_path =
        resolve_project_path_checked(config, &config.project.manifest, "manifest path")?;
    let report_path = resolve_project_path_checked(config, &config.project.report, "report path")?;
    let manifest_project_path = path_to_string(&project_relative(config, &manifest_path));
    let report_project_path = path_to_string(&project_relative(config, &report_path));
    let mut checks = Vec::new();
    let mut warnings = Vec::new();
    let mut issues = Vec::new();

    checks.push(pass_check(
        "config",
        format!("loaded {}", config.path.display()),
    ));

    inspect_source_dirs(config, &mut checks, &mut warnings, &mut issues)?;

    let mut source_image_count = 0usize;
    let mut planned_variant_count = 0usize;
    let mut generated_variant_count = 0usize;
    let mut source_bytes = 0u64;
    let mut output_bytes = 0u64;
    let mut budget_status = "not evaluated".to_string();

    if issues.is_empty() {
        match scan_sources(config) {
            Ok(sources) => {
                source_image_count = sources.len();
                source_bytes = unique_source_bytes(&sources);
                if source_image_count == 0 {
                    issues.push(diagnostic(
                        "empty_sources",
                        ".",
                        "no source images matched the configured sources",
                        "Add source images, or update `include`/`exclude` in devimg.toml.",
                    ));
                }

                match build_plan(config, &sources) {
                    Ok(plan) => {
                        planned_variant_count = plan.operations.len();
                        checks.push(pass_check(
                            "plan",
                            format!("planned {planned_variant_count} variant(s)"),
                        ));
                    }
                    Err(error) => issues.push(error_diagnostic(
                        "plan_failed",
                        config_path.as_str(),
                        &error,
                        "Fix the plan error, then run `devimg doctor` again.",
                    )),
                }
            }
            Err(error) => issues.push(error_diagnostic(
                "source_scan_failed",
                config_path.as_str(),
                &error,
                "Fix the source image or config problem, then run `devimg doctor` again.",
            )),
        }
    }

    if manifest_path.exists() {
        checks.push(pass_check(
            "manifest",
            format!("found {}", manifest_project_path),
        ));
    } else {
        checks.push(fail_check(
            "manifest",
            format!("missing {}", manifest_project_path),
        ));
    }

    if report_path.exists() {
        checks.push(pass_check(
            "report",
            format!("found {}", report_project_path),
        ));
    } else {
        warnings.push(diagnostic(
            "missing_report",
            report_project_path.as_str(),
            "Markdown report is missing",
            format!("Refresh it with `{}`.", check_command(&config_path)),
        ));
    }

    if issues.is_empty() {
        match check_with_options(
            config,
            CheckOptions {
                write_report: false,
            },
        ) {
            Ok(check_result) => {
                generated_variant_count = check_result.result.manifest.outputs.len();
                output_bytes = check_result.result.output_bytes;
                budget_status = check_result.result.budget_status.clone();
                for warning in check_result.result.warnings {
                    warnings.push(diagnostic(
                        "plan_warning",
                        config_path.as_str(),
                        warning,
                        "Review the preset/source config, then run `devimg doctor` again.",
                    ));
                }
                for issue in check_result.result.issues {
                    issues.push(check_issue_diagnostic(&issue, &config_path));
                }
                if check_result.passed {
                    checks.push(pass_check(
                        "outputs",
                        format!("all {generated_variant_count} generated variant(s) are current"),
                    ));
                } else {
                    checks.push(fail_check(
                        "outputs",
                        "generated outputs are missing, stale, modified, or over budget",
                    ));
                }
            }
            Err(error) => issues.push(error_diagnostic(
                "check_failed",
                config_path.as_str(),
                &error,
                "Fix the check error, then run `devimg doctor` again.",
            )),
        }
    }

    if let Some(export_options) = options.manifest_export {
        inspect_manifest_export(&manifest_path, export_options, &mut checks, &mut issues)?;
    }

    let budget = DoctorBudget {
        status: budget_status,
        max_total_bytes: config.budgets.max_total_bytes,
        max_file_bytes: config.budgets.max_file_bytes,
        total_output_bytes: output_bytes,
        total_headroom_bytes: config
            .budgets
            .max_total_bytes
            .map(|max| max as i64 - output_bytes as i64),
    };
    let next_command = next_command(config, &issues, &warnings);
    let status = if issues.is_empty() { "pass" } else { "fail" }.to_string();

    Ok(DoctorReport {
        status,
        config_path,
        project_root,
        manifest_path: manifest_project_path,
        report_path: report_project_path,
        source_image_count,
        planned_variant_count,
        generated_variant_count,
        source_bytes,
        output_bytes,
        budget,
        checks,
        warnings,
        issues,
        next_command,
    })
}

pub fn doctor_report_to_json(report: &DoctorReport) -> String {
    serde_json::to_string_pretty(report).expect("doctor report serialization cannot fail") + "\n"
}

fn inspect_source_dirs(
    config: &Config,
    checks: &mut Vec<DoctorCheck>,
    warnings: &mut Vec<DoctorDiagnostic>,
    issues: &mut Vec<DoctorDiagnostic>,
) -> Result<()> {
    for source in &config.sources {
        let input_root = resolve_project_path_checked(config, &source.input, "source input")?;
        let output_root = resolve_project_path_checked(config, &source.output, "source output")?;
        let input_project_path = path_to_string(&project_relative(config, &input_root));
        let output_project_path = path_to_string(&project_relative(config, &output_root));

        if !input_root.exists() {
            checks.push(fail_check(
                "sources",
                format!(
                    "source `{}` input is missing: {}",
                    source.name, input_project_path
                ),
            ));
            issues.push(diagnostic(
                "missing_source_dir",
                input_project_path,
                format!("source `{}` input directory does not exist", source.name),
                "Create the directory, add images, or update `input` in devimg.toml.",
            ));
            continue;
        }
        if !input_root.is_dir() {
            checks.push(fail_check(
                "sources",
                format!(
                    "source `{}` input is not a directory: {}",
                    source.name, input_project_path
                ),
            ));
            issues.push(diagnostic(
                "invalid_source_dir",
                input_project_path,
                format!("source `{}` input is not a directory", source.name),
                "Point `input` at a directory, then run `devimg doctor` again.",
            ));
            continue;
        }

        checks.push(pass_check(
            "sources",
            format!(
                "source `{}` input exists: {}",
                source.name, input_project_path
            ),
        ));

        if output_root.starts_with(&input_root) {
            warnings.push(diagnostic(
                "output_inside_source",
                output_project_path,
                format!(
                    "source `{}` output directory is inside its input directory",
                    source.name
                ),
                "This is supported, but keep generated paths excluded from source globs.",
            ));
        } else if input_root.starts_with(&output_root) {
            warnings.push(diagnostic(
                "source_inside_output",
                input_project_path,
                format!(
                    "source `{}` input directory is inside its output directory",
                    source.name
                ),
                "Move input or output paths apart to avoid accidentally scanning generated files.",
            ));
        }
    }
    Ok(())
}

fn inspect_manifest_export(
    manifest_path: &Path,
    export_options: DoctorManifestExportOptions,
    checks: &mut Vec<DoctorCheck>,
    issues: &mut Vec<DoctorDiagnostic>,
) -> Result<()> {
    let manifest = match read_manifest(manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            issues.push(error_diagnostic(
                "manifest_export_unavailable",
                path_to_string(manifest_path),
                &error,
                "Run `devimg optimize --config <path>` before checking manifest export drift.",
            ));
            checks.push(fail_check(
                "manifest_export",
                "manifest export could not be rendered",
            ));
            return Ok(());
        }
    };
    let output = export_options.output;
    let output_label = path_to_string(&output);
    let options = ManifestExportOptions {
        strip_prefix: export_options.strip_prefix,
        url_prefix: export_options.url_prefix,
    };
    let rendered = match export_options.format {
        DoctorManifestExportFormat::Json => manifest_export_to_json(&manifest, &options),
        DoctorManifestExportFormat::Typescript => {
            manifest_export_to_typescript(&manifest, &options)
        }
    };

    match fs::read(&output) {
        Ok(current) if current == rendered.as_bytes() => checks.push(pass_check(
            "manifest_export",
            format!("export is up to date: {output_label}"),
        )),
        Ok(_) => {
            checks.push(fail_check(
                "manifest_export",
                format!("export is stale: {output_label}"),
            ));
            issues.push(diagnostic(
                "manifest_export_stale",
                output_label,
                "manifest export differs from the current manifest",
                "Regenerate it with `devimg manifest export --manifest <manifest> --output <file>`.",
            ));
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            checks.push(fail_check(
                "manifest_export",
                format!("export is missing: {output_label}"),
            ));
            issues.push(diagnostic(
                "manifest_export_missing",
                output_label,
                "manifest export file does not exist",
                "Generate it with `devimg manifest export --manifest <manifest> --output <file>`.",
            ));
        }
        Err(source) => return Err(DevimgError::io(output, source)),
    }
    Ok(())
}

fn unique_source_bytes(sources: &[crate::SourceImage]) -> u64 {
    let mut seen = Vec::<(&str, u64)>::new();
    for source in sources {
        if !seen.iter().any(|(path, _)| *path == source.project_path) {
            seen.push((&source.project_path, source.bytes));
        }
    }
    seen.iter().map(|(_, bytes)| *bytes).sum()
}

fn display_path(path: &Path) -> String {
    let rendered = path_to_string(path);
    if rendered.is_empty() {
        ".".to_string()
    } else {
        rendered
    }
}

fn check_issue_diagnostic(issue: &CheckIssue, config_path: &str) -> DoctorDiagnostic {
    let hint = match issue.kind.as_str() {
        "missing_manifest" | "missing" | "modified" | "stale" | "outdated_config"
        | "invalid_output" => format!(
            "Regenerate outputs with `{}`.",
            optimize_command(config_path)
        ),
        "oversized_file" | "oversized_total" => {
            "Reduce generated image bytes or update budgets, then run `devimg check` again."
                .to_string()
        }
        _ => "Review the issue, then run `devimg doctor` again.".to_string(),
    };
    diagnostic(
        issue.kind.as_str(),
        issue.path.as_str(),
        issue.message.as_str(),
        hint,
    )
}

fn error_diagnostic(
    code: impl Into<String>,
    path: impl Into<String>,
    error: &DevimgError,
    hint: impl Into<String>,
) -> DoctorDiagnostic {
    diagnostic(code, path, error.to_string(), hint)
}

fn diagnostic(
    code: impl Into<String>,
    path: impl Into<String>,
    message: impl Into<String>,
    hint: impl Into<String>,
) -> DoctorDiagnostic {
    DoctorDiagnostic {
        code: code.into(),
        path: path.into(),
        message: message.into(),
        hint: hint.into(),
    }
}

fn pass_check(name: impl Into<String>, message: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        name: name.into(),
        status: "pass".to_string(),
        message: message.into(),
    }
}

fn fail_check(name: impl Into<String>, message: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        name: name.into(),
        status: "fail".to_string(),
        message: message.into(),
    }
}

fn next_command(
    config: &Config,
    issues: &[DoctorDiagnostic],
    warnings: &[DoctorDiagnostic],
) -> String {
    let config_path = path_to_string(&config.path);
    if issues.iter().any(|issue| {
        matches!(
            issue.code.as_str(),
            "missing_manifest"
                | "missing"
                | "modified"
                | "stale"
                | "outdated_config"
                | "invalid_output"
        )
    }) {
        return optimize_command(&config_path);
    }
    if issues
        .iter()
        .any(|issue| issue.code.starts_with("manifest_export_"))
    {
        return "devimg manifest export --manifest <manifest> --output <file>".to_string();
    }
    if issues.iter().any(|issue| {
        matches!(
            issue.code.as_str(),
            "oversized_file" | "oversized_total" | "empty_sources" | "missing_source_dir"
        )
    }) {
        return doctor_command(&config_path);
    }
    if warnings
        .iter()
        .any(|warning| warning.code == "missing_report")
    {
        return check_command(&config_path);
    }
    check_command(&config_path)
}

fn optimize_command(config_path: &str) -> String {
    format!(
        "devimg optimize --config {} --allow-overwrite",
        shell_arg(config_path)
    )
}

fn check_command(config_path: &str) -> String {
    format!("devimg check --config {}", shell_arg(config_path))
}

fn doctor_command(config_path: &str) -> String {
    format!("devimg doctor --config {}", shell_arg(config_path))
}

fn shell_arg(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '@'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}
