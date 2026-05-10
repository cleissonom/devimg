use crate::config::{Config, WarningAcknowledgementConfig};

pub(crate) const QUALITY_LOW_LOSSY: &str = "quality:low-lossy-quality";
pub(crate) const QUALITY_SKIPPED_UPSCALE: &str = "quality:skipped-upscale";
pub(crate) const QUALITY_ALLOWED_UPSCALE: &str = "quality:allowed-upscale";
pub(crate) const QUALITY_COVER_CROP: &str = "quality:cover-crop";
pub(crate) const QUALITY_GENERATED_UPSCALE: &str = "quality:generated-upscale";
pub(crate) const QUALITY_OUTPUT_LARGER_THAN_SOURCE: &str = "quality:output-larger-than-source";
pub(crate) const PLAN_METADATA_NOT_PRESERVED: &str = "plan:metadata-not-preserved";
pub(crate) const PLAN_NO_VARIANTS: &str = "plan:no-variants";
pub(crate) const BUDGET_FAILED: &str = "budget:failed";

#[derive(Debug, Clone, Default)]
pub(crate) struct WarningGroups {
    pub active: Vec<String>,
    pub acknowledged: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WarningInfo {
    pub code: String,
    pub source: Option<String>,
    pub output: Option<String>,
    pub preset: Option<String>,
    pub width: Option<u32>,
}

pub(crate) fn warning_message(code: &str, message: impl Into<String>) -> String {
    format!("{code}: {}", message.into())
}

pub(crate) fn split_acknowledged_warnings(config: &Config, warnings: Vec<String>) -> WarningGroups {
    let mut groups = WarningGroups::default();
    for warning in warnings {
        let info = warning_info(&warning);
        if is_acknowledgeable(&info)
            && config
                .warnings
                .acknowledgements
                .iter()
                .any(|acknowledgement| acknowledgement_matches(acknowledgement, &info))
        {
            groups.acknowledged.push(warning);
        } else {
            groups.active.push(warning);
        }
    }
    groups
}

pub(crate) fn warning_info(warning: &str) -> WarningInfo {
    let (code, message) = split_code_and_message(warning);
    let mut info = WarningInfo {
        code: code.to_string(),
        source: None,
        output: None,
        preset: None,
        width: None,
    };

    match code {
        QUALITY_LOW_LOSSY | QUALITY_ALLOWED_UPSCALE | QUALITY_COVER_CROP => {
            apply_source_preset_width(message, &mut info);
        }
        QUALITY_SKIPPED_UPSCALE => {
            apply_source_preset_width(
                message.strip_prefix("skipped ").unwrap_or(message),
                &mut info,
            );
        }
        QUALITY_GENERATED_UPSCALE | QUALITY_OUTPUT_LARGER_THAN_SOURCE => {
            if let Some((output, _)) = message.split_once(" is ") {
                info.output = Some(normalize_warning_path(output));
            }
        }
        PLAN_NO_VARIANTS => {
            if let Some(source) = message
                .strip_prefix("no variants planned for ")
                .and_then(|rest| rest.split_once(" after ").map(|(source, _)| source))
            {
                info.source = Some(normalize_warning_path(source));
            }
        }
        _ => {}
    }

    info
}

pub(crate) fn split_code_and_message(warning: &str) -> (&str, &str) {
    let Some(first_colon) = warning.find(':') else {
        return ("warning", warning);
    };
    let rest = &warning[first_colon + 1..];
    let Some(second_colon) = rest.find(':') else {
        return (warning[..first_colon].trim(), rest.trim());
    };
    let code_end = first_colon + 1 + second_colon;
    (warning[..code_end].trim(), warning[code_end + 1..].trim())
}

fn apply_source_preset_width(message: &str, info: &mut WarningInfo) {
    let Some((source, rest)) = message.split_once(" preset ") else {
        return;
    };
    info.source = Some(normalize_warning_path(source));

    let preset_markers = [" width ", " uses ", " allows "];
    let preset_end = preset_markers
        .iter()
        .filter_map(|marker| rest.find(marker))
        .min()
        .unwrap_or(rest.len());
    let preset = rest[..preset_end].trim();
    if !preset.is_empty() {
        info.preset = Some(preset.to_string());
    }

    if let Some(width_start) = rest.find(" width ") {
        let raw_width = &rest[width_start + " width ".len()..];
        let width_end = raw_width
            .find(|ch: char| !ch.is_ascii_digit())
            .unwrap_or(raw_width.len());
        info.width = raw_width[..width_end].parse::<u32>().ok();
    }
}

fn acknowledgement_matches(
    acknowledgement: &WarningAcknowledgementConfig,
    info: &WarningInfo,
) -> bool {
    acknowledgement.code == info.code
        && acknowledgement
            .source
            .as_ref()
            .is_none_or(|source| info.source.as_ref() == Some(source))
        && acknowledgement
            .output
            .as_ref()
            .is_none_or(|output| info.output.as_ref() == Some(output))
        && acknowledgement
            .preset
            .as_ref()
            .is_none_or(|preset| info.preset.as_ref() == Some(preset))
        && acknowledgement
            .width
            .is_none_or(|width| info.width == Some(width))
}

fn is_acknowledgeable(info: &WarningInfo) -> bool {
    info.code.starts_with("quality:") || info.code.starts_with("plan:")
}

pub(crate) fn normalize_warning_path(path: &str) -> String {
    let normalized = path
        .trim()
        .replace([std::path::MAIN_SEPARATOR, '\\'], "/")
        .trim_start_matches("./")
        .to_string();
    if normalized.is_empty() {
        ".".to_string()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::{warning_info, QUALITY_COVER_CROP, QUALITY_OUTPUT_LARGER_THAN_SOURCE};

    #[test]
    fn parses_cover_crop_warning_metadata() {
        let info = warning_info(
            "quality:cover-crop: assets/images/accesstrace.png preset project-card width 1200 uses cover and crops about 44% of the resized image",
        );

        assert_eq!(info.code, QUALITY_COVER_CROP);
        assert_eq!(
            info.source.as_deref(),
            Some("assets/images/accesstrace.png")
        );
        assert_eq!(info.preset.as_deref(), Some("project-card"));
        assert_eq!(info.width, Some(1200));
    }

    #[test]
    fn parses_manifest_output_warning_metadata() {
        let info = warning_info(
            "quality:output-larger-than-source: public/images/card.webp is larger than its source file (20 KB vs 10 KB)",
        );

        assert_eq!(info.code, QUALITY_OUTPUT_LARGER_THAN_SOURCE);
        assert_eq!(info.output.as_deref(), Some("public/images/card.webp"));
    }
}
