use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{project_relative, resolve_project_path_checked, Config};
use crate::manifest::read_manifest;
use crate::pipeline::path_to_string;
use crate::{scan_sources, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

fn display_path(path: &Path) -> String {
    let rendered = path_to_string(path);
    if rendered.is_empty() {
        ".".to_string()
    } else {
        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ai_consent_preview_to_json, AiConsentPreview, AiGeneratedOutput, AiProvider,
        AiProviderClient, AiSelectedFile, MockAiProviderClient,
    };

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
}
