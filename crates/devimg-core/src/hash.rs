use std::fs;
use std::path::Path;

use crate::{DevimgError, Result};

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}

pub fn hash_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| DevimgError::io(path, source))?;
    Ok(hash_bytes(&bytes))
}
