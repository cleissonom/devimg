use crate::doctor::DoctorReport;
use crate::manifest::Manifest;
use crate::pipeline::OptimizeResult;

pub fn render_run_report(result: &OptimizeResult) -> String {
    let mut out = String::new();
    out.push_str("# Dev Image Pipeline Report\n\n");
    out.push_str(&format!("- Mode: `{}`\n", result.mode));
    out.push_str(&format!(
        "- Source files processed: `{}`\n",
        result.source_count
    ));
    out.push_str(&format!("- Variants planned: `{}`\n", result.planned_count));
    out.push_str(&format!(
        "- Variants generated: `{}`\n",
        result.manifest.outputs.len()
    ));
    out.push_str(&format!("- Source bytes: `{}`\n", result.source_bytes));
    out.push_str(&format!("- Output bytes: `{}`\n", result.output_bytes));
    out.push_str(&format!("- Budget status: `{}`\n", result.budget_status));
    if !result.warnings.is_empty() {
        out.push_str("\n## Warnings\n\n");
        for warning in &result.warnings {
            out.push_str(&format!("- {warning}\n"));
        }
    }
    if !result.issues.is_empty() {
        out.push_str("\n## Check Issues\n\n");
        for issue in &result.issues {
            out.push_str(&format!("- `{}`: {}\n", issue.kind, issue.message));
        }
    }
    out
}

pub fn render_manifest_report(manifest: &Manifest) -> String {
    let mut out = String::new();
    out.push_str("# Dev Image Pipeline Manifest Report\n\n");
    out.push_str(&format!("- Manifest version: `{}`\n", manifest.version));
    out.push_str(&format!("- Generated at: `{}`\n", manifest.generated_at));
    out.push_str(&format!("- Config path: `{}`\n", manifest.config_path));
    out.push_str(&format!("- Config hash: `{}`\n", manifest.config_hash));
    out.push_str(&format!("- Variants: `{}`\n", manifest.outputs.len()));
    out.push_str(&format!(
        "- Source bytes: `{}`\n",
        manifest.source_bytes_total()
    ));
    out.push_str(&format!(
        "- Output bytes: `{}`\n",
        manifest.output_bytes_total()
    ));
    out.push_str("\n## Outputs\n\n");
    for output in &manifest.outputs {
        out.push_str(&format!(
            "- `{}` -> `{}` ({}x{} {}, {} bytes)\n",
            output.source_path,
            output.output_path,
            output.width,
            output.height,
            output.format,
            output.bytes
        ));
    }
    out
}

pub fn render_doctor_report(report: &DoctorReport) -> String {
    let mut out = String::new();
    out.push_str("DevImg Doctor\n\n");
    if report.passed() {
        if report.warnings.is_empty() {
            out.push_str("Status: pass\n\n");
        } else {
            out.push_str("Status: pass with warnings\n\n");
        }
    } else {
        out.push_str("Status: action required\n\n");
    }

    out.push_str("Summary\n");
    out.push_str(&format!("- Config: `{}`\n", report.config_path));
    out.push_str(&format!("- Project root: `{}`\n", report.project_root));
    out.push_str(&format!(
        "- Source images: `{}`\n",
        report.source_image_count
    ));
    out.push_str(&format!(
        "- Variants planned: `{}`\n",
        report.planned_variant_count
    ));
    out.push_str(&format!(
        "- Variants generated: `{}`\n",
        report.generated_variant_count
    ));
    out.push_str(&format!("- Source bytes: `{}`\n", report.source_bytes));
    out.push_str(&format!("- Output bytes: `{}`\n", report.output_bytes));
    out.push_str(&format!("- Budget status: `{}`\n", report.budget.status));

    out.push_str("\nChecks\n");
    for check in &report.checks {
        out.push_str(&format!(
            "- {} `{}`: {}\n",
            check.status, check.name, check.message
        ));
    }

    if !report.issues.is_empty() {
        out.push_str("\nIssues\n");
        for issue in &report.issues {
            out.push_str(&format!(
                "- `{}` at `{}`: {}\n  Hint: {}\n",
                issue.code, issue.path, issue.message, issue.hint
            ));
        }
    }

    if !report.warnings.is_empty() {
        out.push_str("\nWarnings\n");
        for warning in &report.warnings {
            out.push_str(&format!(
                "- `{}` at `{}`: {}\n  Hint: {}\n",
                warning.code, warning.path, warning.message, warning.hint
            ));
        }
    }

    out.push_str(&format!("\nNext: {}\n", report.next_command));
    out
}
