use std::collections::HashMap;
use std::fs;

use crate::budget::{budget_issues, budget_status};
use crate::config::{project_relative, resolve_project_path_checked, Config};
use crate::hash::hash_file;
use crate::manifest::{Manifest, ManifestOutput};
use crate::pipeline::{
    path_to_string, unique_source_bytes, write_report, CheckIssue, CheckResult, OptimizeResult,
};
use crate::{build_plan, scan_sources, DevimgError, Result};

pub fn check(config: &Config) -> Result<CheckResult> {
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

    let mut actual_outputs = Vec::new();
    for operation in &plan.operations {
        match by_output.get(operation.output_project_path.as_str()) {
            Some(manifest_output) => {
                if manifest_output.operation_hash != operation.operation_hash {
                    issues.push(CheckIssue {
                        kind: "stale".to_string(),
                        path: operation.output_project_path.clone(),
                        message: "planned operation no longer matches manifest".to_string(),
                    });
                }
            }
            None => issues.push(CheckIssue {
                kind: "stale".to_string(),
                path: operation.output_project_path.clone(),
                message: "planned output is missing from manifest".to_string(),
            }),
        }

        if !operation.output_path.exists() {
            issues.push(CheckIssue {
                kind: "missing".to_string(),
                path: operation.output_project_path.clone(),
                message: "output file does not exist".to_string(),
            });
            continue;
        }

        let actual_hash = hash_file(&operation.output_path)?;
        if let Some(manifest_output) = by_output.get(operation.output_project_path.as_str()) {
            if manifest_output.hash != actual_hash {
                issues.push(CheckIssue {
                    kind: "modified".to_string(),
                    path: operation.output_project_path.clone(),
                    message: "output content hash differs from manifest".to_string(),
                });
            }
        }
        let metadata = fs::metadata(&operation.output_path)
            .map_err(|source| DevimgError::io(&operation.output_path, source))?;
        match image::image_dimensions(&operation.output_path) {
            Ok((width, height)) if width == operation.width && height == operation.height => {}
            Ok((width, height)) => issues.push(CheckIssue {
                kind: "stale".to_string(),
                path: operation.output_project_path.clone(),
                message: format!(
                    "output dimensions are {}x{}, expected {}x{}",
                    width, height, operation.width, operation.height
                ),
            }),
            Err(source) => issues.push(CheckIssue {
                kind: "invalid_output".to_string(),
                path: operation.output_project_path.clone(),
                message: source.to_string(),
            }),
        }
        actual_outputs.push(ManifestOutput {
            source_path: operation.source.project_path.clone(),
            source_hash: operation.source.hash.clone(),
            source_width: operation.source.width,
            source_height: operation.source.height,
            source_bytes: operation.source.bytes,
            output_path: operation.output_project_path.clone(),
            preset: operation.preset.clone(),
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
    let result_manifest = Manifest::new(
        path_to_string(&config.path),
        config.config_hash.clone(),
        actual_outputs,
    );
    let result = OptimizeResult {
        mode: "check".to_string(),
        source_count: sources.len(),
        planned_count: plan.operations.len(),
        source_bytes: unique_source_bytes(&sources),
        output_bytes: total_output_bytes,
        warnings: plan.warnings,
        issues,
        budget_status,
        manifest: result_manifest,
    };
    write_report(config, &result)?;
    Ok(CheckResult {
        passed: result.issues.is_empty(),
        result,
    })
}
