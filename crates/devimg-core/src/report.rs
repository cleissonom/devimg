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
