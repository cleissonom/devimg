use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::Serialize;

use crate::manifest::{Manifest, ManifestOutput};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifestCompareOptions {
    pub top_limit: usize,
}

impl Default for ManifestCompareOptions {
    fn default() -> Self {
        Self { top_limit: 5 }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManifestCompare {
    pub base_generated_at: String,
    pub head_generated_at: String,
    pub base_config_hash: String,
    pub head_config_hash: String,
    pub summary: ManifestCompareSummary,
    pub added: Vec<ManifestCompareOutput>,
    pub removed: Vec<ManifestCompareOutput>,
    pub changed: Vec<ManifestCompareChange>,
    pub metadata_changed: Vec<ManifestCompareMetadataChange>,
    pub top_byte_contributors: Vec<ManifestCompareOutput>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManifestCompareSummary {
    pub base_variant_count: u64,
    pub head_variant_count: u64,
    pub variant_count_delta: i64,
    pub base_output_bytes: u64,
    pub head_output_bytes: u64,
    pub output_bytes_delta: i64,
    pub added_count: u64,
    pub removed_count: u64,
    pub changed_count: u64,
    pub metadata_changed_count: u64,
    pub unchanged_count: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManifestCompareOutput {
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

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManifestCompareChange {
    pub source_path: String,
    pub preset: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub base_fit: String,
    pub head_fit: String,
    pub base_output_path: String,
    pub head_output_path: String,
    pub base_bytes: u64,
    pub head_bytes: u64,
    pub byte_delta: i64,
    pub base_hash: String,
    pub head_hash: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ManifestCompareMetadataChange {
    pub source_path: String,
    pub preset: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub output_path: String,
    pub bytes: u64,
    pub hash: String,
    pub base_operation_hash: String,
    pub head_operation_hash: String,
}

pub fn compare_manifests(
    base: &Manifest,
    head: &Manifest,
    options: ManifestCompareOptions,
) -> ManifestCompare {
    let base_outputs = outputs_by_identity(base);
    let mut head_outputs = outputs_by_identity(head);
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let mut metadata_changed = Vec::new();
    let mut unchanged_count = 0u64;

    for (identity, base_output) in base_outputs {
        match head_outputs.remove(&identity) {
            Some(head_output) => {
                if outputs_match(base_output, head_output) {
                    unchanged_count += 1;
                } else if outputs_match_except_operation_hash(base_output, head_output) {
                    metadata_changed.push(ManifestCompareMetadataChange::from_outputs(
                        base_output,
                        head_output,
                    ));
                } else {
                    changed.push(ManifestCompareChange::from_outputs(
                        base_output,
                        head_output,
                    ));
                }
            }
            None => removed.push(ManifestCompareOutput::from_output(base_output)),
        }
    }

    for head_output in head_outputs.into_values() {
        added.push(ManifestCompareOutput::from_output(head_output));
    }

    let base_variant_count = base.outputs.len() as u64;
    let head_variant_count = head.outputs.len() as u64;
    let base_output_bytes = base.output_bytes_total();
    let head_output_bytes = head.output_bytes_total();
    let top_byte_contributors = top_byte_contributors(head, options.top_limit);

    ManifestCompare {
        base_generated_at: base.generated_at.clone(),
        head_generated_at: head.generated_at.clone(),
        base_config_hash: base.config_hash.clone(),
        head_config_hash: head.config_hash.clone(),
        summary: ManifestCompareSummary {
            base_variant_count,
            head_variant_count,
            variant_count_delta: signed_delta(head_variant_count, base_variant_count),
            base_output_bytes,
            head_output_bytes,
            output_bytes_delta: signed_delta(head_output_bytes, base_output_bytes),
            added_count: added.len() as u64,
            removed_count: removed.len() as u64,
            changed_count: changed.len() as u64,
            metadata_changed_count: metadata_changed.len() as u64,
            unchanged_count,
        },
        added,
        removed,
        changed,
        metadata_changed,
        top_byte_contributors,
    }
}

pub fn manifest_compare_to_json(compare: &ManifestCompare) -> String {
    serde_json::to_string_pretty(compare).expect("manifest compare serialization cannot fail")
        + "\n"
}

impl ManifestCompareOutput {
    fn from_output(output: &ManifestOutput) -> Self {
        Self {
            source_path: output.source_path.clone(),
            output_path: output.output_path.clone(),
            preset: output.preset.clone(),
            fit: output.fit.clone(),
            width: output.width,
            height: output.height,
            format: output.format.clone(),
            bytes: output.bytes,
            hash: output.hash.clone(),
        }
    }
}

impl ManifestCompareChange {
    fn from_outputs(base: &ManifestOutput, head: &ManifestOutput) -> Self {
        Self {
            source_path: head.source_path.clone(),
            preset: head.preset.clone(),
            width: head.width,
            height: head.height,
            format: head.format.clone(),
            base_fit: base.fit.clone(),
            head_fit: head.fit.clone(),
            base_output_path: base.output_path.clone(),
            head_output_path: head.output_path.clone(),
            base_bytes: base.bytes,
            head_bytes: head.bytes,
            byte_delta: signed_delta(head.bytes, base.bytes),
            base_hash: base.hash.clone(),
            head_hash: head.hash.clone(),
        }
    }
}

impl ManifestCompareMetadataChange {
    fn from_outputs(base: &ManifestOutput, head: &ManifestOutput) -> Self {
        Self {
            source_path: head.source_path.clone(),
            preset: head.preset.clone(),
            width: head.width,
            height: head.height,
            format: head.format.clone(),
            output_path: head.output_path.clone(),
            bytes: head.bytes,
            hash: head.hash.clone(),
            base_operation_hash: base.operation_hash.clone(),
            head_operation_hash: head.operation_hash.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ManifestOutputIdentity {
    source_path: String,
    preset: String,
    width: u32,
    height: u32,
    format: String,
}

impl ManifestOutputIdentity {
    fn from_output(output: &ManifestOutput) -> Self {
        Self {
            source_path: output.source_path.clone(),
            preset: output.preset.clone(),
            width: output.width,
            height: output.height,
            format: output.format.clone(),
        }
    }
}

fn outputs_by_identity(manifest: &Manifest) -> BTreeMap<ManifestOutputIdentity, &ManifestOutput> {
    manifest
        .outputs
        .iter()
        .map(|output| (ManifestOutputIdentity::from_output(output), output))
        .collect()
}

fn outputs_match(base: &ManifestOutput, head: &ManifestOutput) -> bool {
    outputs_match_except_operation_hash(base, head) && base.operation_hash == head.operation_hash
}

fn outputs_match_except_operation_hash(base: &ManifestOutput, head: &ManifestOutput) -> bool {
    base.source_hash == head.source_hash
        && base.source_width == head.source_width
        && base.source_height == head.source_height
        && base.source_bytes == head.source_bytes
        && base.output_path == head.output_path
        && base.fit == head.fit
        && base.bytes == head.bytes
        && base.hash == head.hash
}

fn top_byte_contributors(manifest: &Manifest, limit: usize) -> Vec<ManifestCompareOutput> {
    let mut outputs = manifest.outputs.iter().collect::<Vec<_>>();
    outputs.sort_by(|a, b| match b.bytes.cmp(&a.bytes) {
        Ordering::Equal => a.output_path.cmp(&b.output_path),
        ordering => ordering,
    });
    outputs
        .into_iter()
        .take(limit)
        .map(ManifestCompareOutput::from_output)
        .collect()
}

fn signed_delta(head: u64, base: u64) -> i64 {
    if head >= base {
        head.saturating_sub(base).min(i64::MAX as u64) as i64
    } else {
        -(base.saturating_sub(head).min(i64::MAX as u64) as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::{compare_manifests, manifest_compare_to_json, ManifestCompareOptions};
    use crate::manifest::{Manifest, ManifestOutput};

    #[test]
    fn compare_manifest_records_added_removed_changed_and_unchanged_outputs() {
        let base = Manifest {
            version: 1,
            generated_at: "unix:1".to_string(),
            config_path: "devimg.toml".to_string(),
            config_hash: "blake3:base".to_string(),
            outputs: vec![
                output(
                    "assets/card.png",
                    "project-card",
                    640,
                    360,
                    "webp",
                    100,
                    "same",
                ),
                output(
                    "assets/card.png",
                    "project-card",
                    960,
                    540,
                    "webp",
                    200,
                    "old",
                ),
                output(
                    "assets/card.png",
                    "project-card",
                    1280,
                    720,
                    "webp",
                    300,
                    "removed",
                ),
            ],
        };
        let mut changed = output(
            "assets/card.png",
            "project-card",
            960,
            540,
            "webp",
            260,
            "new",
        );
        changed.output_path =
            "public/images/generated/card.project-card.960.newhash.webp".to_string();
        changed.operation_hash = "blake3:new-operation".to_string();
        let head = Manifest {
            version: 1,
            generated_at: "unix:2".to_string(),
            config_path: "devimg.toml".to_string(),
            config_hash: "blake3:head".to_string(),
            outputs: vec![
                output(
                    "assets/card.png",
                    "project-card",
                    640,
                    360,
                    "webp",
                    100,
                    "same",
                ),
                changed,
                output("assets/avatar.png", "avatar", 256, 256, "jpeg", 50, "added"),
            ],
        };

        let compare = compare_manifests(&base, &head, ManifestCompareOptions { top_limit: 2 });

        assert_eq!(compare.summary.base_variant_count, 3);
        assert_eq!(compare.summary.head_variant_count, 3);
        assert_eq!(compare.summary.variant_count_delta, 0);
        assert_eq!(compare.summary.base_output_bytes, 600);
        assert_eq!(compare.summary.head_output_bytes, 410);
        assert_eq!(compare.summary.output_bytes_delta, -190);
        assert_eq!(compare.summary.added_count, 1);
        assert_eq!(compare.summary.removed_count, 1);
        assert_eq!(compare.summary.changed_count, 1);
        assert_eq!(compare.summary.metadata_changed_count, 0);
        assert_eq!(compare.summary.unchanged_count, 1);
        assert_eq!(
            compare.added[0].output_path,
            "public/images/generated/avatar.avatar.256.jpeg"
        );
        assert_eq!(compare.removed[0].hash, "blake3:removed");
        assert_eq!(compare.changed[0].base_bytes, 200);
        assert_eq!(compare.changed[0].head_bytes, 260);
        assert_eq!(compare.changed[0].byte_delta, 60);
        assert_eq!(
            compare.changed[0].base_output_path,
            "public/images/generated/card.project-card.960.webp"
        );
        assert_eq!(
            compare.changed[0].head_output_path,
            "public/images/generated/card.project-card.960.newhash.webp"
        );
        assert_eq!(compare.top_byte_contributors.len(), 2);
        assert_eq!(compare.top_byte_contributors[0].bytes, 260);
        assert_eq!(compare.top_byte_contributors[1].bytes, 100);
    }

    #[test]
    fn compare_manifest_separates_metadata_only_operation_changes() {
        let base = Manifest {
            version: 1,
            generated_at: "unix:1".to_string(),
            config_path: "devimg.toml".to_string(),
            config_hash: "blake3:base".to_string(),
            outputs: vec![output(
                "assets/card.png",
                "project-card",
                640,
                360,
                "webp",
                100,
                "same",
            )],
        };
        let mut head = base.clone();
        head.generated_at = "unix:2".to_string();
        head.config_hash = "blake3:head".to_string();
        head.outputs[0].operation_hash = "blake3:operation-new-metadata".to_string();

        let compare = compare_manifests(&base, &head, ManifestCompareOptions::default());

        assert_eq!(compare.summary.changed_count, 0);
        assert_eq!(compare.summary.metadata_changed_count, 1);
        assert_eq!(compare.summary.unchanged_count, 0);
        assert_eq!(
            compare.metadata_changed[0].output_path,
            base.outputs[0].output_path
        );
        assert_eq!(compare.metadata_changed[0].bytes, 100);
        assert_eq!(compare.metadata_changed[0].hash, "blake3:same");
    }

    #[test]
    fn compare_manifest_json_is_machine_readable() {
        let manifest = Manifest {
            version: 1,
            generated_at: "unix:1".to_string(),
            config_path: "devimg.toml".to_string(),
            config_hash: "blake3:config".to_string(),
            outputs: vec![output(
                "assets/card.png",
                "project-card",
                640,
                360,
                "webp",
                100,
                "same",
            )],
        };

        let compare = compare_manifests(&manifest, &manifest, ManifestCompareOptions::default());
        let json = manifest_compare_to_json(&compare);

        assert!(json.contains("\"unchanged_count\": 1"));
        assert!(json.contains("\"metadata_changed_count\": 0"));
        assert!(json.contains("\"top_byte_contributors\""));
    }

    fn output(
        source_path: &str,
        preset: &str,
        width: u32,
        height: u32,
        format: &str,
        bytes: u64,
        hash_suffix: &str,
    ) -> ManifestOutput {
        let stem = source_path
            .rsplit('/')
            .next()
            .expect("source filename")
            .split('.')
            .next()
            .expect("source stem");
        ManifestOutput {
            source_path: source_path.to_string(),
            source_hash: format!("blake3:source-{stem}"),
            source_width: 1600,
            source_height: 900,
            source_bytes: 1000,
            output_path: format!("public/images/generated/{stem}.{preset}.{width}.{format}"),
            preset: preset.to_string(),
            fit: "cover".to_string(),
            width,
            height,
            format: format.to_string(),
            bytes,
            hash: format!("blake3:{hash_suffix}"),
            operation_hash: format!("blake3:operation-{hash_suffix}"),
        }
    }
}
