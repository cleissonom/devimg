use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::{
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    imageops::FilterType,
    DynamicImage, ExtendedColorType, ImageEncoder,
};
use ravif::{Encoder as AvifEncoder, Img as AvifImage, RGBA8};

use crate::config::{CropPosition, FitMode, FormatKind};
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
        operation.crop,
        operation.width,
        operation.height,
    );
    let encoded = encode_image(&processed, operation.format, operation.quality)
        .map_err(|message| DevimgError::image(&operation.output_path, message))?;
    let encoded_hash = hash_bytes(&encoded);
    let output_path = final_output_path(&operation.output_path, &encoded_hash, operation)?;
    let output_project_path =
        final_output_project_path(&operation.output_project_path, &encoded_hash, operation)?;

    if output_path.exists() {
        let existing_hash = hash_file(&output_path)?;
        if existing_hash == encoded_hash {
            let metadata = fs::metadata(&output_path)
                .map_err(|source| DevimgError::io(&output_path, source))?;
            return Ok(manifest_output(
                operation,
                output_project_path,
                metadata.len(),
                encoded_hash,
            ));
        }
        if !allow_overwrite {
            return Err(DevimgError::UnsafeOverwrite { path: output_path });
        }
    }

    safe_write(&output_path, &encoded, allow_overwrite)?;
    Ok(manifest_output(
        operation,
        output_project_path,
        encoded.len() as u64,
        encoded_hash,
    ))
}

fn manifest_output(
    operation: &Operation,
    output_project_path: String,
    bytes: u64,
    hash: String,
) -> ManifestOutput {
    ManifestOutput {
        source_path: operation.source.project_path.clone(),
        source_hash: operation.source.hash.clone(),
        source_width: operation.source.width,
        source_height: operation.source.height,
        source_bytes: operation.source.bytes,
        output_path: output_project_path,
        preset: operation.preset.clone(),
        fit: operation.fit.label().to_string(),
        width: operation.width,
        height: operation.height,
        format: operation.format.label().to_string(),
        bytes,
        hash,
        operation_hash: operation.operation_hash.clone(),
    }
}

pub(crate) fn final_output_project_path(
    output_project_path: &str,
    hash: &str,
    operation: &Operation,
) -> Result<String> {
    if operation.content_hash_filenames {
        add_hash_to_output_project_path(output_project_path, hash)
    } else {
        Ok(output_project_path.to_string())
    }
}

fn final_output_path(path: &Path, hash: &str, operation: &Operation) -> Result<PathBuf> {
    if operation.content_hash_filenames {
        add_hash_to_output_path(path, hash)
    } else {
        Ok(path.to_path_buf())
    }
}

fn add_hash_to_output_project_path(output_project_path: &str, hash: &str) -> Result<String> {
    let separator = output_project_path
        .rfind('/')
        .map(|index| index + 1)
        .unwrap_or(0);
    let (parent, file_name) = output_project_path.split_at(separator);
    let (stem, extension) = file_name.rsplit_once('.').ok_or_else(|| {
        DevimgError::image(
            output_project_path,
            "could not derive extension for hashed output filename",
        )
    })?;
    Ok(format!(
        "{parent}{stem}.{}.{}",
        hash_fragment(hash),
        extension
    ))
}

fn add_hash_to_output_path(path: &Path, hash: &str) -> Result<PathBuf> {
    let stem = path
        .file_stem()
        .ok_or_else(|| DevimgError::image(path, "could not derive file stem"))?
        .to_string_lossy();
    let extension = path
        .extension()
        .ok_or_else(|| DevimgError::image(path, "could not derive file extension"))?
        .to_string_lossy();
    let file_name = format!("{stem}.{}.{}", hash_fragment(hash), extension);
    Ok(path.with_file_name(file_name))
}

fn hash_fragment(hash: &str) -> &str {
    let raw = hash.strip_prefix("blake3:").unwrap_or(hash);
    &raw[..raw.len().min(12)]
}

fn transform_image(
    image: DynamicImage,
    fit: FitMode,
    crop: CropPosition,
    width: u32,
    height: u32,
) -> DynamicImage {
    match fit {
        FitMode::Fill => image.resize_exact(width, height, FilterType::Lanczos3),
        FitMode::Contain => image.resize(width, height, FilterType::Lanczos3),
        FitMode::Cover => {
            let (resize_width, resize_height) =
                crate::plan::cover_dimensions(image.width(), image.height(), width, height);
            let resized = image.resize_exact(resize_width, resize_height, FilterType::Lanczos3);
            let x = crop_offset(resize_width.saturating_sub(width), crop.x);
            let y = crop_offset(resize_height.saturating_sub(height), crop.y);
            resized.crop_imm(x, y, width, height)
        }
    }
}

fn crop_offset(overflow: u32, position: f32) -> u32 {
    ((overflow as f32) * position).round() as u32
}

fn encode_image(
    image: &DynamicImage,
    format: FormatKind,
    quality: u8,
) -> std::result::Result<Vec<u8>, String> {
    let mut cursor = Cursor::new(Vec::new());
    match format {
        FormatKind::Jpeg => {
            let rgb = image.to_rgb8();
            JpegEncoder::new_with_quality(&mut cursor, quality)
                .write_image(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    ExtendedColorType::Rgb8,
                )
                .map_err(|source| source.to_string())?;
        }
        FormatKind::Png => {
            let rgba = image.to_rgba8();
            PngEncoder::new(&mut cursor)
                .write_image(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    ExtendedColorType::Rgba8,
                )
                .map_err(|source| source.to_string())?;
        }
        FormatKind::Webp => return encode_webp(image, quality),
        FormatKind::Avif => return encode_avif(image, quality),
    }
    Ok(cursor.into_inner())
}

fn encode_webp(image: &DynamicImage, quality: u8) -> std::result::Result<Vec<u8>, String> {
    let rgba = image.to_rgba8();
    let encoder = webp::Encoder::from_rgba(&rgba, rgba.width(), rgba.height());
    let encoded = encoder.encode(f32::from(quality));
    if encoded.is_empty() {
        return Err("WebP encoder returned empty output".to_string());
    }
    Ok(encoded.to_vec())
}

fn encode_avif(image: &DynamicImage, quality: u8) -> std::result::Result<Vec<u8>, String> {
    let rgba = image.to_rgba8();
    let pixels = rgba
        .pixels()
        .map(|pixel| {
            let [r, g, b, a] = pixel.0;
            RGBA8 { r, g, b, a }
        })
        .collect::<Vec<_>>();
    let encoded = AvifEncoder::new()
        .with_quality(f32::from(quality.max(1)))
        .with_speed(8)
        .encode_rgba(AvifImage::new(
            &pixels,
            rgba.width() as usize,
            rgba.height() as usize,
        ))
        .map_err(|source| source.to_string())?;
    if encoded.avif_file.is_empty() {
        return Err("AVIF encoder returned empty output".to_string());
    }
    Ok(encoded.avif_file)
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

#[cfg(test)]
mod tests {
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

    use crate::config::{CropPosition, FitMode, FormatKind};

    use super::{encode_image, transform_image};

    #[test]
    fn webp_quality_changes_encoded_size() {
        let image = detailed_image(128, 128);

        let low = encode_image(&image, FormatKind::Webp, 10).expect("low quality webp encodes");
        let high = encode_image(&image, FormatKind::Webp, 90).expect("high quality webp encodes");

        assert_ne!(low, high);
        assert!(
            high.len() > low.len() + (low.len() / 5),
            "expected high-quality WebP to be materially larger, low={} high={}",
            low.len(),
            high.len()
        );

        let decoded =
            image::load_from_memory_with_format(&high, ImageFormat::WebP).expect("webp decodes");
        assert_eq!((decoded.width(), decoded.height()), (128, 128));
    }

    #[test]
    fn jpeg_quality_changes_encoded_size_and_preserves_dimensions() {
        let image = detailed_image(128, 128);

        let low = encode_image(&image, FormatKind::Jpeg, 10).expect("low quality jpeg encodes");
        let high = encode_image(&image, FormatKind::Jpeg, 90).expect("high quality jpeg encodes");

        assert_ne!(low, high);
        assert!(
            high.len() > low.len() + (low.len() / 5),
            "expected high-quality JPEG to be materially larger, low={} high={}",
            low.len(),
            high.len()
        );

        let decoded =
            image::load_from_memory_with_format(&high, ImageFormat::Jpeg).expect("jpeg decodes");
        assert_eq!((decoded.width(), decoded.height()), (128, 128));
    }

    #[test]
    fn png_quality_is_ignored_and_preserves_dimensions() {
        let image = detailed_image(64, 64);

        let low = encode_image(&image, FormatKind::Png, 10).expect("low quality png encodes");
        let high = encode_image(&image, FormatKind::Png, 90).expect("high quality png encodes");

        assert_eq!(low, high);

        let decoded =
            image::load_from_memory_with_format(&high, ImageFormat::Png).expect("png decodes");
        assert_eq!((decoded.width(), decoded.height()), (64, 64));
    }

    #[test]
    fn avif_quality_changes_encoded_size_and_writes_avif_bytes() {
        let image = detailed_image(64, 64);

        let low = encode_image(&image, FormatKind::Avif, 1).expect("low quality avif encodes");
        let high = encode_image(&image, FormatKind::Avif, 90).expect("high quality avif encodes");

        assert_ne!(low, high);
        assert!(
            high.len() > low.len(),
            "expected high-quality AVIF to be larger, low={} high={}",
            low.len(),
            high.len()
        );
        assert_avif_container(&high);
    }

    #[test]
    fn cover_crop_centers_wide_images() {
        let mut source = RgbaImage::new(6, 2);
        for (x, _y, pixel) in source.enumerate_pixels_mut() {
            *pixel = match x {
                0 | 1 => Rgba([255, 0, 0, 255]),
                2 | 3 => Rgba([0, 255, 0, 255]),
                _ => Rgba([0, 0, 255, 255]),
            };
        }

        let cropped = transform_image(
            DynamicImage::ImageRgba8(source),
            FitMode::Cover,
            CropPosition::CENTER,
            2,
            2,
        );

        assert_eq!((cropped.width(), cropped.height()), (2, 2));
        let pixels = cropped.to_rgba8();
        assert!(pixels.pixels().all(|pixel| pixel.0 == [0, 255, 0, 255]));
    }

    #[test]
    fn cover_crop_honors_vertical_anchors() {
        let source = vertical_stripes();

        let top = transform_image(
            DynamicImage::ImageRgba8(source.clone()),
            FitMode::Cover,
            CropPosition { x: 0.5, y: 0.0 },
            2,
            2,
        )
        .to_rgba8();
        let bottom = transform_image(
            DynamicImage::ImageRgba8(source),
            FitMode::Cover,
            CropPosition { x: 0.5, y: 1.0 },
            2,
            2,
        )
        .to_rgba8();

        assert!(top.pixels().all(|pixel| pixel.0 == [255, 0, 0, 255]));
        assert!(bottom.pixels().all(|pixel| pixel.0 == [0, 0, 255, 255]));
    }

    #[test]
    fn cover_crop_honors_horizontal_anchors() {
        let source = horizontal_stripes();

        let left = transform_image(
            DynamicImage::ImageRgba8(source.clone()),
            FitMode::Cover,
            CropPosition { x: 0.0, y: 0.5 },
            2,
            2,
        )
        .to_rgba8();
        let right = transform_image(
            DynamicImage::ImageRgba8(source),
            FitMode::Cover,
            CropPosition { x: 1.0, y: 0.5 },
            2,
            2,
        )
        .to_rgba8();

        assert!(left.pixels().all(|pixel| pixel.0 == [255, 0, 0, 255]));
        assert!(right.pixels().all(|pixel| pixel.0 == [0, 0, 255, 255]));
    }

    #[test]
    fn cover_crop_honors_custom_normalized_position() {
        let source = vertical_stripes();

        let cropped = transform_image(
            DynamicImage::ImageRgba8(source),
            FitMode::Cover,
            CropPosition { x: 0.5, y: 0.25 },
            2,
            2,
        )
        .to_rgba8();

        assert_eq!(cropped.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(cropped.get_pixel(0, 1).0, [0, 255, 0, 255]);
    }

    fn horizontal_stripes() -> RgbaImage {
        let mut source = RgbaImage::new(6, 2);
        for (x, _y, pixel) in source.enumerate_pixels_mut() {
            *pixel = match x {
                0 | 1 => Rgba([255, 0, 0, 255]),
                2 | 3 => Rgba([0, 255, 0, 255]),
                _ => Rgba([0, 0, 255, 255]),
            };
        }
        source
    }

    fn vertical_stripes() -> RgbaImage {
        let mut source = RgbaImage::new(2, 6);
        for (_x, y, pixel) in source.enumerate_pixels_mut() {
            *pixel = match y {
                0 | 1 => Rgba([255, 0, 0, 255]),
                2 | 3 => Rgba([0, 255, 0, 255]),
                _ => Rgba([0, 0, 255, 255]),
            };
        }
        source
    }

    fn detailed_image(width: u32, height: u32) -> DynamicImage {
        let mut image = RgbaImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let mixed = x.wrapping_mul(31) ^ y.wrapping_mul(17);
            *pixel = Rgba([
                ((x * 3 + y * 5 + mixed) % 256) as u8,
                ((x * 11 + y * 7) % 256) as u8,
                ((x * y + mixed * 13) % 256) as u8,
                255,
            ]);
        }
        DynamicImage::ImageRgba8(image)
    }

    fn assert_avif_container(bytes: &[u8]) {
        assert!(bytes.len() > 16, "AVIF output should not be empty");
        assert_eq!(&bytes[4..8], b"ftyp");
        assert!(
            bytes.windows(4).any(|window| window == b"avif"),
            "AVIF output should contain the avif brand"
        );
    }
}
