use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::check::validate_output_file;
use crate::config::{resolve_project_path_checked, Config};
use crate::hash::hash_file;
use crate::manifest::{read_manifest, Manifest, ManifestOutput};
use crate::pipeline::Operation;
use crate::plan::legacy_operation_hash;
use crate::transform::final_output_project_path;
use crate::Result;

pub(crate) enum IncrementalLookup {
    Current(Box<ManifestOutput>),
    Stale,
}

pub(crate) enum IncrementalCache {
    Current(CurrentManifestCache),
}

impl IncrementalCache {
    pub(crate) fn read(_config: &Config, manifest_path: &Path) -> Option<Self> {
        let manifest = read_manifest(manifest_path).ok()?;
        Some(Self::Current(CurrentManifestCache::new(manifest)))
    }

    pub(crate) fn lookup_current(
        &self,
        config: &Config,
        operation: &Operation,
    ) -> Result<IncrementalLookup> {
        match self {
            Self::Current(cache) => cache.lookup_current(config, operation),
        }
    }
}

pub(crate) struct CurrentManifestCache {
    manifest: Manifest,
    by_output_path: HashMap<String, usize>,
    by_operation_hash: HashMap<String, Vec<usize>>,
}

impl CurrentManifestCache {
    fn new(manifest: Manifest) -> Self {
        let mut by_output_path = HashMap::new();
        let mut by_operation_hash = HashMap::<String, Vec<usize>>::new();
        for (index, output) in manifest.outputs.iter().enumerate() {
            by_output_path.insert(output.output_path.clone(), index);
            by_operation_hash
                .entry(output.operation_hash.clone())
                .or_default()
                .push(index);
        }
        Self {
            manifest,
            by_output_path,
            by_operation_hash,
        }
    }

    fn lookup_current(&self, config: &Config, operation: &Operation) -> Result<IncrementalLookup> {
        let Some(output) = self.manifest_output(operation)? else {
            return Ok(IncrementalLookup::Stale);
        };
        match current_output(config, operation, output)? {
            Some(output) => Ok(IncrementalLookup::Current(Box::new(output))),
            None => Ok(IncrementalLookup::Stale),
        }
    }

    fn manifest_output(&self, operation: &Operation) -> Result<Option<&ManifestOutput>> {
        if operation.content_hash_filenames {
            return self.hashed_manifest_output(operation);
        }

        let Some(index) = self.by_output_path.get(&operation.output_project_path) else {
            return Ok(None);
        };
        let output = &self.manifest.outputs[*index];
        if !operation_hash_matches(output, operation, &self.manifest.config_hash) {
            return Ok(None);
        }
        Ok(Some(output))
    }

    fn hashed_manifest_output(&self, operation: &Operation) -> Result<Option<&ManifestOutput>> {
        let legacy_hash = legacy_operation_hash(operation, &self.manifest.config_hash);
        let Some(indexes) = self
            .by_operation_hash
            .get(&operation.operation_hash)
            .or_else(|| self.by_operation_hash.get(&legacy_hash))
        else {
            return Ok(None);
        };
        if indexes.len() != 1 {
            return Ok(None);
        }

        let output = &self.manifest.outputs[indexes[0]];
        let expected_path =
            final_output_project_path(&operation.output_project_path, &output.hash, operation)?;
        if output.output_path != expected_path {
            return Ok(None);
        }
        Ok(Some(output))
    }
}

fn current_output(
    config: &Config,
    operation: &Operation,
    output: &ManifestOutput,
) -> Result<Option<ManifestOutput>> {
    let output_path = resolve_project_path_checked(
        config,
        &PathBuf::from(&output.output_path),
        "manifest output path",
    )?;
    if !output_path.exists() {
        return Ok(None);
    }

    let actual_hash = hash_file(&output_path)?;
    if actual_hash != output.hash {
        return Ok(None);
    }

    if validate_output_file(operation, &output_path, &output.output_path).is_some() {
        return Ok(None);
    }

    let metadata = fs::metadata(&output_path)
        .map_err(|source| crate::DevimgError::io(&output_path, source))?;
    let mut current = output.clone();
    current.bytes = metadata.len();
    current.hash = actual_hash;
    current.operation_hash = operation.operation_hash.clone();
    Ok(Some(current))
}

fn operation_hash_matches(
    output: &ManifestOutput,
    operation: &Operation,
    manifest_config_hash: &str,
) -> bool {
    output.operation_hash == operation.operation_hash
        || output.operation_hash == legacy_operation_hash(operation, manifest_config_hash)
}
