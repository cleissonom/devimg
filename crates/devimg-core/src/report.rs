use crate::compare::{ManifestCompare, ManifestCompareChange, ManifestCompareOutput};
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

pub fn render_manifest_compare_report(compare: &ManifestCompare) -> String {
    let summary = &compare.summary;
    let mut out = String::new();
    out.push_str("# Dev Image Pipeline Compare Report\n\n");
    out.push_str(&format!(
        "- Base generated at: `{}`\n",
        compare.base_generated_at
    ));
    out.push_str(&format!(
        "- Head generated at: `{}`\n",
        compare.head_generated_at
    ));
    out.push_str(&format!(
        "- Base config hash: `{}`\n",
        compare.base_config_hash
    ));
    out.push_str(&format!(
        "- Head config hash: `{}`\n",
        compare.head_config_hash
    ));
    out.push_str(&format!(
        "- Variants: `{}` -> `{}` (`{}`)\n",
        summary.base_variant_count,
        summary.head_variant_count,
        format_signed(summary.variant_count_delta)
    ));
    out.push_str(&format!(
        "- Output bytes: `{}` -> `{}` (`{}`)\n",
        summary.base_output_bytes,
        summary.head_output_bytes,
        format_signed(summary.output_bytes_delta)
    ));
    out.push_str(&format!("- Added outputs: `{}`\n", summary.added_count));
    out.push_str(&format!("- Removed outputs: `{}`\n", summary.removed_count));
    out.push_str(&format!("- Changed outputs: `{}`\n", summary.changed_count));
    out.push_str(&format!(
        "- Unchanged outputs: `{}`\n",
        summary.unchanged_count
    ));

    out.push_str("\n## Added Outputs\n\n");
    push_compare_outputs(&mut out, &compare.added, "No added outputs.");

    out.push_str("\n## Removed Outputs\n\n");
    push_compare_outputs(&mut out, &compare.removed, "No removed outputs.");

    out.push_str("\n## Changed Outputs\n\n");
    if compare.changed.is_empty() {
        out.push_str("No changed outputs.\n");
    } else {
        for change in &compare.changed {
            push_changed_output(&mut out, change);
        }
    }

    out.push_str("\n## Top Byte Contributors\n\n");
    push_compare_outputs(&mut out, &compare.top_byte_contributors, "No head outputs.");
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

fn push_compare_outputs(out: &mut String, outputs: &[ManifestCompareOutput], empty: &str) {
    if outputs.is_empty() {
        out.push_str(empty);
        out.push('\n');
        return;
    }

    for output in outputs {
        out.push_str(&format!(
            "- `{}` (source: `{}`, preset: `{}`, fit: `{}`, {}x{} {}, {} bytes)\n",
            output.output_path,
            output.source_path,
            output.preset,
            output.fit,
            output.width,
            output.height,
            output.format,
            output.bytes
        ));
    }
}

fn push_changed_output(out: &mut String, change: &ManifestCompareChange) {
    let path_change = if change.base_output_path == change.head_output_path {
        format!("`{}`", change.head_output_path)
    } else {
        format!(
            "`{}` -> `{}`",
            change.base_output_path, change.head_output_path
        )
    };
    let fit_change = if change.base_fit == change.head_fit {
        format!("`{}`", change.head_fit)
    } else {
        format!("`{}` -> `{}`", change.base_fit, change.head_fit)
    };
    out.push_str(&format!(
        "- `{}` preset `{}` {}x{} {}: bytes `{}` -> `{}` (`{}`), fit {}, output {}\n",
        change.source_path,
        change.preset,
        change.width,
        change.height,
        change.format,
        change.base_bytes,
        change.head_bytes,
        format_signed(change.byte_delta),
        fit_change,
        path_change
    ));
}

fn format_signed(value: i64) -> String {
    if value > 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}
