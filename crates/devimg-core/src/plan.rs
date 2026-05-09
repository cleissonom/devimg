use std::path::{Path, PathBuf};

use globset::GlobSet;

use crate::config::{
    project_relative, AspectRatio, Config, FitMode, FormatKind, PresetConfig, PresetOverrideConfig,
};
use crate::hash::hash_bytes;
use crate::pipeline::{path_to_string, Operation, Plan, SourceImage};
use crate::quality::{
    allowed_upscale_warning, append_unique, cover_crop_warning, lossy_quality_warnings,
    skipped_upscale_warning,
};
use crate::scan::compile_globs;
use crate::{DevimgError, Result};

pub fn build_plan(config: &Config, sources: &[SourceImage]) -> Result<Plan> {
    let mut operations = Vec::new();
    let mut warnings = Vec::new();
    let overrides = compile_overrides(config)?;
    if !config.project.strip_metadata {
        warnings.push(
            "strip_metadata=false was requested, but MVP encoders re-encode images and do not preserve source metadata"
                .to_string(),
        );
    }

    for source in sources {
        let mut planned_for_source = 0usize;
        for preset in &config.presets {
            let effective_preset = apply_overrides(source, preset, &overrides);
            for width in &effective_preset.widths {
                let (target_width, target_height) =
                    target_dimensions(source.width, source.height, *width, &effective_preset);
                if !effective_preset.allow_upscale
                    && (target_width > source.width || target_height > source.height)
                {
                    append_unique(
                        &mut warnings,
                        [skipped_upscale_warning(
                            source,
                            &effective_preset,
                            *width,
                            target_width,
                            target_height,
                        )],
                    );
                    continue;
                }
                append_unique(
                    &mut warnings,
                    lossy_quality_warnings(source, &effective_preset),
                );
                append_unique(
                    &mut warnings,
                    allowed_upscale_warning(
                        source,
                        &effective_preset,
                        *width,
                        target_width,
                        target_height,
                    ),
                );
                append_unique(
                    &mut warnings,
                    cover_crop_warning(
                        source,
                        &effective_preset,
                        *width,
                        target_width,
                        target_height,
                    ),
                );
                for format in &effective_preset.formats {
                    let output_path = output_path_for(source, &effective_preset, *width, *format)?;
                    if output_path == source.path {
                        return Err(DevimgError::UnsafeOverwrite { path: output_path });
                    }
                    let output_project_path =
                        path_to_string(&project_relative(config, &output_path));
                    let op_hash = operation_hash(
                        config,
                        source,
                        &effective_preset,
                        *format,
                        target_width,
                        target_height,
                        &output_project_path,
                    );
                    operations.push(Operation {
                        source: source.clone(),
                        preset: effective_preset.name.clone(),
                        fit: effective_preset.fit,
                        crop: effective_preset.crop,
                        quality: effective_preset.quality,
                        format: *format,
                        width: target_width,
                        height: target_height,
                        content_hash_filenames: config.project.content_hash_filenames,
                        output_path,
                        output_project_path,
                        operation_hash: op_hash,
                    });
                    planned_for_source += 1;
                }
            }
        }
        if planned_for_source == 0 {
            warnings.push(format!(
                "no variants planned for {} after applying presets",
                source.project_path
            ));
        }
    }

    operations.sort_by(|left, right| left.output_project_path.cmp(&right.output_project_path));
    Ok(Plan {
        operations,
        warnings,
    })
}

fn output_path_for(
    source: &SourceImage,
    preset: &PresetConfig,
    width: u32,
    format: FormatKind,
) -> Result<PathBuf> {
    let stem = source
        .relative_path
        .file_stem()
        .ok_or_else(|| DevimgError::image(&source.path, "could not derive file stem"))?
        .to_string_lossy();
    let parent = source
        .relative_path
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let file_name = format!("{stem}.{}.{}.{}", preset.name, width, format.extension());
    Ok(source.output_root.join(parent).join(file_name))
}

fn target_dimensions(
    source_width: u32,
    source_height: u32,
    requested_width: u32,
    preset: &PresetConfig,
) -> (u32, u32) {
    let box_width = requested_width.max(1);
    let box_height = match preset.aspect_ratio {
        Some(AspectRatio { width, height }) => {
            ((u64::from(box_width) * u64::from(height) + u64::from(width / 2)) / u64::from(width))
                .max(1) as u32
        }
        None => ((u64::from(box_width) * u64::from(source_height) + u64::from(source_width / 2))
            / u64::from(source_width))
        .max(1) as u32,
    };

    if preset.fit != FitMode::Contain {
        return (box_width, box_height);
    }

    if u64::from(source_width) * u64::from(box_height)
        >= u64::from(source_height) * u64::from(box_width)
    {
        let height = ((u64::from(box_width) * u64::from(source_height)
            + u64::from(source_width / 2))
            / u64::from(source_width))
        .max(1) as u32;
        (box_width, height)
    } else {
        let width = ((u64::from(box_height) * u64::from(source_width)
            + u64::from(source_height / 2))
            / u64::from(source_height))
        .max(1) as u32;
        (width, box_height)
    }
}

fn operation_hash(
    config: &Config,
    source: &SourceImage,
    preset: &PresetConfig,
    format: FormatKind,
    width: u32,
    height: u32,
    output_path: &str,
) -> String {
    let mut input = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        config.config_hash,
        source.hash,
        source.project_path,
        output_path,
        preset.name,
        preset.fit.label(),
        preset.crop.label(),
        preset.quality,
        width,
        height,
        format.label(),
        config.project.strip_metadata,
    );
    if let Some(component) = encoder_hash_component(format) {
        input.push('|');
        input.push_str(component);
    }
    if config.project.content_hash_filenames {
        input.push_str("|filename:content-hash-v1");
    }
    hash_bytes(input.as_bytes())
}

fn encoder_hash_component(format: FormatKind) -> Option<&'static str> {
    match format {
        FormatKind::Webp => Some("encoder:webp-libwebp-lossy-v1"),
        FormatKind::Avif => Some("encoder:ravif-0.13-speed8-v1"),
        FormatKind::Png | FormatKind::Jpeg => None,
    }
}

struct CompiledPresetOverride<'a> {
    config: &'a PresetOverrideConfig,
    includes: GlobSet,
    excludes: GlobSet,
}

fn compile_overrides(config: &Config) -> Result<Vec<CompiledPresetOverride<'_>>> {
    config
        .overrides
        .iter()
        .map(|preset_override| {
            Ok(CompiledPresetOverride {
                config: preset_override,
                includes: compile_globs(
                    &config.path,
                    "override include",
                    &preset_override.include,
                )?,
                excludes: compile_globs(
                    &config.path,
                    "override exclude",
                    &preset_override.exclude,
                )?,
            })
        })
        .collect()
}

fn apply_overrides(
    source: &SourceImage,
    preset: &PresetConfig,
    overrides: &[CompiledPresetOverride<'_>],
) -> PresetConfig {
    let mut effective = preset.clone();
    let relative_path = path_to_string(&source.relative_path);
    for preset_override in overrides {
        if !override_matches(preset_override, &relative_path, &preset.name) {
            continue;
        }
        if let Some(quality) = preset_override.config.quality {
            effective.quality = quality;
        }
        if let Some(fit) = preset_override.config.fit {
            effective.fit = fit;
        }
        if let Some(crop) = preset_override.config.crop {
            effective.crop = crop;
        }
        if let Some(allow_upscale) = preset_override.config.allow_upscale {
            effective.allow_upscale = allow_upscale;
        }
    }
    effective
}

fn override_matches(
    preset_override: &CompiledPresetOverride<'_>,
    relative_path: &str,
    preset_name: &str,
) -> bool {
    if !preset_override.config.presets.is_empty()
        && !preset_override
            .config
            .presets
            .iter()
            .any(|candidate| candidate == preset_name)
    {
        return false;
    }
    if !preset_override.config.include.is_empty()
        && !preset_override.includes.is_match(relative_path)
    {
        return false;
    }
    !preset_override.excludes.is_match(relative_path)
}

pub(crate) fn cover_dimensions(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> (u32, u32) {
    let sw = u128::from(source_width);
    let sh = u128::from(source_height);
    let tw = u128::from(target_width);
    let th = u128::from(target_height);

    if sw * th >= sh * tw {
        let resize_width = (sw * th).div_ceil(sh) as u32;
        (resize_width, target_height)
    } else {
        let resize_height = (sh * tw).div_ceil(sw) as u32;
        (target_width, resize_height)
    }
}
