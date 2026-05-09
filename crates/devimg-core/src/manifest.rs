use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{DevimgError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "unknown_generated_at")]
    pub generated_at: String,
    pub config_path: String,
    pub config_hash: String,
    pub outputs: Vec<ManifestOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestOutput {
    pub source_path: String,
    pub source_hash: String,
    pub source_width: u32,
    pub source_height: u32,
    pub source_bytes: u64,
    pub output_path: String,
    pub preset: String,
    #[serde(default = "unknown_fit")]
    pub fit: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub bytes: u64,
    pub hash: String,
    pub operation_hash: String,
}

impl Manifest {
    pub fn new(config_path: String, config_hash: String, outputs: Vec<ManifestOutput>) -> Self {
        Self {
            version: default_version(),
            generated_at: generated_at(),
            config_path,
            config_hash,
            outputs,
        }
    }

    pub fn source_bytes_total(&self) -> u64 {
        let mut seen = Vec::<(&str, u64)>::new();
        for output in &self.outputs {
            if !seen.iter().any(|(path, _)| *path == output.source_path) {
                seen.push((&output.source_path, output.source_bytes));
            }
        }
        seen.iter().map(|(_, bytes)| *bytes).sum()
    }

    pub fn output_bytes_total(&self) -> u64 {
        self.outputs.iter().map(|output| output.bytes).sum()
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManifestExport {
    pub version: u32,
    pub generated_at: String,
    pub config_hash: String,
    pub sources: Vec<ManifestExportSource>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManifestExportSource {
    pub source_path: String,
    pub source_hash: String,
    pub source_width: u32,
    pub source_height: u32,
    pub source_bytes: u64,
    pub variants: Vec<ManifestExportVariant>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManifestExportVariant {
    pub src: String,
    pub output_path: String,
    pub preset: String,
    pub fit: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub bytes: u64,
    pub hash: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestExportOptions {
    pub strip_prefix: Option<String>,
    pub url_prefix: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManifestTypescriptOptions {
    pub include_helpers: bool,
}

pub fn export_manifest(manifest: &Manifest, options: &ManifestExportOptions) -> ManifestExport {
    let mut sources = BTreeMap::<String, ManifestExportSource>::new();
    for output in &manifest.outputs {
        let source = sources
            .entry(output.source_path.clone())
            .or_insert_with(|| ManifestExportSource {
                source_path: output.source_path.clone(),
                source_hash: output.source_hash.clone(),
                source_width: output.source_width,
                source_height: output.source_height,
                source_bytes: output.source_bytes,
                variants: Vec::new(),
            });
        source.variants.push(ManifestExportVariant {
            src: manifest_variant_src(&output.output_path, options),
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

    ManifestExport {
        version: manifest.version,
        generated_at: manifest.generated_at.clone(),
        config_hash: manifest.config_hash.clone(),
        sources: sources.into_values().collect(),
    }
}

pub fn manifest_export_to_json(manifest: &Manifest, options: &ManifestExportOptions) -> String {
    let document = export_manifest(manifest, options);
    serde_json::to_string_pretty(&document).expect("manifest export serialization cannot fail")
        + "\n"
}

pub fn manifest_export_to_typescript(
    manifest: &Manifest,
    options: &ManifestExportOptions,
) -> String {
    manifest_export_to_typescript_with_options(
        manifest,
        options,
        &ManifestTypescriptOptions::default(),
    )
}

pub fn manifest_export_to_typescript_with_options(
    manifest: &Manifest,
    options: &ManifestExportOptions,
    typescript_options: &ManifestTypescriptOptions,
) -> String {
    export_to_typescript(&export_manifest(manifest, options), typescript_options)
}

pub fn write_manifest(path: &Path, manifest: &Manifest) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| DevimgError::io(parent, source))?;
    }
    fs::write(path, manifest_to_json(manifest)).map_err(|source| DevimgError::io(path, source))
}

pub fn read_manifest(path: &Path) -> Result<Manifest> {
    let raw = fs::read_to_string(path).map_err(|source| DevimgError::io(path, source))?;
    serde_json::from_str(&raw)
        .map_err(|source| DevimgError::config(path, format!("invalid manifest JSON: {source}")))
}

pub fn manifest_to_json(manifest: &Manifest) -> String {
    let document = ManifestDocument {
        version: manifest.version,
        generated_at: &manifest.generated_at,
        config_path: &manifest.config_path,
        config_hash: &manifest.config_hash,
        outputs: &manifest.outputs,
        totals: ManifestTotals {
            source_bytes: manifest.source_bytes_total(),
            output_bytes: manifest.output_bytes_total(),
        },
    };
    serde_json::to_string_pretty(&document).expect("manifest serialization cannot fail") + "\n"
}

#[derive(Serialize)]
struct ManifestDocument<'a> {
    version: u32,
    generated_at: &'a str,
    config_path: &'a str,
    config_hash: &'a str,
    outputs: &'a [ManifestOutput],
    totals: ManifestTotals,
}

#[derive(Serialize)]
struct ManifestTotals {
    source_bytes: u64,
    output_bytes: u64,
}

fn default_version() -> u32 {
    1
}

fn unknown_generated_at() -> String {
    "unknown".to_string()
}

fn unknown_fit() -> String {
    "unknown".to_string()
}

fn generated_at() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{seconds}")
}

fn manifest_variant_src(output_path: &str, options: &ManifestExportOptions) -> String {
    let path = output_path.replace('\\', "/");
    let stripped = options
        .strip_prefix
        .as_deref()
        .and_then(|prefix| strip_path_prefix(&path, prefix))
        .unwrap_or(path);
    join_url_prefix(&options.url_prefix, &stripped)
}

fn strip_path_prefix(path: &str, prefix: &str) -> Option<String> {
    let prefix = prefix.replace('\\', "/");
    let prefix = prefix.trim_matches('/');
    if prefix.is_empty() {
        return Some(path.trim_start_matches('/').to_string());
    }
    if path == prefix {
        return Some(String::new());
    }
    path.strip_prefix(&format!("{prefix}/"))
        .map(ToString::to_string)
}

fn join_url_prefix(url_prefix: &str, path: &str) -> String {
    let prefix = url_prefix.replace('\\', "/");
    let prefix = prefix.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    if prefix.is_empty() {
        if url_prefix.starts_with('/') {
            format!("/{path}")
        } else {
            path.to_string()
        }
    } else if path.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}/{path}")
    }
}

fn export_to_typescript(
    export: &ManifestExport,
    typescript_options: &ManifestTypescriptOptions,
) -> String {
    let mut out = String::new();
    out.push_str("// Generated by devimg. Do not edit by hand.\n");
    out.push_str("export const DEVIMG_MANIFEST = {\n");
    push_ts_number_field(&mut out, 2, "version", u64::from(export.version), true);
    push_ts_string_field(&mut out, 2, "generated_at", &export.generated_at, true);
    push_ts_string_field(&mut out, 2, "config_hash", &export.config_hash, true);
    out.push_str("  sources: [\n");
    for (source_index, source) in export.sources.iter().enumerate() {
        out.push_str("    {\n");
        push_ts_string_field(&mut out, 6, "source_path", &source.source_path, true);
        push_ts_string_field(&mut out, 6, "source_hash", &source.source_hash, true);
        push_ts_number_field(
            &mut out,
            6,
            "source_width",
            u64::from(source.source_width),
            true,
        );
        push_ts_number_field(
            &mut out,
            6,
            "source_height",
            u64::from(source.source_height),
            true,
        );
        push_ts_number_field(&mut out, 6, "source_bytes", source.source_bytes, true);
        out.push_str("      variants: [\n");
        for (variant_index, variant) in source.variants.iter().enumerate() {
            out.push_str("        {\n");
            push_ts_string_field(&mut out, 10, "src", &variant.src, true);
            push_ts_string_field(&mut out, 10, "output_path", &variant.output_path, true);
            push_ts_string_field(&mut out, 10, "preset", &variant.preset, true);
            push_ts_string_field(&mut out, 10, "fit", &variant.fit, true);
            push_ts_number_field(&mut out, 10, "width", u64::from(variant.width), true);
            push_ts_number_field(&mut out, 10, "height", u64::from(variant.height), true);
            push_ts_string_field(&mut out, 10, "format", &variant.format, true);
            push_ts_number_field(&mut out, 10, "bytes", variant.bytes, true);
            push_ts_string_field(&mut out, 10, "hash", &variant.hash, false);
            out.push_str("        }");
            if variant_index + 1 != source.variants.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("      ]\n");
        out.push_str("    }");
        if source_index + 1 != export.sources.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n");
    out.push_str("} as const\n");
    if typescript_options.include_helpers {
        out.push_str(
            r#"
export type DevimgManifest = typeof DEVIMG_MANIFEST
export type DevimgSource = DevimgManifest["sources"][number]
export type DevimgVariant = DevimgSource["variants"][number]

export type DevimgVariantSelector = {
  source: string
  preset?: string
  format?: string
  width?: number
  minWidth?: number
}

export function findDevimgSource(sourcePath: string): DevimgSource | undefined {
  return DEVIMG_MANIFEST.sources.find((source) => source.source_path === sourcePath)
}

export function listDevimgVariants(sourcePath: string): readonly DevimgVariant[] {
  return findDevimgSource(sourcePath)?.variants ?? []
}

export function findDevimgVariant(selector: DevimgVariantSelector): DevimgVariant | undefined {
  let best: DevimgVariant | undefined
  for (const variant of listDevimgVariants(selector.source)) {
    if (selector.preset !== undefined && variant.preset !== selector.preset) {
      continue
    }
    if (selector.format !== undefined && variant.format !== selector.format) {
      continue
    }
    if (selector.width !== undefined && variant.width !== selector.width) {
      continue
    }
    if (selector.minWidth !== undefined && variant.width < selector.minWidth) {
      continue
    }
    if (best === undefined || variant.width < best.width) {
      best = variant
    }
  }
  if (best !== undefined) {
    return best
  }
  if (selector.minWidth === undefined || selector.width !== undefined) {
    return undefined
  }
  for (const variant of listDevimgVariants(selector.source)) {
    if (selector.preset !== undefined && variant.preset !== selector.preset) {
      continue
    }
    if (selector.format !== undefined && variant.format !== selector.format) {
      continue
    }
    if (best === undefined || variant.width > best.width) {
      best = variant
    }
  }
  return best
}
"#,
        );
    }
    out
}

fn push_ts_string_field(out: &mut String, indent: usize, key: &str, value: &str, comma: bool) {
    let value = serde_json::to_string(value).expect("string serialization cannot fail");
    push_ts_field(out, indent, key, &value, comma);
}

fn push_ts_number_field(out: &mut String, indent: usize, key: &str, value: u64, comma: bool) {
    push_ts_field(out, indent, key, &value.to_string(), comma);
}

fn push_ts_field(out: &mut String, indent: usize, key: &str, value: &str, comma: bool) {
    let prefix = " ".repeat(indent);
    let suffix = if comma { "," } else { "" };
    let line = format!("{prefix}{key}: {value}{suffix}");
    if key == "output_path" && line.len() > 100 {
        out.push_str(&format!("{prefix}{key}:\n{prefix}  {value}{suffix}\n"));
    } else {
        out.push_str(&line);
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::{
        export_manifest, manifest_export_to_json, manifest_export_to_typescript,
        manifest_export_to_typescript_with_options, manifest_to_json, Manifest,
        ManifestExportOptions, ManifestOutput, ManifestTypescriptOptions,
    };

    #[test]
    fn manifest_json_round_trips_and_ignores_totals() {
        let manifest = Manifest {
            version: 1,
            generated_at: "unix:1".to_string(),
            config_path: "dev\"img.toml".to_string(),
            config_hash: "blake3:abc".to_string(),
            outputs: vec![ManifestOutput {
                source_path: "assets/images/card.png".to_string(),
                source_hash: "blake3:source".to_string(),
                source_width: 800,
                source_height: 450,
                source_bytes: 123,
                output_path: "public/images/card.webp".to_string(),
                preset: "project-card".to_string(),
                fit: "cover".to_string(),
                width: 640,
                height: 360,
                format: "webp".to_string(),
                bytes: 45,
                hash: "blake3:output".to_string(),
                operation_hash: "blake3:operation".to_string(),
            }],
        };

        let json = manifest_to_json(&manifest);
        assert!(json.contains("\"totals\""));
        let parsed: Manifest = serde_json::from_str(&json).expect("manifest parses");

        assert_eq!(parsed.config_path, "dev\"img.toml");
        assert_eq!(parsed.outputs[0].source_path, "assets/images/card.png");
        assert_eq!(parsed.outputs[0].fit, "cover");
        assert_eq!(parsed.source_bytes_total(), 123);
        assert_eq!(parsed.output_bytes_total(), 45);
    }

    #[test]
    fn older_manifest_outputs_without_fit_still_parse() {
        let raw = r#"{
  "version": 1,
  "generated_at": "unix:1",
  "config_path": "devimg.toml",
  "config_hash": "blake3:config",
  "outputs": [
    {
      "source_path": "assets/images/card.png",
      "source_hash": "blake3:source",
      "source_width": 800,
      "source_height": 450,
      "source_bytes": 123,
      "output_path": "public/images/card.webp",
      "preset": "project-card",
      "width": 640,
      "height": 360,
      "format": "webp",
      "bytes": 45,
      "hash": "blake3:output",
      "operation_hash": "blake3:operation"
    }
  ]
}"#;

        let parsed: Manifest = serde_json::from_str(raw).expect("old manifest parses");

        assert_eq!(parsed.outputs[0].fit, "unknown");
    }

    #[test]
    fn older_manifest_exports_with_defaulted_fields() {
        let raw = r#"{
  "config_path": "devimg.toml",
  "config_hash": "blake3:config",
  "outputs": [
    {
      "source_path": "assets/images/card.png",
      "source_hash": "blake3:source",
      "source_width": 800,
      "source_height": 450,
      "source_bytes": 123,
      "output_path": "public/images/card.webp",
      "preset": "project-card",
      "width": 640,
      "height": 360,
      "format": "webp",
      "bytes": 45,
      "hash": "blake3:output",
      "operation_hash": "blake3:operation"
    }
  ]
}"#;
        let parsed: Manifest = serde_json::from_str(raw).expect("older manifest parses");
        let options = ManifestExportOptions {
            strip_prefix: Some("public".to_string()),
            url_prefix: "/".to_string(),
        };

        let exported = export_manifest(&parsed, &options);
        let json = manifest_export_to_json(&parsed, &options);

        assert_eq!(exported.version, 1);
        assert_eq!(exported.generated_at, "unknown");
        assert_eq!(exported.sources[0].variants[0].fit, "unknown");
        assert_eq!(exported.sources[0].variants[0].src, "/images/card.webp");
        assert!(json.contains("\"generated_at\": \"unknown\""));
        assert!(json.contains("\"fit\": \"unknown\""));
    }

    #[test]
    fn manifest_export_groups_sources_and_derives_urls() {
        let manifest = sample_manifest();
        let options = ManifestExportOptions {
            strip_prefix: Some("public".to_string()),
            url_prefix: "/".to_string(),
        };

        let exported = export_manifest(&manifest, &options);

        assert_eq!(exported.sources.len(), 1);
        assert_eq!(exported.sources[0].source_path, "assets/images/card.png");
        assert_eq!(exported.sources[0].variants.len(), 2);
        assert_eq!(exported.sources[0].variants[0].preset, "project-card");
        assert_eq!(exported.sources[0].variants[0].fit, "cover");
        assert_eq!(
            exported.sources[0].variants[0].src,
            "/images/generated/card.project-card.640.webp"
        );
    }

    #[test]
    fn manifest_export_renderers_are_app_consumable() {
        let manifest = sample_manifest();
        let options = ManifestExportOptions {
            strip_prefix: Some("public".to_string()),
            url_prefix: "/assets".to_string(),
        };

        let json = manifest_export_to_json(&manifest, &options);
        let typescript = manifest_export_to_typescript(&manifest, &options);

        assert!(json.contains("\"sources\""));
        assert!(json.contains("\"src\": \"/assets/images/generated/card.project-card.640.webp\""));
        assert!(typescript.starts_with("// Generated by devimg."));
        assert!(typescript.contains("export const DEVIMG_MANIFEST = {"));
        assert!(typescript.contains("src: \"/assets/images/generated/card.project-card.640.webp\""));
        assert!(typescript.ends_with(" as const\n"));
    }

    #[test]
    fn typescript_export_can_include_lookup_helpers() {
        let manifest = sample_manifest();
        let options = ManifestExportOptions {
            strip_prefix: Some("public".to_string()),
            url_prefix: "/".to_string(),
        };

        let default_typescript = manifest_export_to_typescript(&manifest, &options);
        let helper_typescript = manifest_export_to_typescript_with_options(
            &manifest,
            &options,
            &ManifestTypescriptOptions {
                include_helpers: true,
            },
        );

        assert!(!default_typescript.contains("findDevimgVariant"));
        assert!(helper_typescript.contains("export type DevimgVariantSelector = {"));
        assert!(helper_typescript.contains("export function findDevimgSource"));
        assert!(helper_typescript.contains("export function listDevimgVariants"));
        assert!(helper_typescript.contains("export function findDevimgVariant"));
        assert!(helper_typescript.contains("variant.width < selector.minWidth"));
    }

    fn sample_manifest() -> Manifest {
        Manifest {
            version: 1,
            generated_at: "unix:1".to_string(),
            config_path: "devimg.toml".to_string(),
            config_hash: "blake3:config".to_string(),
            outputs: vec![
                ManifestOutput {
                    source_path: "assets/images/card.png".to_string(),
                    source_hash: "blake3:source".to_string(),
                    source_width: 800,
                    source_height: 450,
                    source_bytes: 123,
                    output_path: "public/images/generated/card.project-card.640.webp".to_string(),
                    preset: "project-card".to_string(),
                    fit: "cover".to_string(),
                    width: 640,
                    height: 360,
                    format: "webp".to_string(),
                    bytes: 45,
                    hash: "blake3:output-webp".to_string(),
                    operation_hash: "blake3:operation-webp".to_string(),
                },
                ManifestOutput {
                    source_path: "assets/images/card.png".to_string(),
                    source_hash: "blake3:source".to_string(),
                    source_width: 800,
                    source_height: 450,
                    source_bytes: 123,
                    output_path: "public/images/generated/card.project-card.640.jpeg".to_string(),
                    preset: "project-card".to_string(),
                    fit: "cover".to_string(),
                    width: 640,
                    height: 360,
                    format: "jpeg".to_string(),
                    bytes: 67,
                    hash: "blake3:output-jpeg".to_string(),
                    operation_hash: "blake3:operation-jpeg".to_string(),
                },
            ],
        }
    }
}
