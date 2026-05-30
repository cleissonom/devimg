use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{project_relative, resolve_project_path_checked, Config};
use crate::manifest::{read_manifest, Manifest};
use crate::pipeline::path_to_string;
use crate::quality::manifest_quality_warnings;
use crate::{scan_sources, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiProvider {
    Openai,
    Anthropic,
}

impl AiProvider {
    pub fn label(self) -> &'static str {
        match self {
            Self::Openai => "openai",
            Self::Anthropic => "anthropic",
        }
    }

    pub fn credential_env_var(self) -> &'static str {
        match self {
            Self::Openai => "OPENAI_API_KEY",
            Self::Anthropic => "ANTHROPIC_API_KEY",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiConsentOptions {
    pub provider: AiProvider,
    pub model: String,
    pub command: String,
    pub config_path: PathBuf,
    pub dry_run: bool,
    pub include_images: bool,
    pub output_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiConsentPreview {
    pub provider: AiProvider,
    pub model: String,
    pub command: String,
    pub dry_run_command: String,
    pub config_path: String,
    pub project_root: String,
    pub mode: String,
    pub dry_run: bool,
    pub paths_included: bool,
    pub image_bytes_included: bool,
    pub output_path: Option<String>,
    pub manifest_path: String,
    pub report_path: String,
    pub manifest_readable: bool,
    pub source_files: Vec<AiSelectedFile>,
    pub generated_outputs: Vec<AiGeneratedOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiSelectedFile {
    pub source_name: String,
    pub path: String,
    pub role: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub bytes: u64,
    pub hash: String,
    pub image_bytes_included: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiGeneratedOutput {
    pub source_path: String,
    pub output_path: String,
    pub preset: String,
    pub fit: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub bytes: u64,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiProviderResponse {
    pub provider: AiProvider,
    pub model: String,
    pub request_kind: String,
    pub network_called: bool,
    pub selected_file_count: usize,
    pub generated_output_count: usize,
}

pub trait AiProviderClient {
    fn consent_response(&self, preview: &AiConsentPreview) -> AiProviderResponse;
}

pub trait AiReviewProviderClient {
    fn review(&self, request: &AiReviewRequest) -> Result<AiReviewProviderPayload>;
}

pub trait AiAltProviderClient {
    fn alt(&self, request: &AiAltRequest) -> Result<AiAltProviderPayload>;
}

pub trait AiDraftProviderClient {
    fn draft(&self, request: &AiDraftRequest) -> Result<AiDraftProviderPayload>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MockAiProviderClient {
    provider: AiProvider,
}

impl MockAiProviderClient {
    pub fn new(provider: AiProvider) -> Self {
        Self { provider }
    }
}

impl AiProviderClient for MockAiProviderClient {
    fn consent_response(&self, preview: &AiConsentPreview) -> AiProviderResponse {
        AiProviderResponse {
            provider: self.provider,
            model: preview.model.clone(),
            request_kind: "consent-preview".to_string(),
            network_called: false,
            selected_file_count: preview.source_files.len(),
            generated_output_count: preview.generated_outputs.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiReviewOptions {
    pub provider: AiProvider,
    pub model: String,
    pub command: String,
    pub manifest_path: PathBuf,
    pub project_root: PathBuf,
    pub dry_run: bool,
    pub include_images: bool,
    pub image_detail: String,
    pub max_images: usize,
    pub ai_output_path: Option<PathBuf>,
    pub markdown_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiReviewRequest {
    pub schema_version: u32,
    pub provider: AiProvider,
    pub model: String,
    pub command: String,
    pub manifest_path: String,
    pub project_root: String,
    pub mode: String,
    pub dry_run: bool,
    pub paths_included: bool,
    pub image_bytes_included: bool,
    pub image_detail: String,
    pub max_images: usize,
    pub ai_output_path: Option<String>,
    pub markdown_path: Option<String>,
    pub manifest_summary: AiReviewManifestSummary,
    pub review_signals: Vec<String>,
    pub outputs: Vec<AiReviewOutput>,
    pub selected_images: Vec<AiReviewImageInput>,
    pub skipped_image_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiReviewManifestSummary {
    pub config_path: String,
    pub config_hash: String,
    pub source_count: usize,
    pub output_count: usize,
    pub source_bytes: u64,
    pub output_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiReviewOutput {
    pub source_path: String,
    pub output_path: String,
    pub preset: String,
    pub fit: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub bytes: u64,
    pub hash: String,
    pub source_width: u32,
    pub source_height: u32,
    pub source_bytes: u64,
    pub image_bytes_included: bool,
    pub image_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiReviewImageInput {
    pub output_path: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub hash: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AiReviewProviderPayload {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub observations: Vec<AiReviewObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AiReviewObservation {
    pub category: String,
    pub severity: String,
    pub source_path: String,
    pub preset: String,
    pub output_path: String,
    pub rationale: String,
    pub suggested_next_command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiReviewReport {
    pub schema_version: u32,
    pub provider: AiProvider,
    pub model: String,
    pub command: String,
    pub manifest_path: String,
    pub project_root: String,
    pub mode: String,
    pub dry_run: bool,
    pub paths_included: bool,
    pub image_bytes_included: bool,
    pub image_detail: String,
    pub max_images: usize,
    pub ai_output_path: Option<String>,
    pub markdown_path: Option<String>,
    pub provider_called: bool,
    pub summary: AiReviewSummary,
    pub review_signals: Vec<String>,
    pub outputs: Vec<AiReviewOutput>,
    pub selected_images: Vec<AiReviewImageInput>,
    pub provider_summary: String,
    pub observations: Vec<AiReviewObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiReviewSummary {
    pub source_count: usize,
    pub output_count: usize,
    pub selected_image_count: usize,
    pub skipped_image_count: usize,
    pub review_signal_count: usize,
    pub observation_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiAltOptions {
    pub provider: Option<AiProvider>,
    pub model: Option<String>,
    pub command: String,
    pub config_path: PathBuf,
    pub manifest_path: PathBuf,
    pub project_root: PathBuf,
    pub dry_run: bool,
    pub include_images: bool,
    pub image_detail: String,
    pub max_images: usize,
    pub output_path: Option<PathBuf>,
    pub markdown_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiAltRequest {
    pub schema_version: u32,
    pub provider: Option<AiProvider>,
    pub model: Option<String>,
    pub command: String,
    pub config_path: String,
    pub manifest_path: String,
    pub project_root: String,
    pub mode: String,
    pub dry_run: bool,
    pub paths_included: bool,
    pub image_bytes_included: bool,
    pub image_detail: String,
    pub max_images: usize,
    pub output_path: Option<String>,
    pub markdown_path: Option<String>,
    pub manifest_summary: AiAltManifestSummary,
    pub sources: Vec<AiAltSource>,
    pub selected_images: Vec<AiAltImageInput>,
    pub skipped_image_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiAltManifestSummary {
    pub config_path: String,
    pub config_hash: String,
    pub source_count: usize,
    pub output_count: usize,
    pub source_bytes: u64,
    pub output_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiAltSource {
    pub source_path: String,
    pub source_hash: String,
    pub source_width: u32,
    pub source_height: u32,
    pub source_bytes: u64,
    pub representative_image_path: String,
    pub representative_image_width: u32,
    pub representative_image_height: u32,
    pub representative_image_format: String,
    pub image_bytes_included: bool,
    pub image_detail: Option<String>,
    pub variants: Vec<AiAltVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiAltVariant {
    pub output_path: String,
    pub preset: String,
    pub fit: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub bytes: u64,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiAltImageInput {
    pub source_path: String,
    pub image_path: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub hash: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AiAltProviderPayload {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub drafts: Vec<AiAltDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AiAltDraft {
    pub source_path: String,
    pub representative_image_path: String,
    pub candidate_alt_text: String,
    pub review_note: String,
    pub confidence: String,
    pub image_category: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiAltReport {
    pub schema_version: u32,
    pub provider: Option<AiProvider>,
    pub model: Option<String>,
    pub command: String,
    pub config_path: String,
    pub manifest_path: String,
    pub project_root: String,
    pub mode: String,
    pub dry_run: bool,
    pub paths_included: bool,
    pub image_bytes_included: bool,
    pub image_detail: String,
    pub max_images: usize,
    pub output_path: Option<String>,
    pub markdown_path: Option<String>,
    pub provider_called: bool,
    pub summary: AiAltReportSummary,
    pub sources: Vec<AiAltSource>,
    pub selected_images: Vec<AiAltImageInput>,
    pub provider_summary: String,
    pub drafts: Vec<AiAltDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiAltReportSummary {
    pub source_count: usize,
    pub output_count: usize,
    pub selected_image_count: usize,
    pub skipped_image_count: usize,
    pub draft_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiDraftType {
    ReleaseNotes,
    ReadmeSnippet,
    ProjectPageCopy,
    BlogOutline,
    SocialPostOutline,
}

impl AiDraftType {
    pub fn label(self) -> &'static str {
        match self {
            Self::ReleaseNotes => "release-notes",
            Self::ReadmeSnippet => "readme-snippet",
            Self::ProjectPageCopy => "project-page-copy",
            Self::BlogOutline => "blog-outline",
            Self::SocialPostOutline => "social-post-outline",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::ReleaseNotes => "Release Notes",
            Self::ReadmeSnippet => "README Snippet",
            Self::ProjectPageCopy => "Project Page Copy",
            Self::BlogOutline => "Blog Outline",
            Self::SocialPostOutline => "Social Post Outline",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiDraftOptions {
    pub draft_type: AiDraftType,
    pub provider: Option<AiProvider>,
    pub model: Option<String>,
    pub command: String,
    pub config_path: PathBuf,
    pub project_root: PathBuf,
    pub output_path: PathBuf,
    pub dry_run: bool,
    pub compare_json_path: Option<PathBuf>,
    pub ai_review_json_path: Option<PathBuf>,
    pub review_html_path: Option<PathBuf>,
    pub changelog_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiDraftRequest {
    pub schema_version: u32,
    pub draft_type: AiDraftType,
    pub provider: Option<AiProvider>,
    pub model: Option<String>,
    pub command: String,
    pub config_path: String,
    pub project_root: String,
    pub mode: String,
    pub dry_run: bool,
    pub output_path: String,
    pub manifest_summary: AiDraftManifestSummary,
    pub report_summary: AiDraftArtifactSummary,
    pub changelog_summary: Option<AiDraftArtifactSummary>,
    pub optional_artifacts: Vec<AiDraftArtifactSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiDraftManifestSummary {
    pub path: String,
    pub readable: bool,
    pub read_error: Option<String>,
    pub config_hash: Option<String>,
    pub source_count: usize,
    pub output_count: usize,
    pub source_bytes: u64,
    pub output_bytes: u64,
    pub outputs: Vec<AiDraftManifestOutputSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiDraftManifestOutputSummary {
    pub source_path: String,
    pub output_path: String,
    pub preset: String,
    pub fit: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiDraftArtifactSummary {
    pub label: String,
    pub path: String,
    pub readable: bool,
    pub read_error: Option<String>,
    pub bytes: u64,
    pub line_count: usize,
    pub excerpt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AiDraftProviderPayload {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub sections: Vec<AiDraftSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AiDraftSection {
    pub heading: String,
    pub body: String,
    #[serde(default)]
    pub bullets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiDraftReport {
    pub schema_version: u32,
    pub draft_type: AiDraftType,
    pub provider: Option<AiProvider>,
    pub model: Option<String>,
    pub command: String,
    pub config_path: String,
    pub project_root: String,
    pub mode: String,
    pub dry_run: bool,
    pub output_path: String,
    pub provider_called: bool,
    pub summary: AiDraftReportSummary,
    pub manifest_summary: AiDraftManifestSummary,
    pub report_summary: AiDraftArtifactSummary,
    pub changelog_summary: Option<AiDraftArtifactSummary>,
    pub optional_artifacts: Vec<AiDraftArtifactSummary>,
    pub provider_summary: String,
    pub sections: Vec<AiDraftSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AiDraftReportSummary {
    pub source_count: usize,
    pub output_count: usize,
    pub optional_artifact_count: usize,
    pub section_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockAiReviewProviderClient {
    provider: AiProvider,
    fail: bool,
}

impl MockAiReviewProviderClient {
    pub fn new(provider: AiProvider) -> Self {
        Self {
            provider,
            fail: false,
        }
    }

    pub fn failing(provider: AiProvider) -> Self {
        Self {
            provider,
            fail: true,
        }
    }
}

impl AiReviewProviderClient for MockAiReviewProviderClient {
    fn review(&self, request: &AiReviewRequest) -> Result<AiReviewProviderPayload> {
        if self.fail {
            return Err(crate::DevimgError::config(
                &request.manifest_path,
                "mock AI review provider failure",
            ));
        }

        let first = request.outputs.first();
        Ok(AiReviewProviderPayload {
            summary: format!("mock {} AI review", self.provider.label()),
            observations: first
                .map(|output| {
                    vec![AiReviewObservation {
                        category: "format-quality concern".to_string(),
                        severity: "advisory".to_string(),
                        source_path: output.source_path.clone(),
                        preset: output.preset.clone(),
                        output_path: output.output_path.clone(),
                        rationale: "Mock review observation for provider boundary tests."
                            .to_string(),
                        suggested_next_command: format!(
                            "devimg review --manifest {} --output .devimg/review.html",
                            request.manifest_path
                        ),
                    }]
                })
                .unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockAiAltProviderClient {
    provider: AiProvider,
    fail: bool,
}

impl MockAiAltProviderClient {
    pub fn new(provider: AiProvider) -> Self {
        Self {
            provider,
            fail: false,
        }
    }

    pub fn failing(provider: AiProvider) -> Self {
        Self {
            provider,
            fail: true,
        }
    }
}

impl AiAltProviderClient for MockAiAltProviderClient {
    fn alt(&self, request: &AiAltRequest) -> Result<AiAltProviderPayload> {
        if self.fail {
            return Err(crate::DevimgError::config(
                &request.config_path,
                "mock AI alt provider failure",
            ));
        }

        Ok(AiAltProviderPayload {
            summary: format!("mock {} alt text", self.provider.label()),
            drafts: request
                .sources
                .iter()
                .map(|source| AiAltDraft {
                    source_path: source.source_path.clone(),
                    representative_image_path: source.representative_image_path.clone(),
                    candidate_alt_text: "Mock draft alt text for provider boundary tests."
                        .to_string(),
                    review_note: "Review before using in application code.".to_string(),
                    confidence: "medium".to_string(),
                    image_category: "unknown".to_string(),
                    warnings: vec!["needs-human-review".to_string()],
                })
                .collect(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockAiDraftProviderClient {
    provider: AiProvider,
    fail: bool,
}

impl MockAiDraftProviderClient {
    pub fn new(provider: AiProvider) -> Self {
        Self {
            provider,
            fail: false,
        }
    }

    pub fn failing(provider: AiProvider) -> Self {
        Self {
            provider,
            fail: true,
        }
    }
}

impl AiDraftProviderClient for MockAiDraftProviderClient {
    fn draft(&self, request: &AiDraftRequest) -> Result<AiDraftProviderPayload> {
        if self.fail {
            return Err(crate::DevimgError::config(
                &request.config_path,
                "mock AI draft provider failure",
            ));
        }

        Ok(AiDraftProviderPayload {
            summary: format!("mock {} draft", self.provider.label()),
            sections: vec![
                AiDraftSection {
                    heading: request.draft_type.title().to_string(),
                    body: "Mock provider draft for provider boundary tests.".to_string(),
                    bullets: vec![
                        format!("Draft type: {}", request.draft_type.label()),
                        format!(
                            "Outputs summarized: {}",
                            request.manifest_summary.output_count
                        ),
                    ],
                },
                AiDraftSection {
                    heading: "Review Notes".to_string(),
                    body: "Review before publishing.".to_string(),
                    bullets: vec!["needs-human-review".to_string()],
                },
            ],
        })
    }
}

pub fn build_ai_consent_preview(
    config: &Config,
    options: &AiConsentOptions,
) -> Result<AiConsentPreview> {
    let manifest_path =
        resolve_project_path_checked(config, &config.project.manifest, "manifest path")?;
    let report_path = resolve_project_path_checked(config, &config.project.report, "report path")?;
    let sources = scan_sources(config)?;
    let source_files = sources
        .into_iter()
        .map(|source| AiSelectedFile {
            source_name: source.source_name,
            path: source.project_path,
            role: "source-image".to_string(),
            width: source.width,
            height: source.height,
            format: source.format.label().to_string(),
            bytes: source.bytes,
            hash: source.hash,
            image_bytes_included: options.include_images,
        })
        .collect();

    let mut manifest_readable = false;
    let mut generated_outputs = Vec::new();
    if let Ok(manifest) = read_manifest(&manifest_path) {
        manifest_readable = true;
        let mut seen = BTreeSet::new();
        for output in manifest.outputs {
            if seen.insert(output.output_path.clone()) {
                generated_outputs.push(AiGeneratedOutput {
                    source_path: output.source_path,
                    output_path: output.output_path,
                    preset: output.preset,
                    fit: output.fit,
                    width: output.width,
                    height: output.height,
                    format: output.format,
                    bytes: output.bytes,
                    hash: output.hash,
                });
            }
        }
        generated_outputs.sort_by(|left, right| left.output_path.cmp(&right.output_path));
    }

    Ok(AiConsentPreview {
        provider: options.provider,
        model: options.model.clone(),
        command: options.command.clone(),
        dry_run_command: ai_consent_dry_run_command(options),
        config_path: path_to_string(&options.config_path),
        project_root: display_path(&config.project.root),
        mode: if options.include_images {
            "include-images".to_string()
        } else {
            "metadata-only".to_string()
        },
        dry_run: options.dry_run,
        paths_included: true,
        image_bytes_included: options.include_images,
        output_path: options.output_path.as_deref().map(path_to_string),
        manifest_path: path_to_string(&project_relative(config, &manifest_path)),
        report_path: path_to_string(&project_relative(config, &report_path)),
        manifest_readable,
        source_files,
        generated_outputs,
    })
}

pub fn ai_consent_preview_to_json(preview: &AiConsentPreview) -> String {
    serde_json::to_string_pretty(preview).expect("AI consent preview serialization cannot fail")
        + "\n"
}

pub fn build_ai_review_request(manifest: &Manifest, options: &AiReviewOptions) -> AiReviewRequest {
    let max_images = options.max_images.max(1);
    let image_detail = options.image_detail.clone();
    let mut outputs = manifest
        .outputs
        .iter()
        .map(|output| AiReviewOutput {
            source_path: output.source_path.clone(),
            output_path: output.output_path.clone(),
            preset: output.preset.clone(),
            fit: output.fit.clone(),
            width: output.width,
            height: output.height,
            format: output.format.clone(),
            bytes: output.bytes,
            hash: output.hash.clone(),
            source_width: output.source_width,
            source_height: output.source_height,
            source_bytes: output.source_bytes,
            image_bytes_included: false,
            image_detail: None,
        })
        .collect::<Vec<_>>();
    outputs.sort_by(|left, right| {
        left.source_path
            .cmp(&right.source_path)
            .then(left.preset.cmp(&right.preset))
            .then(left.width.cmp(&right.width))
            .then(left.height.cmp(&right.height))
            .then(left.format.cmp(&right.format))
            .then(left.output_path.cmp(&right.output_path))
    });

    let selected_images = if options.include_images {
        outputs
            .iter()
            .filter_map(|output| {
                ai_image_mime_type(&output.format).map(|mime_type| AiReviewImageInput {
                    output_path: output.output_path.clone(),
                    mime_type: mime_type.to_string(),
                    width: output.width,
                    height: output.height,
                    bytes: output.bytes,
                    hash: output.hash.clone(),
                    detail: image_detail.clone(),
                })
            })
            .take(max_images)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let selected_paths = selected_images
        .iter()
        .map(|image| image.output_path.clone())
        .collect::<BTreeSet<_>>();
    for output in &mut outputs {
        if selected_paths.contains(&output.output_path) {
            output.image_bytes_included = true;
            output.image_detail = Some(image_detail.clone());
        }
    }

    let output_count = manifest.outputs.len();
    let skipped_image_count = if options.include_images {
        output_count.saturating_sub(selected_images.len())
    } else {
        0
    };
    let mode = if options.include_images {
        "include-images"
    } else {
        "metadata-only"
    };

    AiReviewRequest {
        schema_version: 1,
        provider: options.provider,
        model: options.model.clone(),
        command: options.command.clone(),
        manifest_path: display_project_path(&options.project_root, &options.manifest_path),
        project_root: display_path(&options.project_root),
        mode: mode.to_string(),
        dry_run: options.dry_run,
        paths_included: true,
        image_bytes_included: options.include_images,
        image_detail,
        max_images,
        ai_output_path: options
            .ai_output_path
            .as_deref()
            .map(|path| display_project_path(&options.project_root, path)),
        markdown_path: options
            .markdown_path
            .as_deref()
            .map(|path| display_project_path(&options.project_root, path)),
        manifest_summary: AiReviewManifestSummary {
            config_path: manifest.config_path.clone(),
            config_hash: manifest.config_hash.clone(),
            source_count: manifest_source_count(manifest),
            output_count,
            source_bytes: manifest.source_bytes_total(),
            output_bytes: manifest.output_bytes_total(),
        },
        review_signals: manifest_quality_warnings(manifest),
        outputs,
        selected_images,
        skipped_image_count,
    }
}

pub fn build_ai_review_report(
    request: &AiReviewRequest,
    payload: AiReviewProviderPayload,
    provider_called: bool,
) -> AiReviewReport {
    let observations = payload
        .observations
        .into_iter()
        .map(normalize_ai_review_observation)
        .collect::<Vec<_>>();
    AiReviewReport {
        schema_version: request.schema_version,
        provider: request.provider,
        model: request.model.clone(),
        command: request.command.clone(),
        manifest_path: request.manifest_path.clone(),
        project_root: request.project_root.clone(),
        mode: request.mode.clone(),
        dry_run: request.dry_run,
        paths_included: request.paths_included,
        image_bytes_included: request.image_bytes_included,
        image_detail: request.image_detail.clone(),
        max_images: request.max_images,
        ai_output_path: request.ai_output_path.clone(),
        markdown_path: request.markdown_path.clone(),
        provider_called,
        summary: AiReviewSummary {
            source_count: request.manifest_summary.source_count,
            output_count: request.manifest_summary.output_count,
            selected_image_count: request.selected_images.len(),
            skipped_image_count: request.skipped_image_count,
            review_signal_count: request.review_signals.len(),
            observation_count: observations.len(),
        },
        review_signals: request.review_signals.clone(),
        outputs: request.outputs.clone(),
        selected_images: request.selected_images.clone(),
        provider_summary: payload.summary,
        observations,
    }
}

pub fn build_ai_review_dry_run_report(request: &AiReviewRequest) -> AiReviewReport {
    build_ai_review_report(
        request,
        AiReviewProviderPayload {
            summary: "Dry run only; no provider call was made.".to_string(),
            observations: Vec::new(),
        },
        false,
    )
}

pub fn ai_review_report_to_json(report: &AiReviewReport) -> String {
    serde_json::to_string_pretty(report).expect("AI review report serialization cannot fail") + "\n"
}

pub fn render_ai_review_markdown(report: &AiReviewReport) -> String {
    let mut out = String::new();
    out.push_str("# DevImg AI Review\n\n");
    out.push_str("AI observations are advisory and must be reviewed by a human before changing source images or configuration.\n\n");
    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Provider: `{}`\n", report.provider.label()));
    out.push_str(&format!("- Model: `{}`\n", report.model));
    out.push_str(&format!("- Mode: `{}`\n", report.mode));
    out.push_str(&format!("- Dry run: `{}`\n", report.dry_run));
    out.push_str(&format!(
        "- Provider called: `{}`\n",
        report.provider_called
    ));
    out.push_str(&format!("- Manifest: `{}`\n", report.manifest_path));
    out.push_str(&format!(
        "- Outputs reviewed: `{}`\n",
        report.summary.output_count
    ));
    out.push_str(&format!(
        "- Image inputs selected: `{}`\n",
        report.summary.selected_image_count
    ));
    out.push_str(&format!(
        "- Observations: `{}`\n\n",
        report.summary.observation_count
    ));

    if !report.review_signals.is_empty() {
        out.push_str("## Deterministic Signals\n\n");
        for signal in &report.review_signals {
            out.push_str(&format!("- {signal}\n"));
        }
        out.push('\n');
    }

    out.push_str("## Observations\n\n");
    if report.observations.is_empty() {
        out.push_str("No AI observations.\n");
    } else {
        for observation in &report.observations {
            out.push_str(&format!("### {}\n\n", observation.category));
            out.push_str(&format!("- Severity: `{}`\n", observation.severity));
            out.push_str(&format!("- Source: `{}`\n", observation.source_path));
            out.push_str(&format!("- Preset: `{}`\n", observation.preset));
            out.push_str(&format!("- Output: `{}`\n", observation.output_path));
            out.push_str(&format!("- Rationale: {}\n", observation.rationale));
            out.push_str(&format!(
                "- Suggested next command: `{}`\n\n",
                observation.suggested_next_command
            ));
        }
    }
    out
}

pub fn build_ai_alt_request(manifest: &Manifest, options: &AiAltOptions) -> AiAltRequest {
    let max_images = options.max_images.max(1);
    let image_detail = options.image_detail.clone();
    let mut grouped = BTreeMap::<String, AiAltSourceBuilder>::new();
    for output in &manifest.outputs {
        let source = grouped
            .entry(output.source_path.clone())
            .or_insert_with(|| AiAltSourceBuilder {
                source_path: output.source_path.clone(),
                source_hash: output.source_hash.clone(),
                source_width: output.source_width,
                source_height: output.source_height,
                source_bytes: output.source_bytes,
                variants: Vec::new(),
            });
        source.variants.push(AiAltVariant {
            output_path: output.output_path.clone(),
            preset: output.preset.clone(),
            fit: output.fit.clone(),
            width: output.width,
            height: output.height,
            format: output.format.clone(),
            bytes: output.bytes,
            hash: output.hash.clone(),
        });
    }

    let mut selected_images = Vec::new();
    let mut sources = Vec::new();
    for mut builder in grouped.into_values() {
        builder.variants.sort_by(|left, right| {
            left.preset
                .cmp(&right.preset)
                .then(left.width.cmp(&right.width))
                .then(left.height.cmp(&right.height))
                .then(left.format.cmp(&right.format))
                .then(left.output_path.cmp(&right.output_path))
        });
        let representative = builder.representative_image();
        let mut image_bytes_included = false;
        let mut image_detail_for_source = None;
        if options.include_images && selected_images.len() < max_images {
            if let Some(image) = representative.as_image_input(&builder.source_path, &image_detail)
            {
                image_bytes_included = true;
                image_detail_for_source = Some(image_detail.clone());
                selected_images.push(image);
            }
        }

        sources.push(AiAltSource {
            source_path: builder.source_path,
            source_hash: builder.source_hash,
            source_width: builder.source_width,
            source_height: builder.source_height,
            source_bytes: builder.source_bytes,
            representative_image_path: representative.path,
            representative_image_width: representative.width,
            representative_image_height: representative.height,
            representative_image_format: representative.format,
            image_bytes_included,
            image_detail: image_detail_for_source,
            variants: builder.variants,
        });
    }

    let skipped_image_count = if options.include_images {
        sources.len().saturating_sub(selected_images.len())
    } else {
        0
    };
    let mode = if options.include_images {
        "include-images"
    } else {
        "metadata-only"
    };

    AiAltRequest {
        schema_version: 1,
        provider: options.provider,
        model: options.model.clone(),
        command: options.command.clone(),
        config_path: display_project_path(&options.project_root, &options.config_path),
        manifest_path: display_project_path(&options.project_root, &options.manifest_path),
        project_root: display_path(&options.project_root),
        mode: mode.to_string(),
        dry_run: options.dry_run,
        paths_included: true,
        image_bytes_included: options.include_images,
        image_detail,
        max_images,
        output_path: options
            .output_path
            .as_deref()
            .map(|path| display_project_path(&options.project_root, path)),
        markdown_path: options
            .markdown_path
            .as_deref()
            .map(|path| display_project_path(&options.project_root, path)),
        manifest_summary: AiAltManifestSummary {
            config_path: manifest.config_path.clone(),
            config_hash: manifest.config_hash.clone(),
            source_count: manifest_source_count(manifest),
            output_count: manifest.outputs.len(),
            source_bytes: manifest.source_bytes_total(),
            output_bytes: manifest.output_bytes_total(),
        },
        sources,
        selected_images,
        skipped_image_count,
    }
}

pub fn build_ai_alt_report(
    request: &AiAltRequest,
    payload: AiAltProviderPayload,
    provider_called: bool,
) -> AiAltReport {
    let drafts = normalize_ai_alt_drafts(request, payload.drafts);
    AiAltReport {
        schema_version: request.schema_version,
        provider: request.provider,
        model: request.model.clone(),
        command: request.command.clone(),
        config_path: request.config_path.clone(),
        manifest_path: request.manifest_path.clone(),
        project_root: request.project_root.clone(),
        mode: request.mode.clone(),
        dry_run: request.dry_run,
        paths_included: request.paths_included,
        image_bytes_included: request.image_bytes_included,
        image_detail: request.image_detail.clone(),
        max_images: request.max_images,
        output_path: request.output_path.clone(),
        markdown_path: request.markdown_path.clone(),
        provider_called,
        summary: AiAltReportSummary {
            source_count: request.sources.len(),
            output_count: request.manifest_summary.output_count,
            selected_image_count: request.selected_images.len(),
            skipped_image_count: request.skipped_image_count,
            draft_count: drafts.len(),
        },
        sources: request.sources.clone(),
        selected_images: request.selected_images.clone(),
        provider_summary: payload.summary,
        drafts,
    }
}

pub fn build_ai_alt_placeholder_report(request: &AiAltRequest) -> AiAltReport {
    let drafts = request
        .sources
        .iter()
        .map(|source| AiAltDraft {
            source_path: source.source_path.clone(),
            representative_image_path: source.representative_image_path.clone(),
            candidate_alt_text: String::new(),
            review_note: "Metadata-only placeholder; inspect the image and write human-reviewed alt text before application use.".to_string(),
            confidence: "low".to_string(),
            image_category: "unknown".to_string(),
            warnings: vec!["needs-human-review".to_string()],
        })
        .collect();
    build_ai_alt_report(
        request,
        AiAltProviderPayload {
            summary: "Metadata-only placeholder; no provider call was made.".to_string(),
            drafts,
        },
        false,
    )
}

pub fn ai_alt_report_to_json(report: &AiAltReport) -> String {
    serde_json::to_string_pretty(report).expect("AI alt report serialization cannot fail") + "\n"
}

pub fn render_ai_alt_markdown(report: &AiAltReport) -> String {
    let mut out = String::new();
    out.push_str("# DevImg Alt-Text Drafts\n\n");
    out.push_str("Alt text is draft content and must be reviewed by a human before application use. DevImg does not insert this text into application code.\n\n");
    out.push_str("## Summary\n\n");
    if let Some(provider) = report.provider {
        out.push_str(&format!("- Provider: `{}`\n", provider.label()));
    } else {
        out.push_str("- Provider: `none`\n");
    }
    if let Some(model) = &report.model {
        out.push_str(&format!("- Model: `{model}`\n"));
    } else {
        out.push_str("- Model: `none`\n");
    }
    out.push_str(&format!("- Mode: `{}`\n", report.mode));
    out.push_str(&format!("- Dry run: `{}`\n", report.dry_run));
    out.push_str(&format!(
        "- Provider called: `{}`\n",
        report.provider_called
    ));
    out.push_str(&format!("- Manifest: `{}`\n", report.manifest_path));
    out.push_str(&format!("- Sources: `{}`\n", report.summary.source_count));
    out.push_str(&format!(
        "- Image inputs selected: `{}`\n",
        report.summary.selected_image_count
    ));
    out.push_str(&format!("- Drafts: `{}`\n\n", report.summary.draft_count));

    out.push_str("## Drafts\n\n");
    if report.drafts.is_empty() {
        out.push_str("No alt-text drafts.\n");
    } else {
        for draft in &report.drafts {
            out.push_str(&format!("### {}\n\n", draft.source_path));
            out.push_str(&format!(
                "- Representative image: `{}`\n",
                draft.representative_image_path
            ));
            out.push_str(&format!("- Category: `{}`\n", draft.image_category));
            out.push_str(&format!("- Confidence: `{}`\n", draft.confidence));
            out.push_str(&format!(
                "- Candidate alt text: {}\n",
                if draft.candidate_alt_text.is_empty() {
                    "_No draft generated in metadata-only mode._"
                } else {
                    draft.candidate_alt_text.as_str()
                }
            ));
            out.push_str(&format!("- Review note: {}\n", draft.review_note));
            if !draft.warnings.is_empty() {
                out.push_str(&format!("- Warnings: `{}`\n", draft.warnings.join("`, `")));
            }
            out.push('\n');
        }
    }
    out
}

pub fn build_ai_draft_request(config: &Config, options: &AiDraftOptions) -> AiDraftRequest {
    let manifest_path =
        resolve_project_path_checked(config, &config.project.manifest, "manifest path")
            .unwrap_or_else(|_| config.project.root.join(&config.project.manifest));
    let report_path = resolve_project_path_checked(config, &config.project.report, "report path")
        .unwrap_or_else(|_| config.project.root.join(&config.project.report));
    let manifest_summary = summarize_draft_manifest(&manifest_path, &config.project.root);
    let report_summary =
        summarize_draft_artifact("devimg-report", &report_path, &config.project.root);
    let changelog_summary = options
        .changelog_path
        .as_ref()
        .map(|path| summarize_draft_artifact("changelog", path, &config.project.root));
    let mut optional_artifacts = Vec::new();
    if let Some(path) = &options.compare_json_path {
        optional_artifacts.push(summarize_draft_artifact(
            "compare-json",
            path,
            &config.project.root,
        ));
    }
    if let Some(path) = &options.ai_review_json_path {
        optional_artifacts.push(summarize_draft_artifact(
            "ai-review-json",
            path,
            &config.project.root,
        ));
    }
    if let Some(path) = &options.review_html_path {
        optional_artifacts.push(summarize_draft_artifact(
            "review-html",
            path,
            &config.project.root,
        ));
    }

    AiDraftRequest {
        schema_version: 1,
        draft_type: options.draft_type,
        provider: options.provider,
        model: options.model.clone(),
        command: options.command.clone(),
        config_path: display_project_path(&options.project_root, &options.config_path),
        project_root: display_path(&options.project_root),
        mode: if options.provider.is_some() {
            "provider-draft".to_string()
        } else {
            "metadata-only".to_string()
        },
        dry_run: options.dry_run,
        output_path: display_project_path(&options.project_root, &options.output_path),
        manifest_summary,
        report_summary,
        changelog_summary,
        optional_artifacts,
    }
}

pub fn build_ai_draft_report(
    request: &AiDraftRequest,
    payload: AiDraftProviderPayload,
    provider_called: bool,
) -> AiDraftReport {
    let sections = normalize_ai_draft_sections(request, payload.sections);
    AiDraftReport {
        schema_version: request.schema_version,
        draft_type: request.draft_type,
        provider: request.provider,
        model: request.model.clone(),
        command: request.command.clone(),
        config_path: request.config_path.clone(),
        project_root: request.project_root.clone(),
        mode: request.mode.clone(),
        dry_run: request.dry_run,
        output_path: request.output_path.clone(),
        provider_called,
        summary: AiDraftReportSummary {
            source_count: request.manifest_summary.source_count,
            output_count: request.manifest_summary.output_count,
            optional_artifact_count: request.optional_artifacts.len(),
            section_count: sections.len(),
        },
        manifest_summary: request.manifest_summary.clone(),
        report_summary: request.report_summary.clone(),
        changelog_summary: request.changelog_summary.clone(),
        optional_artifacts: request.optional_artifacts.clone(),
        provider_summary: sanitize_draft_text(&payload.summary, 800),
        sections,
    }
}

pub fn build_ai_draft_placeholder_report(request: &AiDraftRequest) -> AiDraftReport {
    build_ai_draft_report(request, local_ai_draft_payload(request), false)
}

pub fn ai_draft_report_to_json(report: &AiDraftReport) -> String {
    serde_json::to_string_pretty(report).expect("AI draft report serialization cannot fail") + "\n"
}

pub fn render_ai_draft_markdown(report: &AiDraftReport) -> String {
    let mut out = String::new();
    out.push_str("# Draft; review before publishing\n\n");
    out.push_str("This Markdown is an advisory draft generated from DevImg metadata and optional local text artifacts. DevImg does not publish, commit, post, or edit application content.\n\n");

    out.push_str("## Summary\n\n");
    out.push_str(&format!("- Draft type: `{}`\n", report.draft_type.label()));
    out.push_str(&format!("- Mode: `{}`\n", report.mode));
    out.push_str(&format!("- Dry run: `{}`\n", report.dry_run));
    out.push_str(&format!(
        "- Provider called: `{}`\n",
        report.provider_called
    ));
    if let Some(provider) = report.provider {
        out.push_str(&format!("- Provider: `{}`\n", provider.label()));
    }
    if let Some(model) = &report.model {
        out.push_str(&format!("- Model: `{model}`\n"));
    }
    out.push_str(&format!("- Config: `{}`\n", report.config_path));
    out.push_str(&format!("- Project root: `{}`\n", report.project_root));
    out.push_str(&format!("- Output: `{}`\n", report.output_path));
    out.push_str(&format!(
        "- Manifest outputs summarized: `{}`\n",
        report.summary.output_count
    ));
    out.push_str(&format!(
        "- Optional artifacts summarized: `{}`\n\n",
        report.summary.optional_artifact_count
    ));

    out.push_str("## Source Context\n\n");
    push_draft_manifest_context(&mut out, &report.manifest_summary);
    push_draft_artifact_context(&mut out, "DevImg report", &report.report_summary);
    if let Some(changelog) = &report.changelog_summary {
        push_draft_artifact_context(&mut out, "Changelog", changelog);
    }
    for artifact in &report.optional_artifacts {
        push_draft_artifact_context(&mut out, &artifact.label, artifact);
    }
    out.push('\n');

    out.push_str("## Draft\n\n");
    for section in &report.sections {
        out.push_str(&format!("### {}\n\n", section.heading));
        if !section.body.is_empty() {
            out.push_str(&section.body);
            out.push_str("\n\n");
        }
        for bullet in &section.bullets {
            out.push_str(&format!("- {bullet}\n"));
        }
        out.push('\n');
    }

    out.push_str("## Review Checklist\n\n");
    out.push_str("- Confirm the prose against the current product behavior and release state.\n");
    out.push_str("- Remove placeholders, overclaims, and internal file paths before publishing.\n");
    out.push_str("- Keep this artifact uncommitted unless a human intentionally promotes it.\n");
    out
}

fn summarize_draft_manifest(path: &Path, project_root: &Path) -> AiDraftManifestSummary {
    match read_manifest(path) {
        Ok(manifest) => {
            let mut outputs = manifest
                .outputs
                .iter()
                .map(|output| AiDraftManifestOutputSummary {
                    source_path: output.source_path.clone(),
                    output_path: output.output_path.clone(),
                    preset: output.preset.clone(),
                    fit: output.fit.clone(),
                    width: output.width,
                    height: output.height,
                    format: output.format.clone(),
                    bytes: output.bytes,
                })
                .collect::<Vec<_>>();
            outputs.sort_by(|left, right| {
                left.source_path
                    .cmp(&right.source_path)
                    .then(left.preset.cmp(&right.preset))
                    .then(left.width.cmp(&right.width))
                    .then(left.height.cmp(&right.height))
                    .then(left.format.cmp(&right.format))
                    .then(left.output_path.cmp(&right.output_path))
            });
            AiDraftManifestSummary {
                path: display_project_path(project_root, path),
                readable: true,
                read_error: None,
                config_hash: Some(manifest.config_hash.clone()),
                source_count: manifest_source_count(&manifest),
                output_count: manifest.outputs.len(),
                source_bytes: manifest.source_bytes_total(),
                output_bytes: manifest.output_bytes_total(),
                outputs,
            }
        }
        Err(error) => AiDraftManifestSummary {
            path: display_project_path(project_root, path),
            readable: false,
            read_error: Some(sanitize_draft_text(&error.to_string(), 240)),
            config_hash: None,
            source_count: 0,
            output_count: 0,
            source_bytes: 0,
            output_bytes: 0,
            outputs: Vec::new(),
        },
    }
}

fn summarize_draft_artifact(
    label: &str,
    path: &Path,
    project_root: &Path,
) -> AiDraftArtifactSummary {
    match fs::read_to_string(path) {
        Ok(raw) => AiDraftArtifactSummary {
            label: label.to_string(),
            path: display_project_path(project_root, path),
            readable: true,
            read_error: None,
            bytes: raw.len() as u64,
            line_count: raw.lines().count(),
            excerpt: deterministic_draft_excerpt(&raw),
        },
        Err(source) => AiDraftArtifactSummary {
            label: label.to_string(),
            path: display_project_path(project_root, path),
            readable: false,
            read_error: Some(if source.kind() == ErrorKind::NotFound {
                "file not found".to_string()
            } else {
                sanitize_draft_text(&source.to_string(), 240)
            }),
            bytes: 0,
            line_count: 0,
            excerpt: String::new(),
        },
    }
}

fn deterministic_draft_excerpt(raw: &str) -> String {
    sanitize_draft_text(raw, 4000)
}

fn local_ai_draft_payload(request: &AiDraftRequest) -> AiDraftProviderPayload {
    let context = draft_context_bullets(request);
    let sections = match request.draft_type {
        AiDraftType::ReleaseNotes => vec![
            AiDraftSection {
                heading: "Release Notes Draft".to_string(),
                body: "Use this as a starting point for human-written release notes.".to_string(),
                bullets: context.clone(),
            },
            AiDraftSection {
                heading: "Validation Notes".to_string(),
                body: "Before publishing, confirm that every item maps to committed code, tests, and release artifacts.".to_string(),
                bullets: artifact_review_bullets(request),
            },
        ],
        AiDraftType::ReadmeSnippet => vec![
            AiDraftSection {
                heading: "README Snippet Draft".to_string(),
                body: "Add a concise, user-facing description of the DevImg image workflow and any generated artifacts that are relevant to this project.".to_string(),
                bullets: context.clone(),
            },
            AiDraftSection {
                heading: "Integration Notes".to_string(),
                body: "Keep commands copyable, avoid provider-key assumptions, and explain which outputs are generated versus human-authored.".to_string(),
                bullets: artifact_review_bullets(request),
            },
        ],
        AiDraftType::ProjectPageCopy => vec![
            AiDraftSection {
                heading: "Project Page Copy Draft".to_string(),
                body: "Position the project around deterministic image optimization, checked generated artifacts, and reviewable AI-assisted draft workflows.".to_string(),
                bullets: context.clone(),
            },
            AiDraftSection {
                heading: "Proof Points".to_string(),
                body: "Use concrete pipeline facts from the manifest and report instead of broad marketing claims.".to_string(),
                bullets: artifact_review_bullets(request),
            },
        ],
        AiDraftType::BlogOutline => vec![
            AiDraftSection {
                heading: "Blog Outline Draft".to_string(),
                body: "Frame the post around the problem, the DevImg workflow, verification, and what remains human-reviewed.".to_string(),
                bullets: context.clone(),
            },
            AiDraftSection {
                heading: "Sections To Fill".to_string(),
                body: "Turn each bullet into a short section only after verifying the current repository state.".to_string(),
                bullets: artifact_review_bullets(request),
            },
        ],
        AiDraftType::SocialPostOutline => vec![
            AiDraftSection {
                heading: "Social Post Outline Draft".to_string(),
                body: "Draft short post angles that point to concrete DevImg outcomes without implying automatic publication or unreviewed AI prose.".to_string(),
                bullets: context.clone(),
            },
            AiDraftSection {
                heading: "Post Variants".to_string(),
                body: "Keep each variant concise and review all claims before posting.".to_string(),
                bullets: artifact_review_bullets(request),
            },
        ],
    };
    AiDraftProviderPayload {
        summary: if request.provider.is_some() && request.dry_run {
            "Dry run only; no provider call was made.".to_string()
        } else {
            "Metadata-only draft; no provider call was made.".to_string()
        },
        sections,
    }
}

fn draft_context_bullets(request: &AiDraftRequest) -> Vec<String> {
    let mut bullets = Vec::new();
    bullets.push(format!(
        "Summarized `{}` source image(s) and `{}` generated output(s).",
        request.manifest_summary.source_count, request.manifest_summary.output_count
    ));
    bullets.push(format!(
        "DevImg report `{}` is `{}`.",
        request.report_summary.path,
        if request.report_summary.readable {
            "readable"
        } else {
            "not readable"
        }
    ));
    if let Some(changelog) = &request.changelog_summary {
        bullets.push(format!(
            "Changelog `{}` is `{}`.",
            changelog.path,
            if changelog.readable {
                "readable"
            } else {
                "not readable"
            }
        ));
    }
    for artifact in &request.optional_artifacts {
        bullets.push(format!(
            "{} `{}` is `{}`.",
            artifact.label,
            artifact.path,
            if artifact.readable {
                "readable"
            } else {
                "not readable"
            }
        ));
    }
    bullets
}

fn artifact_review_bullets(request: &AiDraftRequest) -> Vec<String> {
    let mut bullets = vec![
        "Treat this draft as review input, not publishable final copy.".to_string(),
        "Do not paste internal-only paths, command output, or generated file hashes into public prose without review.".to_string(),
        format!(
            "Run `{}` and inspect `{}` before using this draft.",
            request.command, request.output_path
        ),
    ];
    if request.provider.is_some() && request.dry_run {
        bullets.push("Dry-run provider mode did not call any external AI service.".to_string());
    }
    bullets
}

fn normalize_ai_draft_sections(
    request: &AiDraftRequest,
    sections: Vec<AiDraftSection>,
) -> Vec<AiDraftSection> {
    let normalized = sections
        .into_iter()
        .filter_map(|section| {
            let heading = sanitize_draft_text(&section.heading, 120);
            let body = sanitize_draft_text(&section.body, 3000);
            let bullets = section
                .bullets
                .into_iter()
                .map(|bullet| sanitize_draft_text(&bullet, 500))
                .filter(|bullet| !bullet.is_empty())
                .collect::<Vec<_>>();
            if heading.is_empty() && body.is_empty() && bullets.is_empty() {
                None
            } else {
                Some(AiDraftSection {
                    heading: if heading.is_empty() {
                        request.draft_type.title().to_string()
                    } else {
                        heading
                    },
                    body,
                    bullets,
                })
            }
        })
        .collect::<Vec<_>>();

    if normalized.is_empty() {
        local_ai_draft_payload(request).sections
    } else {
        normalized
    }
}

fn sanitize_draft_text(raw: &str, max_bytes: usize) -> String {
    let mut sanitized = String::new();
    for (index, line) in raw
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .enumerate()
    {
        if index > 0 {
            sanitized.push('\n');
        }
        if is_sensitive_draft_line(line) {
            sanitized.push_str("[redacted sensitive provider or image data]");
        } else {
            sanitized.push_str(line.trim_end());
        }
    }

    truncate_to_bytes(sanitized.trim().to_string(), max_bytes)
}

fn is_sensitive_draft_line(line: &str) -> bool {
    let lowered = line.to_ascii_lowercase();
    lowered.contains("openai_api_key")
        || lowered.contains("anthropic_api_key")
        || lowered.contains("data:image/")
        || lowered.contains(";base64,")
}

fn truncate_to_bytes(value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }

    let mut end = 0;
    for (index, ch) in value.char_indices() {
        let next = index + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    let mut truncated = value[..end].trim_end().to_string();
    truncated.push_str("\n[excerpt truncated]");
    truncated
}

fn push_draft_manifest_context(out: &mut String, manifest: &AiDraftManifestSummary) {
    out.push_str(&format!("- Manifest: `{}`\n", manifest.path));
    out.push_str(&format!("- Manifest readable: `{}`\n", manifest.readable));
    if let Some(error) = &manifest.read_error {
        out.push_str(&format!("- Manifest read note: {error}\n"));
    }
    out.push_str(&format!("- Sources: `{}`\n", manifest.source_count));
    out.push_str(&format!("- Outputs: `{}`\n", manifest.output_count));
    out.push_str(&format!("- Source bytes: `{}`\n", manifest.source_bytes));
    out.push_str(&format!("- Output bytes: `{}`\n", manifest.output_bytes));
    if !manifest.outputs.is_empty() {
        out.push_str("- Output presets:\n");
        for output in manifest.outputs.iter().take(12) {
            out.push_str(&format!(
                "  - `{}` `{}` {}x{} `{}` (`{}` bytes)\n",
                output.source_path,
                output.preset,
                output.width,
                output.height,
                output.format,
                output.bytes
            ));
        }
        if manifest.outputs.len() > 12 {
            out.push_str("  - additional outputs omitted from Markdown context\n");
        }
    }
}

fn push_draft_artifact_context(out: &mut String, label: &str, artifact: &AiDraftArtifactSummary) {
    out.push_str(&format!("- {label}: `{}`\n", artifact.path));
    out.push_str(&format!("  - Readable: `{}`\n", artifact.readable));
    if let Some(error) = &artifact.read_error {
        out.push_str(&format!("  - Read note: {error}\n"));
    }
    if artifact.readable {
        out.push_str(&format!("  - Bytes: `{}`\n", artifact.bytes));
        out.push_str(&format!("  - Lines: `{}`\n", artifact.line_count));
        if !artifact.excerpt.is_empty() {
            out.push_str("  - Excerpt:\n\n");
            out.push_str("```text\n");
            out.push_str(&artifact.excerpt);
            out.push_str("\n```\n");
        }
    }
}

pub fn ai_image_mime_type(format: &str) -> Option<&'static str> {
    match format.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpeg" | "jpg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiAltSourceBuilder {
    source_path: String,
    source_hash: String,
    source_width: u32,
    source_height: u32,
    source_bytes: u64,
    variants: Vec<AiAltVariant>,
}

impl AiAltSourceBuilder {
    fn representative_image(&self) -> AiAltRepresentativeImage {
        let source_format = path_extension(&self.source_path);
        if ai_image_mime_type(&source_format).is_some() {
            return AiAltRepresentativeImage {
                path: self.source_path.clone(),
                width: self.source_width,
                height: self.source_height,
                format: source_format,
                bytes: self.source_bytes,
                hash: self.source_hash.clone(),
            };
        }

        self.variants
            .iter()
            .filter(|variant| ai_image_mime_type(&variant.format).is_some())
            .max_by_key(|variant| u64::from(variant.width) * u64::from(variant.height))
            .map(|variant| AiAltRepresentativeImage {
                path: variant.output_path.clone(),
                width: variant.width,
                height: variant.height,
                format: variant.format.clone(),
                bytes: variant.bytes,
                hash: variant.hash.clone(),
            })
            .or_else(|| {
                self.variants
                    .first()
                    .map(|variant| AiAltRepresentativeImage {
                        path: variant.output_path.clone(),
                        width: variant.width,
                        height: variant.height,
                        format: variant.format.clone(),
                        bytes: variant.bytes,
                        hash: variant.hash.clone(),
                    })
            })
            .unwrap_or_else(|| AiAltRepresentativeImage {
                path: self.source_path.clone(),
                width: self.source_width,
                height: self.source_height,
                format: source_format,
                bytes: self.source_bytes,
                hash: self.source_hash.clone(),
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AiAltRepresentativeImage {
    path: String,
    width: u32,
    height: u32,
    format: String,
    bytes: u64,
    hash: String,
}

impl AiAltRepresentativeImage {
    fn as_image_input(&self, source_path: &str, detail: &str) -> Option<AiAltImageInput> {
        ai_image_mime_type(&self.format).map(|mime_type| AiAltImageInput {
            source_path: source_path.to_string(),
            image_path: self.path.clone(),
            mime_type: mime_type.to_string(),
            width: self.width,
            height: self.height,
            bytes: self.bytes,
            hash: self.hash.clone(),
            detail: detail.to_string(),
        })
    }
}

fn display_path(path: &Path) -> String {
    let rendered = path_to_string(path);
    if rendered.is_empty() {
        ".".to_string()
    } else {
        rendered
    }
}

fn display_project_path(project_root: &Path, path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    };
    absolute
        .strip_prefix(project_root)
        .map(path_to_string)
        .unwrap_or_else(|_| path_to_string(path))
}

fn ai_consent_dry_run_command(options: &AiConsentOptions) -> String {
    let mut command = format!(
        "{} --config {} --ai-provider {} --model {}",
        options.command,
        shell_arg_path(&options.config_path),
        options.provider.label(),
        shell_arg(&options.model)
    );
    if options.include_images {
        command.push_str(" --include-images");
    } else {
        command.push_str(" --metadata-only");
    }
    command.push_str(" --dry-run");
    if let Some(output_path) = &options.output_path {
        command.push_str(&format!(" --output {}", shell_arg_path(output_path)));
    }
    command
}

fn shell_arg_path(path: &Path) -> String {
    shell_arg(&path_to_string(path))
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

fn manifest_source_count(manifest: &Manifest) -> usize {
    manifest
        .outputs
        .iter()
        .map(|output| output.source_path.as_str())
        .collect::<BTreeSet<_>>()
        .len()
}

fn path_extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("unknown")
        .to_ascii_lowercase()
}

fn normalize_ai_review_observation(mut observation: AiReviewObservation) -> AiReviewObservation {
    observation.severity = "advisory".to_string();
    observation.category = normalize_observation_category(&observation.category).to_string();
    observation
}

fn normalize_observation_category(category: &str) -> &'static str {
    match category {
        "crop risk" => "crop risk",
        "readability risk" => "readability risk",
        "excessive padding" => "excessive padding",
        "low-resolution source" => "low-resolution source",
        "format-quality concern" => "format-quality concern",
        "accessibility note" => "accessibility note",
        _ => "format-quality concern",
    }
}

fn normalize_ai_alt_drafts(request: &AiAltRequest, drafts: Vec<AiAltDraft>) -> Vec<AiAltDraft> {
    let mut by_source = drafts
        .into_iter()
        .map(|mut draft| {
            draft.confidence = normalize_alt_confidence(&draft.confidence).to_string();
            draft.image_category = normalize_alt_category(&draft.image_category).to_string();
            draft.warnings = normalize_alt_warnings(draft.warnings);
            draft
        })
        .map(|draft| (draft.source_path.clone(), draft))
        .collect::<BTreeMap<_, _>>();

    request
        .sources
        .iter()
        .map(|source| {
            let mut draft = by_source
                .remove(&source.source_path)
                .unwrap_or_else(|| AiAltDraft {
                    source_path: source.source_path.clone(),
                    representative_image_path: source.representative_image_path.clone(),
                    candidate_alt_text: String::new(),
                    review_note: "Provider did not return a draft for this source; write human-reviewed alt text manually.".to_string(),
                    confidence: "low".to_string(),
                    image_category: "unknown".to_string(),
                    warnings: vec!["needs-human-review".to_string()],
                });
            draft.source_path = source.source_path.clone();
            draft.representative_image_path = source.representative_image_path.clone();
            draft
        })
        .collect()
}

fn normalize_alt_confidence(confidence: &str) -> &'static str {
    match confidence {
        "low" => "low",
        "medium" => "medium",
        "high" => "high",
        _ => "low",
    }
}

fn normalize_alt_category(category: &str) -> &'static str {
    match category {
        "content-photo" => "content-photo",
        "screenshot" => "screenshot",
        "logo" => "logo",
        "illustration" => "illustration",
        "diagram" => "diagram",
        "icon" => "icon",
        "decorative" => "decorative",
        "text-heavy" => "text-heavy",
        "unknown" => "unknown",
        _ => "unknown",
    }
}

fn normalize_alt_warnings(warnings: Vec<String>) -> Vec<String> {
    let mut normalized = warnings
        .into_iter()
        .map(|warning| match warning.as_str() {
            "decorative" => "decorative",
            "text-heavy" => "text-heavy",
            "logo" => "logo",
            "screenshot" => "screenshot",
            "uncertain-description" => "uncertain-description",
            "needs-human-review" => "needs-human-review",
            _ => "needs-human-review",
        })
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        normalized.push("needs-human-review".to_string());
    }
    normalized
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        ai_alt_report_to_json, ai_consent_preview_to_json, ai_image_mime_type,
        ai_review_report_to_json, build_ai_alt_placeholder_report, build_ai_alt_report,
        build_ai_alt_request, build_ai_draft_placeholder_report, build_ai_draft_report,
        build_ai_review_dry_run_report, build_ai_review_report, build_ai_review_request,
        render_ai_alt_markdown, render_ai_draft_markdown, render_ai_review_markdown, AiAltOptions,
        AiAltProviderClient, AiConsentPreview, AiDraftArtifactSummary, AiDraftManifestSummary,
        AiDraftProviderClient, AiDraftRequest, AiDraftSection, AiDraftType, AiGeneratedOutput,
        AiProvider, AiProviderClient, AiReviewOptions, AiReviewProviderClient, AiSelectedFile,
        MockAiAltProviderClient, MockAiDraftProviderClient, MockAiProviderClient,
        MockAiReviewProviderClient,
    };
    use crate::manifest::{Manifest, ManifestOutput};

    #[test]
    fn provider_labels_and_env_vars_are_stable() {
        assert_eq!(AiProvider::Openai.label(), "openai");
        assert_eq!(AiProvider::Openai.credential_env_var(), "OPENAI_API_KEY");
        assert_eq!(AiProvider::Anthropic.label(), "anthropic");
        assert_eq!(
            AiProvider::Anthropic.credential_env_var(),
            "ANTHROPIC_API_KEY"
        );
    }

    #[test]
    fn mock_provider_client_returns_stable_non_network_response() {
        let preview = AiConsentPreview {
            provider: AiProvider::Openai,
            model: "test-model".to_string(),
            command: "devimg ai consent".to_string(),
            dry_run_command: "devimg ai consent --config devimg.toml --ai-provider openai --model test-model --metadata-only --dry-run".to_string(),
            config_path: "devimg.toml".to_string(),
            project_root: ".".to_string(),
            mode: "metadata-only".to_string(),
            dry_run: true,
            paths_included: true,
            image_bytes_included: false,
            output_path: None,
            manifest_path: "public/images/devimg-manifest.json".to_string(),
            report_path: "devimg-report.md".to_string(),
            manifest_readable: false,
            source_files: vec![AiSelectedFile {
                source_name: "portfolio".to_string(),
                path: "assets/images/sample.png".to_string(),
                role: "source-image".to_string(),
                width: 100,
                height: 50,
                format: "png".to_string(),
                bytes: 12,
                hash: "blake3:source".to_string(),
                image_bytes_included: false,
            }],
            generated_outputs: vec![AiGeneratedOutput {
                source_path: "assets/images/sample.png".to_string(),
                output_path: "public/images/generated/sample.webp".to_string(),
                preset: "card".to_string(),
                fit: "cover".to_string(),
                width: 64,
                height: 36,
                format: "webp".to_string(),
                bytes: 10,
                hash: "blake3:output".to_string(),
            }],
        };

        let response = MockAiProviderClient::new(AiProvider::Anthropic).consent_response(&preview);

        assert_eq!(response.provider, AiProvider::Anthropic);
        assert_eq!(response.model, "test-model");
        assert_eq!(response.request_kind, "consent-preview");
        assert!(!response.network_called);
        assert_eq!(response.selected_file_count, 1);
        assert_eq!(response.generated_output_count, 1);
    }

    #[test]
    fn consent_preview_json_is_timestamp_free() {
        let preview = AiConsentPreview {
            provider: AiProvider::Anthropic,
            model: "stable-model".to_string(),
            command: "devimg ai consent".to_string(),
            dry_run_command: "devimg ai consent --config devimg.toml --ai-provider anthropic --model stable-model --metadata-only --dry-run --output /tmp/consent.json".to_string(),
            config_path: "devimg.toml".to_string(),
            project_root: ".".to_string(),
            mode: "metadata-only".to_string(),
            dry_run: true,
            paths_included: true,
            image_bytes_included: false,
            output_path: Some("/tmp/consent.json".to_string()),
            manifest_path: "public/images/devimg-manifest.json".to_string(),
            report_path: "devimg-report.md".to_string(),
            manifest_readable: false,
            source_files: Vec::new(),
            generated_outputs: Vec::new(),
        };

        let json = ai_consent_preview_to_json(&preview);

        assert!(json.contains("\"provider\": \"anthropic\""));
        assert!(json.contains("\"model\": \"stable-model\""));
        assert!(!json.contains("generated_at"));
        assert!(!json.contains("timestamp"));
    }

    #[test]
    fn ai_review_request_selects_metadata_and_image_inputs() {
        let manifest = sample_manifest();
        let request = build_ai_review_request(
            &manifest,
            &AiReviewOptions {
                provider: AiProvider::Openai,
                model: "vision-model".to_string(),
                command: "devimg review --ai".to_string(),
                manifest_path: PathBuf::from("/repo/public/images/devimg-manifest.json"),
                project_root: PathBuf::from("/repo"),
                dry_run: true,
                include_images: true,
                image_detail: "low".to_string(),
                max_images: 1,
                ai_output_path: Some(PathBuf::from("/repo/devimg-ai-review.json")),
                markdown_path: Some(PathBuf::from("/tmp/devimg-ai-review.md")),
            },
        );

        assert_eq!(request.provider, AiProvider::Openai);
        assert_eq!(request.mode, "include-images");
        assert_eq!(request.manifest_path, "public/images/devimg-manifest.json");
        assert_eq!(
            request.ai_output_path.as_deref(),
            Some("devimg-ai-review.json")
        );
        assert_eq!(
            request.markdown_path.as_deref(),
            Some("/tmp/devimg-ai-review.md")
        );
        assert_eq!(request.manifest_summary.source_count, 1);
        assert_eq!(request.manifest_summary.output_count, 2);
        assert_eq!(request.outputs.len(), 2);
        assert_eq!(request.selected_images.len(), 1);
        assert_eq!(request.selected_images[0].mime_type, "image/webp");
        let webp_output = request
            .outputs
            .iter()
            .find(|output| output.format == "webp")
            .expect("webp output exists");
        let avif_output = request
            .outputs
            .iter()
            .find(|output| output.format == "avif")
            .expect("avif output exists");
        assert!(webp_output.image_bytes_included);
        assert!(!avif_output.image_bytes_included);
        assert_eq!(request.skipped_image_count, 1);
    }

    #[test]
    fn ai_review_dry_run_json_and_markdown_are_timestamp_free() {
        let manifest = sample_manifest();
        let request = build_ai_review_request(
            &manifest,
            &AiReviewOptions {
                provider: AiProvider::Openai,
                model: "dry-run-model".to_string(),
                command: "devimg review --ai".to_string(),
                manifest_path: PathBuf::from("public/images/devimg-manifest.json"),
                project_root: PathBuf::from("."),
                dry_run: true,
                include_images: false,
                image_detail: "low".to_string(),
                max_images: 8,
                ai_output_path: None,
                markdown_path: None,
            },
        );
        let report = build_ai_review_dry_run_report(&request);
        let json = ai_review_report_to_json(&report);
        let markdown = render_ai_review_markdown(&report);

        assert!(json.contains("\"provider\": \"openai\""));
        assert!(json.contains("\"dry_run\": true"));
        assert!(json.contains("\"provider_called\": false"));
        assert!(!json.contains("generated_at"));
        assert!(!json.contains("timestamp"));
        assert!(markdown.contains("# DevImg AI Review"));
        assert!(markdown.contains("Dry run: `true`"));
    }

    #[test]
    fn mock_ai_review_provider_returns_stable_payload_and_failure() {
        let manifest = sample_manifest();
        let request = build_ai_review_request(
            &manifest,
            &AiReviewOptions {
                provider: AiProvider::Anthropic,
                model: "mock-model".to_string(),
                command: "devimg review --ai".to_string(),
                manifest_path: PathBuf::from("public/images/devimg-manifest.json"),
                project_root: PathBuf::from("."),
                dry_run: false,
                include_images: false,
                image_detail: "low".to_string(),
                max_images: 8,
                ai_output_path: None,
                markdown_path: None,
            },
        );

        let payload = MockAiReviewProviderClient::new(AiProvider::Anthropic)
            .review(&request)
            .expect("mock provider succeeds");
        let report = build_ai_review_report(&request, payload, true);

        assert!(report.provider_called);
        assert_eq!(report.provider_summary, "mock anthropic AI review");
        assert_eq!(report.observations.len(), 1);
        assert_eq!(report.observations[0].severity, "advisory");

        let error = MockAiReviewProviderClient::failing(AiProvider::Openai)
            .review(&request)
            .expect_err("mock provider fails");
        assert!(error
            .to_string()
            .contains("mock AI review provider failure"));
    }

    #[test]
    fn ai_alt_request_groups_sources_and_selects_representative_images() {
        let manifest = sample_manifest();
        let request = build_ai_alt_request(
            &manifest,
            &AiAltOptions {
                provider: Some(AiProvider::Openai),
                model: Some("alt-model".to_string()),
                command: "devimg alt".to_string(),
                config_path: PathBuf::from("/repo/devimg.toml"),
                manifest_path: PathBuf::from("/repo/public/images/devimg-manifest.json"),
                project_root: PathBuf::from("/repo"),
                dry_run: true,
                include_images: true,
                image_detail: "low".to_string(),
                max_images: 1,
                output_path: Some(PathBuf::from("/repo/devimg-alt.json")),
                markdown_path: Some(PathBuf::from("/tmp/devimg-alt.md")),
            },
        );

        assert_eq!(request.provider, Some(AiProvider::Openai));
        assert_eq!(request.mode, "include-images");
        assert_eq!(request.config_path, "devimg.toml");
        assert_eq!(request.manifest_path, "public/images/devimg-manifest.json");
        assert_eq!(request.output_path.as_deref(), Some("devimg-alt.json"));
        assert_eq!(request.markdown_path.as_deref(), Some("/tmp/devimg-alt.md"));
        assert_eq!(request.sources.len(), 1);
        assert_eq!(request.sources[0].variants.len(), 2);
        assert_eq!(
            request.sources[0].representative_image_path,
            "assets/images/sample.png"
        );
        assert!(request.sources[0].image_bytes_included);
        assert_eq!(request.selected_images.len(), 1);
        assert_eq!(request.selected_images[0].mime_type, "image/png");
        assert_eq!(request.skipped_image_count, 0);
    }

    #[test]
    fn ai_alt_placeholder_json_and_markdown_are_timestamp_free() {
        let manifest = sample_manifest();
        let request = build_ai_alt_request(
            &manifest,
            &AiAltOptions {
                provider: Some(AiProvider::Openai),
                model: Some("dry-run-model".to_string()),
                command: "devimg alt".to_string(),
                config_path: PathBuf::from("devimg.toml"),
                manifest_path: PathBuf::from("public/images/devimg-manifest.json"),
                project_root: PathBuf::from("."),
                dry_run: true,
                include_images: false,
                image_detail: "low".to_string(),
                max_images: 8,
                output_path: None,
                markdown_path: None,
            },
        );
        let report = build_ai_alt_placeholder_report(&request);
        let json = ai_alt_report_to_json(&report);
        let markdown = render_ai_alt_markdown(&report);

        assert!(json.contains("\"provider\": \"openai\""));
        assert!(json.contains("\"provider_called\": false"));
        assert!(json.contains("\"candidate_alt_text\": \"\""));
        assert!(!json.contains("generated_at"));
        assert!(!json.contains("timestamp"));
        assert!(markdown.contains("# DevImg Alt-Text Drafts"));
        assert!(markdown.contains("Provider called: `false`"));
        assert!(markdown.contains("_No draft generated in metadata-only mode._"));
    }

    #[test]
    fn mock_ai_alt_provider_returns_stable_payload_and_failure() {
        let manifest = sample_manifest();
        let request = build_ai_alt_request(
            &manifest,
            &AiAltOptions {
                provider: Some(AiProvider::Anthropic),
                model: Some("mock-model".to_string()),
                command: "devimg alt".to_string(),
                config_path: PathBuf::from("devimg.toml"),
                manifest_path: PathBuf::from("public/images/devimg-manifest.json"),
                project_root: PathBuf::from("."),
                dry_run: false,
                include_images: false,
                image_detail: "low".to_string(),
                max_images: 8,
                output_path: None,
                markdown_path: None,
            },
        );

        let payload = MockAiAltProviderClient::new(AiProvider::Anthropic)
            .alt(&request)
            .expect("mock provider succeeds");
        let report = build_ai_alt_report(&request, payload, true);

        assert!(report.provider_called);
        assert_eq!(report.provider_summary, "mock anthropic alt text");
        assert_eq!(report.drafts.len(), 1);
        assert_eq!(report.drafts[0].confidence, "medium");
        assert_eq!(report.drafts[0].warnings, vec!["needs-human-review"]);

        let error = MockAiAltProviderClient::failing(AiProvider::Openai)
            .alt(&request)
            .expect_err("mock provider fails");
        assert!(error.to_string().contains("mock AI alt provider failure"));
    }

    #[test]
    fn ai_draft_placeholder_markdown_is_timestamp_free_and_review_marked() {
        let request = sample_draft_request(AiDraftType::ProjectPageCopy, None, None, true);
        let report = build_ai_draft_placeholder_report(&request);
        let markdown = render_ai_draft_markdown(&report);
        let json = serde_json::to_string_pretty(&report).expect("draft report serializes");

        assert_eq!(report.draft_type, AiDraftType::ProjectPageCopy);
        assert_eq!(report.mode, "metadata-only");
        assert!(!report.provider_called);
        assert_eq!(report.summary.source_count, 1);
        assert_eq!(report.summary.output_count, 2);
        assert!(markdown.starts_with("# Draft; review before publishing"));
        assert!(markdown.contains("- Draft type: `project-page-copy`"));
        assert!(markdown.contains("- Provider called: `false`"));
        assert!(markdown.contains("## Draft"));
        assert!(!markdown.contains("generated_at"));
        assert!(!json.contains("timestamp"));
    }

    #[test]
    fn mock_ai_draft_provider_returns_stable_payload_and_failure() {
        let request = sample_draft_request(
            AiDraftType::ReleaseNotes,
            Some(AiProvider::Anthropic),
            Some("mock-model".to_string()),
            false,
        );

        let payload = MockAiDraftProviderClient::new(AiProvider::Anthropic)
            .draft(&request)
            .expect("mock provider succeeds");
        let report = build_ai_draft_report(&request, payload, true);

        assert!(report.provider_called);
        assert_eq!(report.provider_summary, "mock anthropic draft");
        assert_eq!(report.sections.len(), 2);
        assert_eq!(report.sections[0].heading, "Release Notes");

        let error = MockAiDraftProviderClient::failing(AiProvider::Openai)
            .draft(&request)
            .expect_err("mock provider fails");
        assert!(error.to_string().contains("mock AI draft provider failure"));
    }

    #[test]
    fn ai_draft_provider_payload_is_normalized_before_markdown() {
        let request = sample_draft_request(
            AiDraftType::BlogOutline,
            Some(AiProvider::Openai),
            Some("draft-model".to_string()),
            false,
        );
        let report = build_ai_draft_report(
            &request,
            super::AiDraftProviderPayload {
                summary: "summary with data:image/png;base64,abc".to_string(),
                sections: vec![AiDraftSection {
                    heading: String::new(),
                    body: "body\nOPENAI_API_KEY=test-secret\nok".to_string(),
                    bullets: vec!["ANTHROPIC_API_KEY=test-secret".to_string()],
                }],
            },
            true,
        );
        let markdown = render_ai_draft_markdown(&report);

        assert_eq!(
            report.provider_summary,
            "[redacted sensitive provider or image data]"
        );
        assert_eq!(report.sections[0].heading, "Blog Outline");
        assert!(!markdown.contains("test-secret"));
        assert!(!markdown.contains("data:image"));
        assert!(markdown.contains("[redacted sensitive provider or image data]"));
    }

    #[test]
    fn ai_image_mime_type_matches_openai_supported_inputs() {
        assert_eq!(ai_image_mime_type("png"), Some("image/png"));
        assert_eq!(ai_image_mime_type("jpeg"), Some("image/jpeg"));
        assert_eq!(ai_image_mime_type("jpg"), Some("image/jpeg"));
        assert_eq!(ai_image_mime_type("webp"), Some("image/webp"));
        assert_eq!(ai_image_mime_type("gif"), Some("image/gif"));
        assert_eq!(ai_image_mime_type("avif"), None);
    }

    fn sample_manifest() -> Manifest {
        Manifest {
            version: 1,
            generated_at: "unix:123".to_string(),
            config_path: "devimg.toml".to_string(),
            config_hash: "blake3:config".to_string(),
            outputs: vec![
                ManifestOutput {
                    source_path: "assets/images/sample.png".to_string(),
                    source_hash: "blake3:source".to_string(),
                    source_width: 100,
                    source_height: 80,
                    source_bytes: 1000,
                    output_path: "public/images/generated/sample.project-card.64.webp".to_string(),
                    preset: "project-card".to_string(),
                    fit: "cover".to_string(),
                    width: 64,
                    height: 64,
                    format: "webp".to_string(),
                    bytes: 700,
                    hash: "blake3:webp".to_string(),
                    operation_hash: "blake3:operation-webp".to_string(),
                },
                ManifestOutput {
                    source_path: "assets/images/sample.png".to_string(),
                    source_hash: "blake3:source".to_string(),
                    source_width: 100,
                    source_height: 80,
                    source_bytes: 1000,
                    output_path: "public/images/generated/sample.project-card.64.avif".to_string(),
                    preset: "project-card".to_string(),
                    fit: "cover".to_string(),
                    width: 64,
                    height: 64,
                    format: "avif".to_string(),
                    bytes: 600,
                    hash: "blake3:avif".to_string(),
                    operation_hash: "blake3:operation-avif".to_string(),
                },
            ],
        }
    }

    fn sample_draft_request(
        draft_type: AiDraftType,
        provider: Option<AiProvider>,
        model: Option<String>,
        dry_run: bool,
    ) -> AiDraftRequest {
        AiDraftRequest {
            schema_version: 1,
            draft_type,
            provider,
            model,
            command: "devimg draft".to_string(),
            config_path: "devimg.toml".to_string(),
            project_root: ".".to_string(),
            mode: if provider.is_some() {
                "provider-draft".to_string()
            } else {
                "metadata-only".to_string()
            },
            dry_run,
            output_path: "devimg-draft.md".to_string(),
            manifest_summary: AiDraftManifestSummary {
                path: "public/images/devimg-manifest.json".to_string(),
                readable: true,
                read_error: None,
                config_hash: Some("blake3:config".to_string()),
                source_count: 1,
                output_count: 2,
                source_bytes: 1000,
                output_bytes: 1300,
                outputs: Vec::new(),
            },
            report_summary: AiDraftArtifactSummary {
                label: "devimg-report".to_string(),
                path: "devimg-report.md".to_string(),
                readable: true,
                read_error: None,
                bytes: 120,
                line_count: 4,
                excerpt: "Dev Image Pipeline Report".to_string(),
            },
            changelog_summary: None,
            optional_artifacts: Vec::new(),
        }
    }
}
