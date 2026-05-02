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

fn generated_at() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{seconds}")
}

#[cfg(test)]
mod tests {
    use super::{manifest_to_json, Manifest, ManifestOutput};

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
        assert_eq!(parsed.source_bytes_total(), 123);
        assert_eq!(parsed.output_bytes_total(), 45);
    }
}
