use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::budget::{budget_issues, budget_status};
use crate::config::{project_relative, resolve_project_path_checked, Config, FormatKind};
use crate::hash::hash_file;
use crate::manifest::{Manifest, ManifestOutput};
use crate::pipeline::{
    path_to_string, unique_source_bytes, write_report, CheckIssue, CheckResult, OptimizeResult,
};
use crate::plan::legacy_operation_hash;
use crate::quality::{append_unique, manifest_quality_warnings};
use crate::transform::final_output_project_path;
use crate::warnings::split_acknowledged_warnings;
use crate::{build_plan, scan_sources, DevimgError, Result};

#[derive(Debug, Clone, Copy)]
pub struct CheckOptions {
    pub write_report: bool,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self { write_report: true }
    }
}

pub fn check(config: &Config) -> Result<CheckResult> {
    check_with_options(config, CheckOptions::default())
}

pub fn check_with_options(config: &Config, options: CheckOptions) -> Result<CheckResult> {
    let sources = scan_sources(config)?;
    let plan = build_plan(config, &sources)?;
    let manifest_path =
        resolve_project_path_checked(config, &config.project.manifest, "manifest path")?;
    let mut issues = Vec::new();
    let manifest = match crate::manifest::read_manifest(&manifest_path) {
        Ok(manifest) => manifest,
        Err(error) => {
            issues.push(CheckIssue {
                kind: "missing_manifest".to_string(),
                path: path_to_string(&project_relative(config, &manifest_path)),
                message: error.to_string(),
            });
            Manifest::new(path_to_string(&config.path), String::new(), Vec::new())
        }
    };

    if !manifest.config_hash.is_empty() && manifest.config_hash != config.config_hash {
        issues.push(CheckIssue {
            kind: "outdated_config".to_string(),
            path: path_to_string(&config.path),
            message: "manifest was generated with a different config hash".to_string(),
        });
    }

    let by_output: HashMap<&str, &ManifestOutput> = manifest
        .outputs
        .iter()
        .map(|output| (output.output_path.as_str(), output))
        .collect();
    let mut by_operation_hash = HashMap::<&str, Vec<&ManifestOutput>>::new();
    for output in &manifest.outputs {
        by_operation_hash
            .entry(output.operation_hash.as_str())
            .or_default()
            .push(output);
    }

    let mut actual_outputs = Vec::new();
    for operation in &plan.operations {
        let manifest_output = if operation.content_hash_filenames {
            hashed_manifest_output(
                operation,
                &manifest.config_hash,
                &by_operation_hash,
                &mut issues,
            )?
        } else {
            stable_manifest_output(operation, &manifest.config_hash, &by_output, &mut issues)
        };

        let actual_project_path = manifest_output
            .map(|output| output.output_path.clone())
            .unwrap_or_else(|| operation.output_project_path.clone());
        let actual_path = if let Some(output) = manifest_output {
            resolve_project_path_checked(
                config,
                &PathBuf::from(&output.output_path),
                "manifest output path",
            )?
        } else {
            operation.output_path.clone()
        };

        if !actual_path.exists() {
            issues.push(CheckIssue {
                kind: "missing".to_string(),
                path: actual_project_path,
                message: "output file does not exist".to_string(),
            });
            continue;
        }

        let actual_hash = hash_file(&actual_path)?;
        if let Some(manifest_output) = manifest_output {
            if manifest_output.hash != actual_hash {
                issues.push(CheckIssue {
                    kind: "modified".to_string(),
                    path: actual_project_path.clone(),
                    message: "output content hash differs from manifest".to_string(),
                });
            }
        }
        let metadata =
            fs::metadata(&actual_path).map_err(|source| DevimgError::io(&actual_path, source))?;
        if let Some(issue) = validate_output_file(operation, &actual_path, &actual_project_path) {
            issues.push(issue);
        }
        actual_outputs.push(ManifestOutput {
            source_path: operation.source.project_path.clone(),
            source_hash: operation.source.hash.clone(),
            source_width: operation.source.width,
            source_height: operation.source.height,
            source_bytes: operation.source.bytes,
            output_path: actual_project_path,
            preset: operation.preset.clone(),
            fit: operation.fit.label().to_string(),
            width: operation.width,
            height: operation.height,
            format: operation.format.label().to_string(),
            bytes: metadata.len(),
            hash: actual_hash,
            operation_hash: operation.operation_hash.clone(),
        });
    }

    let total_output_bytes = actual_outputs
        .iter()
        .map(|output| output.bytes)
        .sum::<u64>();
    issues.extend(budget_issues(config, &actual_outputs));
    let budget_status = budget_status(&issues);
    let generated_count = actual_outputs.len();
    let result_manifest = Manifest::new(
        path_to_string(&config.path),
        config.config_hash.clone(),
        actual_outputs,
    );
    let mut warnings = plan.warnings;
    append_unique(&mut warnings, manifest_quality_warnings(&result_manifest));
    let warning_groups = split_acknowledged_warnings(config, warnings);
    let result = OptimizeResult {
        mode: "check".to_string(),
        source_count: sources.len(),
        planned_count: plan.operations.len(),
        generated_count,
        skipped_count: 0,
        stale_count: issues.iter().filter(|issue| issue.kind == "stale").count(),
        source_bytes: unique_source_bytes(&sources),
        output_bytes: total_output_bytes,
        warnings: warning_groups.active,
        acknowledged_warnings: warning_groups.acknowledged,
        issues,
        budget_status,
        manifest: result_manifest,
    };
    if options.write_report {
        write_report(config, &result)?;
    }
    Ok(CheckResult {
        passed: result.issues.is_empty(),
        result,
    })
}

pub(crate) fn validate_output_file(
    operation: &crate::pipeline::Operation,
    actual_path: &PathBuf,
    actual_project_path: &str,
) -> Option<CheckIssue> {
    if operation.format == FormatKind::Avif {
        return validate_avif_container(actual_path)
            .err()
            .map(|message| CheckIssue {
                kind: "invalid_output".to_string(),
                path: actual_project_path.to_string(),
                message,
            });
    }

    match image::image_dimensions(actual_path) {
        Ok((width, height)) if width == operation.width && height == operation.height => None,
        Ok((width, height)) => Some(CheckIssue {
            kind: "stale".to_string(),
            path: actual_project_path.to_string(),
            message: format!(
                "output dimensions are {}x{}, expected {}x{}",
                width, height, operation.width, operation.height
            ),
        }),
        Err(source) => Some(CheckIssue {
            kind: "invalid_output".to_string(),
            path: actual_project_path.to_string(),
            message: source.to_string(),
        }),
    }
}

fn validate_avif_container(path: &PathBuf) -> std::result::Result<(), String> {
    let bytes = fs::read(path).map_err(|source| source.to_string())?;
    if bytes.len() <= 16 || &bytes[4..8] != b"ftyp" {
        return Err("AVIF output is missing the ftyp box".to_string());
    }
    if !bytes.windows(4).any(|window| window == b"avif") {
        return Err("AVIF output is missing the avif brand".to_string());
    }
    Ok(())
}

fn stable_manifest_output<'a>(
    operation: &crate::pipeline::Operation,
    manifest_config_hash: &str,
    by_output: &'a HashMap<&str, &'a ManifestOutput>,
    issues: &mut Vec<CheckIssue>,
) -> Option<&'a ManifestOutput> {
    match by_output.get(operation.output_project_path.as_str()) {
        Some(manifest_output) => {
            if !operation_hash_matches(manifest_output, operation, manifest_config_hash) {
                issues.push(CheckIssue {
                    kind: "stale".to_string(),
                    path: operation.output_project_path.clone(),
                    message: "planned operation no longer matches manifest".to_string(),
                });
            }
            Some(*manifest_output)
        }
        None => {
            issues.push(CheckIssue {
                kind: "stale".to_string(),
                path: operation.output_project_path.clone(),
                message: "planned output is missing from manifest".to_string(),
            });
            None
        }
    }
}

fn hashed_manifest_output<'a>(
    operation: &crate::pipeline::Operation,
    manifest_config_hash: &str,
    by_operation_hash: &'a HashMap<&str, Vec<&'a ManifestOutput>>,
    issues: &mut Vec<CheckIssue>,
) -> Result<Option<&'a ManifestOutput>> {
    let legacy_hash = legacy_operation_hash(operation, manifest_config_hash);
    let Some(outputs) = by_operation_hash
        .get(operation.operation_hash.as_str())
        .or_else(|| by_operation_hash.get(legacy_hash.as_str()))
    else {
        issues.push(CheckIssue {
            kind: "stale".to_string(),
            path: operation.output_project_path.clone(),
            message: "planned hashed output is missing from manifest".to_string(),
        });
        return Ok(None);
    };

    if outputs.len() > 1 {
        issues.push(CheckIssue {
            kind: "stale".to_string(),
            path: operation.output_project_path.clone(),
            message: "manifest contains duplicate outputs for one planned operation".to_string(),
        });
    }

    let output = outputs[0];
    let expected_path =
        final_output_project_path(&operation.output_project_path, &output.hash, operation)?;
    if output.output_path != expected_path {
        issues.push(CheckIssue {
            kind: "stale".to_string(),
            path: output.output_path.clone(),
            message: "manifest output path does not match its content hash".to_string(),
        });
    }

    Ok(Some(output))
}

fn operation_hash_matches(
    output: &ManifestOutput,
    operation: &crate::pipeline::Operation,
    manifest_config_hash: &str,
) -> bool {
    output.operation_hash == operation.operation_hash
        || output.operation_hash == legacy_operation_hash(operation, manifest_config_hash)
}
