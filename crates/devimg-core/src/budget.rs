use crate::config::Config;
use crate::manifest::ManifestOutput;
use crate::pipeline::{path_to_string, CheckIssue};

pub(crate) fn evaluate_budgets(
    config: &Config,
    outputs: &[ManifestOutput],
) -> (String, Vec<CheckIssue>) {
    let issues = budget_issues(config, outputs);
    let status = budget_status(&issues);
    (status, issues)
}

pub(crate) fn budget_issues(config: &Config, outputs: &[ManifestOutput]) -> Vec<CheckIssue> {
    let mut issues = Vec::new();
    for output in outputs {
        if let Some(max) = config.budgets.max_file_bytes {
            if output.bytes > max {
                issues.push(CheckIssue {
                    kind: "oversized_file".to_string(),
                    path: output.output_path.clone(),
                    message: format!("{} bytes exceeds max_file_bytes {}", output.bytes, max),
                });
            }
        }
    }
    let total = outputs.iter().map(|output| output.bytes).sum::<u64>();
    if let Some(max) = config.budgets.max_total_bytes {
        if total > max {
            issues.push(CheckIssue {
                kind: "oversized_total".to_string(),
                path: path_to_string(&config.project.manifest),
                message: format!("{total} bytes exceeds max_total_bytes {max}"),
            });
        }
    }
    issues
}

pub(crate) fn budget_status(issues: &[CheckIssue]) -> String {
    if issues
        .iter()
        .any(|issue| issue.kind.starts_with("oversized"))
    {
        "fail".to_string()
    } else {
        "pass".to_string()
    }
}
