use crate::config::{CropPosition, FitMode, FormatKind, PresetConfig};
use crate::manifest::Manifest;
use crate::pipeline::SourceImage;

const DETAIL_SENSITIVE_KEYWORDS: &[&str] = &[
    "banner",
    "card",
    "chart",
    "diagram",
    "hero",
    "logo",
    "og",
    "open-graph",
    "opengraph",
    "screen",
    "screenshot",
    "text",
    "ui",
];

pub(crate) fn append_unique(
    warnings: &mut Vec<String>,
    new_warnings: impl IntoIterator<Item = String>,
) {
    for warning in new_warnings {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }
}

pub(crate) fn lossy_quality_warnings(source: &SourceImage, preset: &PresetConfig) -> Vec<String> {
    let detail_sensitive = is_detail_sensitive_asset(&source.project_path, &preset.name);
    let mut warnings = Vec::new();
    for format in &preset.formats {
        let Some(minimum) = suggested_min_quality(*format, detail_sensitive) else {
            continue;
        };
        if preset.quality >= minimum {
            continue;
        }

        let reason = if detail_sensitive {
            "screenshot/banner/text-heavy assets"
        } else {
            "general lossy output"
        };
        warnings.push(format!(
            "quality: {} preset {} uses {} quality {} below the suggested minimum {} for {}; raise quality or use png when crisp text/graphics matter",
            source.project_path,
            preset.name,
            format.label(),
            preset.quality,
            minimum,
            reason
        ));
    }
    warnings
}

pub(crate) fn skipped_upscale_warning(
    source: &SourceImage,
    preset: &PresetConfig,
    requested_width: u32,
    target_width: u32,
    target_height: u32,
) -> String {
    format!(
        "quality: skipped {} preset {} width {} because it would require upscaling from {}x{} to {}x{}; provide a larger source, reduce widths, or set allow_upscale=true if softness is acceptable",
        source.project_path,
        preset.name,
        requested_width,
        source.width,
        source.height,
        target_width,
        target_height
    )
}

pub(crate) fn allowed_upscale_warning(
    source: &SourceImage,
    preset: &PresetConfig,
    requested_width: u32,
    target_width: u32,
    target_height: u32,
) -> Option<String> {
    if target_width <= source.width && target_height <= source.height {
        return None;
    }

    Some(format!(
        "quality: {} preset {} width {} allows upscaling from {}x{} to {}x{}; generated images may look soft, so prefer a larger source or reduce widths",
        source.project_path,
        preset.name,
        requested_width,
        source.width,
        source.height,
        target_width,
        target_height
    ))
}

pub(crate) fn cover_crop_warning(
    source: &SourceImage,
    preset: &PresetConfig,
    requested_width: u32,
    target_width: u32,
    target_height: u32,
) -> Option<String> {
    if preset.fit != FitMode::Cover || preset.aspect_ratio.is_none() {
        return None;
    }

    let loss = cover_crop_loss(source.width, source.height, target_width, target_height);
    if loss < 0.15 {
        return None;
    }

    let crop_hint = if preset.crop == CropPosition::CENTER {
        "set crop to an anchor/focal point"
    } else {
        "confirm the crop anchor/focal point"
    };
    Some(format!(
        "quality: {} preset {} width {} uses cover and crops about {}% of the resized image; {}, or use fit=contain if the full composition must remain visible",
        source.project_path,
        preset.name,
        requested_width,
        (loss * 100.0).round() as u32,
        crop_hint
    ))
}

pub(crate) fn manifest_quality_warnings(manifest: &Manifest) -> Vec<String> {
    let mut warnings = Vec::new();
    for output in &manifest.outputs {
        if output.width > output.source_width || output.height > output.source_height {
            warnings.push(format!(
                "quality: {} is {}x{} from a {}x{} source; generated images may look soft, so use a larger source or reduce configured widths",
                output.output_path,
                output.width,
                output.height,
                output.source_width,
                output.source_height
            ));
        }
        if output.bytes > output.source_bytes {
            warnings.push(format!(
                "quality: {} is larger than its source file ({} vs {}); this can happen with small optimized sources, high quality settings, format changes, or graphic/transparent assets",
                output.output_path,
                format_bytes(output.bytes),
                format_bytes(output.source_bytes)
            ));
        }
    }
    warnings.sort();
    warnings.dedup();
    warnings
}

fn suggested_min_quality(format: FormatKind, detail_sensitive: bool) -> Option<u8> {
    match format {
        FormatKind::Jpeg | FormatKind::Webp => Some(if detail_sensitive { 82 } else { 70 }),
        FormatKind::Avif => Some(if detail_sensitive { 60 } else { 45 }),
        FormatKind::Png => None,
    }
}

fn is_detail_sensitive_asset(source_path: &str, preset_name: &str) -> bool {
    let haystack = format!(
        "{} {}",
        source_path.to_ascii_lowercase().replace('_', "-"),
        preset_name.to_ascii_lowercase().replace('_', "-")
    );
    DETAIL_SENSITIVE_KEYWORDS
        .iter()
        .any(|keyword| haystack.contains(keyword))
}

fn cover_crop_loss(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> f64 {
    let (resize_width, resize_height) =
        crate::plan::cover_dimensions(source_width, source_height, target_width, target_height);
    let width_loss =
        resize_width.saturating_sub(target_width) as f64 / f64::from(resize_width.max(1));
    let height_loss =
        resize_height.saturating_sub(target_height) as f64 / f64::from(resize_height.max(1));
    width_loss.max(height_loss)
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= MB {
        format!("{:.2} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{} B", bytes as u64)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::config::{AspectRatio, CropPosition, FitMode, FormatKind, PresetConfig};
    use crate::manifest::{Manifest, ManifestOutput};
    use crate::pipeline::SourceImage;

    use super::{
        allowed_upscale_warning, cover_crop_warning, lossy_quality_warnings,
        manifest_quality_warnings, skipped_upscale_warning,
    };

    #[test]
    fn warns_for_low_lossy_quality_on_detail_sensitive_assets() {
        let source = source("assets/images/hero-screenshot.png", 1600, 900, 120_000);
        let preset = preset(
            "hero",
            74,
            vec![FormatKind::Webp, FormatKind::Jpeg, FormatKind::Png],
        );

        let warnings = lossy_quality_warnings(&source, &preset);

        assert_eq!(warnings.len(), 2);
        assert!(warnings[0].contains("webp quality 74"));
        assert!(warnings[0].contains("suggested minimum 82"));
        assert!(warnings[1].contains("jpeg quality 74"));
    }

    #[test]
    fn general_lossy_quality_threshold_is_less_noisy() {
        let source = source("assets/photos/trip.png", 1600, 900, 120_000);
        let preset = preset("thumbnail", 74, vec![FormatKind::Webp, FormatKind::Avif]);

        assert!(lossy_quality_warnings(&source, &preset).is_empty());
    }

    #[test]
    fn warns_for_allowed_and_skipped_upscaling() {
        let source = source("assets/images/small.png", 320, 180, 4_000);
        let mut preset = preset("project-card", 82, vec![FormatKind::Webp]);
        preset.allow_upscale = true;

        let allowed = allowed_upscale_warning(&source, &preset, 640, 640, 360)
            .expect("allowed upscale warns");
        let skipped = skipped_upscale_warning(&source, &preset, 640, 640, 360);

        assert!(allowed.contains("allows upscaling"));
        assert!(allowed.contains("generated images may look soft"));
        assert!(skipped.contains("would require upscaling"));
    }

    #[test]
    fn warns_for_material_cover_crop_loss() {
        let source = source("assets/images/banner.png", 1000, 1000, 40_000);
        let preset = preset("hero", 90, vec![FormatKind::Jpeg]);

        let warning =
            cover_crop_warning(&source, &preset, 1200, 1200, 675).expect("cover crop warns");

        assert!(warning.contains("uses cover"));
        assert!(warning.contains("crops about 44%"));
        assert!(warning.contains("fit=contain"));
    }

    #[test]
    fn manifest_warnings_cover_upscale_and_larger_outputs() {
        let manifest = Manifest {
            version: 1,
            generated_at: "unix:1".to_string(),
            config_path: "devimg.toml".to_string(),
            config_hash: "blake3:config".to_string(),
            outputs: vec![ManifestOutput {
                source_path: "assets/small.png".to_string(),
                source_hash: "blake3:source".to_string(),
                source_width: 320,
                source_height: 180,
                source_bytes: 100,
                output_path: "public/images/small.project-card.640.webp".to_string(),
                preset: "project-card".to_string(),
                fit: "cover".to_string(),
                width: 640,
                height: 360,
                format: "webp".to_string(),
                bytes: 200,
                hash: "blake3:output".to_string(),
                operation_hash: "blake3:operation".to_string(),
            }],
        };

        let warnings = manifest_quality_warnings(&manifest);

        assert_eq!(warnings.len(), 2);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("larger than its source")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("may look soft")));
    }

    fn source(path: &str, width: u32, height: u32, bytes: u64) -> SourceImage {
        SourceImage {
            source_name: "portfolio".to_string(),
            path: PathBuf::from(path),
            project_path: path.to_string(),
            relative_path: PathBuf::from(path)
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_default(),
            output_root: PathBuf::from("public/images/generated"),
            output_root_project_path: "public/images/generated".to_string(),
            width,
            height,
            bytes,
            hash: "blake3:source".to_string(),
            format: FormatKind::Png,
        }
    }

    fn preset(name: &str, quality: u8, formats: Vec<FormatKind>) -> PresetConfig {
        PresetConfig {
            name: name.to_string(),
            widths: vec![1200],
            formats,
            quality,
            fit: FitMode::Cover,
            crop: CropPosition::CENTER,
            aspect_ratio: Some(AspectRatio {
                width: 16,
                height: 9,
            }),
            allow_upscale: false,
        }
    }
}
