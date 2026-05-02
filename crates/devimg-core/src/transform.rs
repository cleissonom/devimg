use std::fs;
use std::io::Cursor;
use std::path::Path;

use image::{imageops::FilterType, DynamicImage};

use crate::config::{FitMode, FormatKind};
use crate::hash::{hash_bytes, hash_file};
use crate::manifest::ManifestOutput;
use crate::pipeline::Operation;
use crate::{DevimgError, Result};

pub(crate) fn execute_operation(
    operation: &Operation,
    allow_overwrite: bool,
) -> Result<ManifestOutput> {
    let source_image = image::open(&operation.source.path).map_err(|source| {
        DevimgError::image(
            &operation.source.path,
            format!("could not decode image: {source}"),
        )
    })?;
    let processed = transform_image(
        source_image,
        operation.fit,
        operation.width,
        operation.height,
    );
    let encoded = encode_image(&processed, operation.format, operation.quality)
        .map_err(|message| DevimgError::image(&operation.output_path, message))?;
    let encoded_hash = hash_bytes(&encoded);

    if operation.output_path.exists() {
        let existing_hash = hash_file(&operation.output_path)?;
        if existing_hash == encoded_hash {
            let metadata = fs::metadata(&operation.output_path)
                .map_err(|source| DevimgError::io(&operation.output_path, source))?;
            return Ok(manifest_output(operation, metadata.len(), encoded_hash));
        }
        if !allow_overwrite {
            return Err(DevimgError::UnsafeOverwrite {
                path: operation.output_path.clone(),
            });
        }
    }

    safe_write(&operation.output_path, &encoded, allow_overwrite)?;
    Ok(manifest_output(
        operation,
        encoded.len() as u64,
        encoded_hash,
    ))
}

fn manifest_output(operation: &Operation, bytes: u64, hash: String) -> ManifestOutput {
    ManifestOutput {
        source_path: operation.source.project_path.clone(),
        source_hash: operation.source.hash.clone(),
        source_width: operation.source.width,
        source_height: operation.source.height,
        source_bytes: operation.source.bytes,
        output_path: operation.output_project_path.clone(),
        preset: operation.preset.clone(),
        width: operation.width,
        height: operation.height,
        format: operation.format.label().to_string(),
        bytes,
        hash,
        operation_hash: operation.operation_hash.clone(),
    }
}

fn transform_image(image: DynamicImage, fit: FitMode, width: u32, height: u32) -> DynamicImage {
    match fit {
        FitMode::Fill => image.resize_exact(width, height, FilterType::Lanczos3),
        FitMode::Contain => image.resize(width, height, FilterType::Lanczos3),
        FitMode::Cover => {
            let (resize_width, resize_height) =
                crate::plan::cover_dimensions(image.width(), image.height(), width, height);
            let resized = image.resize_exact(resize_width, resize_height, FilterType::Lanczos3);
            let x = resize_width.saturating_sub(width) / 2;
            let y = resize_height.saturating_sub(height) / 2;
            resized.crop_imm(x, y, width, height)
        }
    }
}

fn encode_image(
    image: &DynamicImage,
    format: FormatKind,
    quality: u8,
) -> std::result::Result<Vec<u8>, String> {
    let mut cursor = Cursor::new(Vec::new());
    match format {
        FormatKind::Jpeg => DynamicImage::ImageRgb8(image.to_rgb8())
            .write_to(&mut cursor, format.output_format(quality))
            .map_err(|source| source.to_string())?,
        FormatKind::Png | FormatKind::Webp => image
            .write_to(&mut cursor, format.output_format(quality))
            .map_err(|source| source.to_string())?,
    }
    Ok(cursor.into_inner())
}

fn safe_write(path: &Path, bytes: &[u8], allow_replace: bool) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| DevimgError::io(parent, source))?;
    }
    let file_name = path.file_name().ok_or_else(|| {
        DevimgError::io(
            path,
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing filename"),
        )
    })?;
    let tmp_name = format!("{}.tmp-{}", file_name.to_string_lossy(), std::process::id());
    let tmp_path = path.with_file_name(tmp_name);
    fs::write(&tmp_path, bytes).map_err(|source| DevimgError::io(&tmp_path, source))?;
    if path.exists() && allow_replace {
        fs::remove_file(path).map_err(|source| DevimgError::io(path, source))?;
    }
    fs::rename(&tmp_path, path).map_err(|source| DevimgError::io(path, source))
}
