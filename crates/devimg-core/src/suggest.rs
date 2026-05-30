use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::config::{project_relative, resolve_project_path_checked, Config};
use crate::doctor::{doctor, DoctorDiagnostic, DoctorOptions};
use crate::manifest::{read_manifest, ManifestOutput};
use crate::pipeline::{path_to_string, Operation};
use crate::warnings::{
    split_code_and_message, warning_info, BUDGET_FAILED, PLAN_NO_VARIANTS, QUALITY_ALLOWED_UPSCALE,
    QUALITY_COVER_CROP, QUALITY_GENERATED_UPSCALE, QUALITY_LOW_LOSSY,
    QUALITY_OUTPUT_LARGER_THAN_SOURCE, QUALITY_SKIPPED_UPSCALE,
};
use crate::{build_plan, scan_sources, DevimgError, Result};

const SUGGESTION_VERSION: u32 = 1;
const DEFAULT_CONFIG_PATH: &str = "devimg.toml";

#[derive(Debug, Clone, Copy)]
pub struct SuggestOptions {
    pub metadata_only: bool,
}

impl Default for SuggestOptions {
    fn default() -> Self {
        Self {
            metadata_only: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SuggestionReport {
    pub version: u32,
    pub config_path: String,
    pub mode: String,
    pub summary: SuggestionSummary,
    pub items: Vec<SuggestionItem>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SuggestionSummary {
    pub suggestion_count: usize,
    pub error_count: usize,
    pub warning_count: usize,
    pub advisory_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SuggestionItem {
    pub id: String,
    pub source_path: Option<String>,
    pub source_kind: Option<String>,
    pub output_path: Option<String>,
    pub preset: Option<String>,
    pub width: Option<u32>,
    pub format: Option<String>,
    pub warning_code: String,
    pub severity: String,
    pub acknowledged: bool,
    pub rationale: String,
    pub suggested_config: Option<SuggestedConfigPatch>,
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SuggestedConfigPatch {
    pub target: String,
    pub source_path: Option<String>,
    pub preset: Option<String>,
    pub changes: BTreeMap<String, Value>,
    pub toml_hint: String,
}

#[derive(Debug, Clone)]
struct SuggestionMetadata {
    source_path: String,
    output_path: String,
    preset: String,
    width: u32,
    format: String,
}

#[derive(Debug, Default)]
struct SuggestionContext {
    operations_by_output: BTreeMap<String, SuggestionMetadata>,
    operations_by_source_preset_width: BTreeMap<(String, String, u32), Vec<SuggestionMetadata>>,
    outputs_by_output: BTreeMap<String, SuggestionMetadata>,
    source_paths: BTreeSet<String>,
}

#[derive(Debug, Default)]
struct SuggestionFields {
    source_path: Option<String>,
    output_path: Option<String>,
    preset: Option<String>,
    width: Option<u32>,
    format: Option<String>,
}

pub fn suggest(config: &Config, options: SuggestOptions) -> Result<SuggestionReport> {
    if !options.metadata_only {
        return Err(DevimgError::config(
            &config.path,
            "devimg suggest currently requires --metadata-only",
        ));
    }

    let doctor_report = doctor(config, DoctorOptions::default())?;
    let context = SuggestionContext::new(config);
    let mut items = Vec::new();

    for diagnostic in &doctor_report.issues {
        items.push(suggestion_from_diagnostic(
            config, &context, diagnostic, "error", false,
        ));
    }
    for diagnostic in &doctor_report.warnings {
        items.push(suggestion_from_diagnostic(
            config,
            &context,
            diagnostic,
            severity_for_warning(&diagnostic.code),
            false,
        ));
    }
    for diagnostic in &doctor_report.acknowledged_warnings {
        items.push(suggestion_from_diagnostic(
            config,
            &context,
            diagnostic,
            severity_for_warning(&diagnostic.code),
            true,
        ));
    }

    items.sort_by(compare_suggestions);
    items.dedup_by(|left, right| left.id == right.id);

    let summary = SuggestionSummary {
        suggestion_count: items.len(),
        error_count: items.iter().filter(|item| item.severity == "error").count(),
        warning_count: items
            .iter()
            .filter(|item| item.severity == "warning")
            .count(),
        advisory_count: items
            .iter()
            .filter(|item| item.severity == "advisory")
            .count(),
    };

    Ok(SuggestionReport {
        version: SUGGESTION_VERSION,
        config_path: path_to_string(&config.path),
        mode: "metadata-only".to_string(),
        summary,
        items,
    })
}

pub fn suggestion_report_to_json(report: &SuggestionReport) -> String {
    serde_json::to_string_pretty(report).expect("suggestion report serialization cannot fail")
        + "\n"
}

pub fn render_suggestion_markdown(report: &SuggestionReport) -> String {
    let mut out = String::new();
    out.push_str("# DevImg Suggestions\n\n");
    out.push_str("Deterministic suggestions generated from local DevImg diagnostics. No external AI provider was called.\n\n");
    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Config: `{}`\n", report.config_path));
    out.push_str(&format!("- Mode: `{}`\n", report.mode));
    out.push_str(&format!(
        "- Suggestions: `{}`\n",
        report.summary.suggestion_count
    ));
    out.push_str(&format!("- Errors: `{}`\n", report.summary.error_count));
    out.push_str(&format!("- Warnings: `{}`\n", report.summary.warning_count));
    out.push_str(&format!(
        "- Advisories: `{}`\n\n",
        report.summary.advisory_count
    ));

    out.push_str("## Items\n\n");
    if report.items.is_empty() {
        out.push_str("No suggestions.\n");
        return out;
    }

    for item in &report.items {
        let location = item
            .output_path
            .as_ref()
            .or(item.source_path.as_ref())
            .map(String::as_str)
            .unwrap_or(".");
        out.push_str(&format!(
            "- `{}` `{}` at `{}`: {}\n",
            item.severity, item.warning_code, location, item.rationale
        ));
        if let Some(patch) = &item.suggested_config {
            let changes = serde_json::to_string(&patch.changes)
                .expect("suggested config changes serialization cannot fail");
            out.push_str(&format!(
                "  - Suggested config: target `{}` changes `{}`\n",
                patch.target, changes
            ));
            out.push_str(&format!("  - Hint: {}\n", patch.toml_hint));
        } else {
            out.push_str("  - Suggested config: manual review or regeneration required.\n");
        }
        if !item.commands.is_empty() {
            out.push_str("  - Commands:\n");
            for command in &item.commands {
                out.push_str(&format!("    - `{command}`\n"));
            }
        }
    }

    out
}

impl SuggestionContext {
    fn new(config: &Config) -> Self {
        let mut context = Self::default();

        if let Ok(sources) = scan_sources(config) {
            for source in &sources {
                context.source_paths.insert(source.project_path.clone());
            }
            if let Ok(plan) = build_plan(config, &sources) {
                for operation in &plan.operations {
                    let metadata = SuggestionMetadata::from_operation(operation);
                    context
                        .operations_by_output
                        .insert(metadata.output_path.clone(), metadata.clone());
                    context
                        .operations_by_source_preset_width
                        .entry((
                            metadata.source_path.clone(),
                            metadata.preset.clone(),
                            metadata.width,
                        ))
                        .or_default()
                        .push(metadata);
                }
            }
        }

        if let Ok(manifest_path) =
            resolve_project_path_checked(config, &config.project.manifest, "manifest path")
        {
            if let Ok(manifest) = read_manifest(&manifest_path) {
                for output in &manifest.outputs {
                    let metadata = SuggestionMetadata::from_output(output);
                    context
                        .outputs_by_output
                        .insert(metadata.output_path.clone(), metadata);
                }
            }
        }

        context
    }

    fn enrich(&self, fields: &mut SuggestionFields) {
        if let Some(output_path) = &fields.output_path {
            if let Some(metadata) = self
                .outputs_by_output
                .get(output_path)
                .or_else(|| self.operations_by_output.get(output_path))
            {
                fields.apply(metadata);
            }
        }

        let Some(source_path) = &fields.source_path else {
            return;
        };
        let Some(preset) = &fields.preset else {
            return;
        };
        let Some(width) = fields.width else {
            return;
        };
        if let Some(metadatas) = self.operations_by_source_preset_width.get(&(
            source_path.clone(),
            preset.clone(),
            width,
        )) {
            if fields.format.is_none() {
                let formats = metadatas
                    .iter()
                    .map(|metadata| metadata.format.as_str())
                    .collect::<BTreeSet<_>>();
                if formats.len() == 1 {
                    fields.format = formats.iter().next().map(|format| (*format).to_string());
                }
            }
            if fields.output_path.is_none() && metadatas.len() == 1 {
                fields.output_path = Some(metadatas[0].output_path.clone());
            }
        }
    }

    fn is_source_path(&self, path: &str) -> bool {
        self.source_paths.contains(path)
    }
}

impl SuggestionMetadata {
    fn from_operation(operation: &Operation) -> Self {
        Self {
            source_path: operation.source.project_path.clone(),
            output_path: operation.output_project_path.clone(),
            preset: operation.preset.clone(),
            width: operation.width,
            format: operation.format.label().to_string(),
        }
    }

    fn from_output(output: &ManifestOutput) -> Self {
        Self {
            source_path: output.source_path.clone(),
            output_path: output.output_path.clone(),
            preset: output.preset.clone(),
            width: output.width,
            format: output.format.clone(),
        }
    }
}

impl SuggestionFields {
    fn apply(&mut self, metadata: &SuggestionMetadata) {
        self.source_path
            .get_or_insert_with(|| metadata.source_path.clone());
        self.output_path
            .get_or_insert_with(|| metadata.output_path.clone());
        self.preset.get_or_insert_with(|| metadata.preset.clone());
        self.width.get_or_insert(metadata.width);
        self.format.get_or_insert_with(|| metadata.format.clone());
    }
}

fn suggestion_from_diagnostic(
    config: &Config,
    context: &SuggestionContext,
    diagnostic: &DoctorDiagnostic,
    severity: &str,
    acknowledged: bool,
) -> SuggestionItem {
    let mut fields = fields_from_diagnostic(context, diagnostic);
    context.enrich(&mut fields);
    let message = split_code_and_message(&diagnostic.message).1;
    if fields.format.is_none() {
        fields.format = lossy_format_from_message(message);
    }

    let source_kind = source_kind(&diagnostic.code, &fields, diagnostic.path.as_str());
    let suggested_config = suggested_config_patch(&diagnostic.code, &fields, message);
    let commands = commands_for_diagnostic(config, diagnostic, &fields);
    let id = suggestion_id(diagnostic, &fields, severity, acknowledged);

    SuggestionItem {
        id,
        source_path: fields.source_path,
        source_kind,
        output_path: fields.output_path,
        preset: fields.preset,
        width: fields.width,
        format: fields.format,
        warning_code: diagnostic.code.clone(),
        severity: severity.to_string(),
        acknowledged,
        rationale: diagnostic.message.clone(),
        suggested_config,
        commands,
    }
}

fn fields_from_diagnostic(
    context: &SuggestionContext,
    diagnostic: &DoctorDiagnostic,
) -> SuggestionFields {
    let mut fields = SuggestionFields::default();
    let info = warning_info(&diagnostic.message);
    if info.code == diagnostic.code {
        fields.source_path = info.source;
        fields.output_path = info.output;
        fields.preset = info.preset;
        fields.width = info.width;
    }

    if fields.output_path.is_none()
        && (context.outputs_by_output.contains_key(&diagnostic.path)
            || context.operations_by_output.contains_key(&diagnostic.path)
            || output_like_code(&diagnostic.code))
    {
        fields.output_path = Some(diagnostic.path.clone());
    }

    if fields.source_path.is_none() && context.is_source_path(&diagnostic.path) {
        fields.source_path = Some(diagnostic.path.clone());
    }

    fields
}

fn severity_for_warning(code: &str) -> &'static str {
    match code {
        "missing_report" | "output_inside_source" | "source_inside_output" | BUDGET_FAILED => {
            "warning"
        }
        code if code.starts_with("framework_") => "warning",
        _ => "advisory",
    }
}

fn source_kind(code: &str, fields: &SuggestionFields, diagnostic_path: &str) -> Option<String> {
    if fields.output_path.is_some() {
        return Some("generated".to_string());
    }
    if fields.source_path.is_some() {
        return Some("source".to_string());
    }
    if code.contains("manifest") {
        return Some("manifest".to_string());
    }
    if code.starts_with("framework_") {
        return Some("framework".to_string());
    }
    if code.starts_with("oversized") || code == BUDGET_FAILED {
        return Some("budget".to_string());
    }
    if code.contains("source") || diagnostic_path.contains("assets/") {
        return Some("source".to_string());
    }
    if code == "missing_report" {
        return Some("report".to_string());
    }
    Some("project".to_string())
}

fn suggested_config_patch(
    code: &str,
    fields: &SuggestionFields,
    message: &str,
) -> Option<SuggestedConfigPatch> {
    let mut changes = BTreeMap::new();
    let (target, hint) = match code {
        QUALITY_LOW_LOSSY => {
            if let Some(minimum) = suggested_min_quality_from_message(message) {
                changes.insert("quality".to_string(), json!(minimum));
            } else {
                changes.insert("quality".to_string(), json!("raise"));
            }
            (
                "preset",
                "Raise the preset quality for detail-sensitive lossy output, or use png when crisp text or graphics matter.",
            )
        }
        QUALITY_COVER_CROP => {
            changes.insert("fit".to_string(), json!("contain"));
            (
                "preset",
                "Consider fit = \"contain\" when the full composition must remain visible, or choose a crop anchor after visual review.",
            )
        }
        QUALITY_SKIPPED_UPSCALE => {
            changes.insert("widths".to_string(), json!("reduce to source dimensions"));
            (
                "preset",
                "Prefer a larger source image or reduce configured widths. Set allow_upscale = true only after accepting softness.",
            )
        }
        QUALITY_ALLOWED_UPSCALE | QUALITY_GENERATED_UPSCALE => {
            changes.insert("widths".to_string(), json!("reduce"));
            (
                "preset",
                "Prefer a larger source image or reduce configured widths so generated output is not upscaled.",
            )
        }
        QUALITY_OUTPUT_LARGER_THAN_SOURCE => {
            changes.insert("quality".to_string(), json!("review lower value"));
            changes.insert("formats".to_string(), json!("review output formats"));
            (
                "preset",
                "Review quality and output formats; small optimized sources or graphic assets can produce larger generated files.",
            )
        }
        PLAN_NO_VARIANTS => {
            changes.insert(
                "widths".to_string(),
                json!("add a generatable width or reduce configured widths"),
            );
            (
                "preset",
                "Adjust widths or provide a larger source so the source has at least one generated variant.",
            )
        }
        "missing_source_dir" | "invalid_source_dir" | "empty_sources" => {
            changes.insert(
                "input".to_string(),
                json!("point to a directory with source images"),
            );
            changes.insert("include".to_string(), json!("review include/exclude globs"));
            (
                "source",
                "Update the affected [[sources]] entry or add source images before regenerating.",
            )
        }
        "output_inside_source" | "source_inside_output" => {
            changes.insert("input".to_string(), json!("separate from generated output"));
            changes.insert("output".to_string(), json!("separate from source input"));
            (
                "source",
                "Keep source and generated output trees separate, or ensure generated paths are excluded from source globs.",
            )
        }
        "oversized_file" | "oversized_total" | BUDGET_FAILED => {
            changes.insert("max_total_bytes".to_string(), json!("review"));
            changes.insert("max_file_bytes".to_string(), json!("review"));
            (
                "budget",
                "Reduce generated image bytes, tune presets, or update budgets only when the higher budget is intentional.",
            )
        }
        code if code.starts_with("framework_") => {
            changes.insert(
                "manifest_export".to_string(),
                json!("verify checked helper paths"),
            );
            (
                "project",
                "Regenerate or verify manifest helper files with the same export options used by the project.",
            )
        }
        _ => return None,
    };

    Some(SuggestedConfigPatch {
        target: target.to_string(),
        source_path: fields.source_path.clone(),
        preset: fields.preset.clone(),
        changes,
        toml_hint: hint.to_string(),
    })
}

fn commands_for_diagnostic(
    config: &Config,
    diagnostic: &DoctorDiagnostic,
    fields: &SuggestionFields,
) -> Vec<String> {
    let config_path = path_to_string(&config.path);
    let mut commands = Vec::new();
    match diagnostic.code.as_str() {
        QUALITY_COVER_CROP
        | QUALITY_ALLOWED_UPSCALE
        | QUALITY_GENERATED_UPSCALE
        | QUALITY_OUTPUT_LARGER_THAN_SOURCE => {
            commands.push(review_command(config));
            commands.push(optimize_command(&config_path));
            commands.push(doctor_command(&config_path));
        }
        QUALITY_LOW_LOSSY | QUALITY_SKIPPED_UPSCALE | PLAN_NO_VARIANTS => {
            commands.push(optimize_command(&config_path));
            commands.push(doctor_command(&config_path));
        }
        "missing_manifest" | "missing" | "modified" | "stale" | "outdated_config" => {
            commands.push(optimize_command(&config_path));
            commands.push(doctor_command(&config_path));
        }
        "invalid_output" => {
            if let Some(path) = fields.output_path.as_ref().or(fields.source_path.as_ref()) {
                commands.push(format!("devimg inspect {}", shell_arg(path)));
            }
            commands.push(optimize_command(&config_path));
            commands.push(doctor_command(&config_path));
        }
        "missing_report" => {
            commands.push(check_command(&config_path));
            commands.push(doctor_command(&config_path));
        }
        "manifest_export_missing" | "manifest_export_stale" | "manifest_export_unavailable" => {
            commands
                .push("devimg manifest export --manifest <manifest> --output <file>".to_string());
            commands.push(doctor_command(&config_path));
        }
        _ => {
            commands.push(doctor_command(&config_path));
        }
    }
    dedup_commands(commands)
}

fn compare_suggestions(left: &SuggestionItem, right: &SuggestionItem) -> std::cmp::Ordering {
    severity_rank(&left.severity)
        .cmp(&severity_rank(&right.severity))
        .then_with(|| left.warning_code.cmp(&right.warning_code))
        .then_with(|| optional_cmp(&left.source_path, &right.source_path))
        .then_with(|| optional_cmp(&left.output_path, &right.output_path))
        .then_with(|| optional_cmp(&left.preset, &right.preset))
        .then_with(|| {
            left.width
                .unwrap_or_default()
                .cmp(&right.width.unwrap_or_default())
        })
        .then_with(|| optional_cmp(&left.format, &right.format))
        .then_with(|| left.id.cmp(&right.id))
}

fn optional_cmp(left: &Option<String>, right: &Option<String>) -> std::cmp::Ordering {
    left.as_deref()
        .unwrap_or("")
        .cmp(right.as_deref().unwrap_or(""))
}

fn severity_rank(severity: &str) -> u8 {
    match severity {
        "error" => 0,
        "warning" => 1,
        _ => 2,
    }
}

fn suggestion_id(
    diagnostic: &DoctorDiagnostic,
    fields: &SuggestionFields,
    severity: &str,
    acknowledged: bool,
) -> String {
    [
        severity.to_string(),
        diagnostic.code.clone(),
        fields.source_path.clone().unwrap_or_default(),
        fields.output_path.clone().unwrap_or_default(),
        fields.preset.clone().unwrap_or_default(),
        fields
            .width
            .map(|width| width.to_string())
            .unwrap_or_default(),
        fields.format.clone().unwrap_or_default(),
        acknowledged.to_string(),
    ]
    .join(":")
}

fn output_like_code(code: &str) -> bool {
    matches!(
        code,
        "missing" | "modified" | "stale" | "invalid_output" | "oversized_file"
    )
}

fn lossy_format_from_message(message: &str) -> Option<String> {
    for format in ["avif", "jpeg", "webp"] {
        if message.contains(&format!("uses {format} quality")) {
            return Some(format.to_string());
        }
    }
    None
}

fn suggested_min_quality_from_message(message: &str) -> Option<u8> {
    let raw = message.split_once("suggested minimum ")?.1;
    let digits = raw
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok()
}

fn optimize_command(config_path: &str) -> String {
    format!(
        "devimg optimize{} --allow-overwrite",
        config_option(config_path)
    )
}

fn doctor_command(config_path: &str) -> String {
    format!("devimg doctor{}", config_option(config_path))
}

fn check_command(config_path: &str) -> String {
    format!("devimg check{}", config_option(config_path))
}

fn review_command(config: &Config) -> String {
    let manifest = resolve_project_path_checked(config, &config.project.manifest, "manifest path")
        .map(|path| path_to_string(&project_relative(config, &path)))
        .unwrap_or_else(|_| path_to_string(&config.project.manifest));
    format!(
        "devimg review --manifest {} --output .devimg/review.html",
        shell_arg(&manifest)
    )
}

fn config_option(config_path: &str) -> String {
    if config_path == DEFAULT_CONFIG_PATH {
        String::new()
    } else {
        format!(" --config {}", shell_arg(config_path))
    }
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

fn dedup_commands(commands: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for command in commands {
        if seen.insert(command.clone()) {
            deduped.push(command);
        }
    }
    deduped
}

#[cfg(test)]
mod tests {
    use super::suggested_min_quality_from_message;

    #[test]
    fn parses_suggested_quality_from_lossy_warning() {
        assert_eq!(
            suggested_min_quality_from_message(
                "asset.png preset hero uses webp quality 74 below the suggested minimum 82 for screenshot assets"
            ),
            Some(82)
        );
    }
}
