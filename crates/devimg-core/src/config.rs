use std::fs;
use std::path::{Component, Path, PathBuf};

use image::{ImageFormat, ImageOutputFormat};
use serde::{Deserialize, Deserializer};

use crate::hash::hash_bytes;
use crate::{DevimgError, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub path: PathBuf,
    pub config_hash: String,
    pub project: ProjectConfig,
    pub sources: Vec<SourceConfig>,
    pub presets: Vec<PresetConfig>,
    pub overrides: Vec<PresetOverrideConfig>,
    pub budgets: BudgetConfig,
}

#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub report: PathBuf,
    pub overwrite: bool,
    pub strip_metadata: bool,
    pub content_hash_filenames: bool,
}

#[derive(Debug, Clone)]
pub struct SourceConfig {
    pub name: String,
    pub input: PathBuf,
    pub output: PathBuf,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PresetConfig {
    pub name: String,
    pub widths: Vec<u32>,
    pub formats: Vec<FormatKind>,
    pub quality: u8,
    pub fit: FitMode,
    pub crop: CropPosition,
    pub aspect_ratio: Option<AspectRatio>,
    pub allow_upscale: bool,
}

#[derive(Debug, Clone)]
pub struct PresetOverrideConfig {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub presets: Vec<String>,
    pub quality: Option<u8>,
    pub fit: Option<FitMode>,
    pub crop: Option<CropPosition>,
    pub allow_upscale: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct BudgetConfig {
    pub max_total_bytes: Option<u64>,
    pub max_file_bytes: Option<u64>,
    pub fail_on_regression: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FitMode {
    Cover,
    Contain,
    Fill,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropPosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormatKind {
    Png,
    Jpeg,
    Webp,
    Avif,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AspectRatio {
    pub width: u32,
    pub height: u32,
}

impl FormatKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str()
        {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::Webp),
            "avif" => Some(Self::Avif),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Webp => "webp",
            Self::Avif => "avif",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Webp => "webp",
            Self::Avif => "avif",
        }
    }

    pub fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Webp => ImageFormat::WebP,
            Self::Avif => ImageFormat::Avif,
        }
    }

    pub fn output_format(self, quality: u8) -> ImageOutputFormat {
        match self {
            Self::Png => ImageOutputFormat::Png,
            Self::Jpeg => ImageOutputFormat::Jpeg(quality),
            Self::Webp => ImageOutputFormat::WebP,
            Self::Avif => unreachable!("AVIF output uses the ravif encoder"),
        }
    }

    pub fn supports_source_input(self) -> bool {
        match self {
            Self::Png | Self::Jpeg | Self::Webp => true,
            Self::Avif => false,
        }
    }
}

impl<'de> Deserialize<'de> for FormatKind {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw)
            .ok_or_else(|| serde::de::Error::custom(format!("unsupported image format `{raw}`")))
    }
}

impl FitMode {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "cover" | "crop" => Some(Self::Cover),
            "contain" => Some(Self::Contain),
            "fill" | "stretch" => Some(Self::Fill),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Contain => "contain",
            Self::Fill => "fill",
        }
    }
}

impl<'de> Deserialize<'de> for FitMode {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw)
            .ok_or_else(|| serde::de::Error::custom(format!("unsupported fit `{raw}`")))
    }
}

impl Default for CropPosition {
    fn default() -> Self {
        Self::CENTER
    }
}

impl CropPosition {
    pub const CENTER: Self = Self { x: 0.5, y: 0.5 };

    pub fn label(self) -> String {
        format!("crop:{:.4}:{:.4}", self.x, self.y)
    }

    fn parse_anchor(raw: &str) -> Option<Self> {
        match normalize_crop_anchor(raw).as_str() {
            "center" => Some(Self::CENTER),
            "top" => Some(Self { x: 0.5, y: 0.0 }),
            "bottom" => Some(Self { x: 0.5, y: 1.0 }),
            "left" => Some(Self { x: 0.0, y: 0.5 }),
            "right" => Some(Self { x: 1.0, y: 0.5 }),
            "top-left" => Some(Self { x: 0.0, y: 0.0 }),
            "top-right" => Some(Self { x: 1.0, y: 0.0 }),
            "bottom-left" => Some(Self { x: 0.0, y: 1.0 }),
            "bottom-right" => Some(Self { x: 1.0, y: 1.0 }),
            _ => None,
        }
    }

    fn new(x: f32, y: f32) -> std::result::Result<Self, String> {
        if !x.is_finite() || !y.is_finite() {
            return Err("crop x and y must be finite numbers".to_string());
        }
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            return Err("crop x and y must be between 0.0 and 1.0".to_string());
        }
        Ok(Self { x, y })
    }
}

fn normalize_crop_anchor(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace(['_', ' '], "-")
}

impl<'de> Deserialize<'de> for CropPosition {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum CropValue {
            Anchor(String),
            Point(CropPoint),
        }

        #[derive(Deserialize)]
        struct CropPoint {
            x: f32,
            y: f32,
        }

        match CropValue::deserialize(deserializer)? {
            CropValue::Anchor(raw) => Self::parse_anchor(&raw)
                .ok_or_else(|| serde::de::Error::custom(format!("unsupported crop `{raw}`"))),
            CropValue::Point(point) => {
                Self::new(point.x, point.y).map_err(serde::de::Error::custom)
            }
        }
    }
}

impl AspectRatio {
    fn parse(raw: &str) -> Option<Self> {
        let (left, right) = raw.trim().split_once(':')?;
        let width = left.trim().parse::<u32>().ok()?;
        let height = right.trim().parse::<u32>().ok()?;
        if width == 0 || height == 0 {
            return None;
        }
        Some(Self { width, height })
    }
}

impl<'de> Deserialize<'de> for AspectRatio {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw)
            .ok_or_else(|| serde::de::Error::custom(format!("invalid aspect_ratio `{raw}`")))
    }
}

pub fn load_config(path: impl AsRef<Path>) -> Result<Config> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            DevimgError::config(path, "config file not found")
        } else {
            DevimgError::io(path, source)
        }
    })?;
    parse_config(path, &raw)
}

pub fn parse_config(path: &Path, raw: &str) -> Result<Config> {
    let value: toml::Value = toml::from_str(raw)
        .map_err(|source| DevimgError::config(path, format!("invalid TOML: {source}")))?;
    reject_unknown_top_level(path, &value)?;
    let parsed: RawConfig = value
        .try_into()
        .map_err(|source| DevimgError::config(path, format!("invalid config: {source}")))?;
    parsed.into_config(path, raw)
}

pub(crate) fn resolve_project_path(config: &Config, path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize_path(path)
    } else {
        normalize_path(&config.project.root.join(path))
    }
}

pub(crate) fn resolve_project_path_checked(
    config: &Config,
    path: &Path,
    label: &str,
) -> Result<PathBuf> {
    let resolved = resolve_project_path(config, path);
    if config.project.root.as_os_str().is_empty() || resolved.starts_with(&config.project.root) {
        Ok(resolved)
    } else {
        Err(DevimgError::config(
            &config.path,
            format!("{label} escapes project root: {}", resolved.display()),
        ))
    }
}

pub(crate) fn project_relative(config: &Config, path: &Path) -> PathBuf {
    let normalized = normalize_path(path);
    normalized
        .strip_prefix(&config.project.root)
        .map(Path::to_path_buf)
        .unwrap_or(normalized)
}

pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
        }
    }
    normalized
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawConfig {
    project: RawProjectConfig,
    #[serde(alias = "source")]
    sources: Vec<RawSourceConfig>,
    #[serde(alias = "presets")]
    preset: Vec<RawPresetConfig>,
    #[serde(alias = "override")]
    overrides: Vec<RawPresetOverrideConfig>,
    budgets: RawBudgetConfig,
}

impl RawConfig {
    fn into_config(self, path: &Path, raw: &str) -> Result<Config> {
        let config_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let root_raw = self.project.root.unwrap_or_else(|| PathBuf::from("."));
        let root = normalize_path(&config_dir.join(root_raw));
        let manifest = self
            .project
            .manifest
            .ok_or_else(|| DevimgError::config(path, "missing [project].manifest"))?;
        let report = self
            .project
            .report
            .unwrap_or_else(|| PathBuf::from("devimg-report.md"));

        let mut sources = Vec::new();
        for source in self.sources {
            sources.push(SourceConfig {
                name: required(source.name, path, "source.name")?,
                input: required(source.input, path, "source.input")?,
                output: required(source.output, path, "source.output")?,
                include: source.include,
                exclude: source.exclude,
            });
        }

        let mut presets = Vec::new();
        for preset in self.preset {
            let widths = required(preset.widths, path, "preset.widths")?;
            if widths.is_empty() {
                return Err(DevimgError::config(path, "preset.widths cannot be empty"));
            }
            let formats = required(preset.formats, path, "preset.formats")?;
            if formats.is_empty() {
                return Err(DevimgError::config(path, "preset.formats cannot be empty"));
            }
            let quality = preset.quality.unwrap_or(82);
            if quality > 100 {
                return Err(DevimgError::config(path, "quality must be 0-100"));
            }
            presets.push(PresetConfig {
                name: required(preset.name, path, "preset.name")?,
                widths,
                formats,
                quality,
                fit: preset.fit.unwrap_or(FitMode::Cover),
                crop: preset.crop.unwrap_or_default(),
                aspect_ratio: preset.aspect_ratio,
                allow_upscale: preset.allow_upscale.unwrap_or(false),
            });
        }

        let mut overrides = Vec::new();
        for preset_override in self.overrides {
            let quality = preset_override.quality;
            if quality.is_some_and(|quality| quality > 100) {
                return Err(DevimgError::config(path, "override quality must be 0-100"));
            }
            overrides.push(PresetOverrideConfig {
                include: preset_override.include,
                exclude: preset_override.exclude,
                presets: preset_override.presets,
                quality,
                fit: preset_override.fit,
                crop: preset_override.crop,
                allow_upscale: preset_override.allow_upscale,
            });
        }

        if sources.is_empty() {
            return Err(DevimgError::config(
                path,
                "at least one [[sources]] entry is required",
            ));
        }
        if presets.is_empty() {
            return Err(DevimgError::config(
                path,
                "at least one [[preset]] entry is required",
            ));
        }

        Ok(Config {
            path: normalize_path(path),
            config_hash: hash_bytes(raw.as_bytes()),
            project: ProjectConfig {
                root,
                manifest,
                report,
                overwrite: self.project.overwrite.unwrap_or(false),
                strip_metadata: self.project.strip_metadata.unwrap_or(true),
                content_hash_filenames: self.project.content_hash_filenames.unwrap_or(false),
            },
            sources,
            presets,
            overrides,
            budgets: BudgetConfig {
                max_total_bytes: self.budgets.max_total_bytes,
                max_file_bytes: self.budgets.max_file_bytes,
                fail_on_regression: self.budgets.fail_on_regression,
            },
        })
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawProjectConfig {
    root: Option<PathBuf>,
    manifest: Option<PathBuf>,
    report: Option<PathBuf>,
    #[serde(alias = "allow_overwrite")]
    overwrite: Option<bool>,
    strip_metadata: Option<bool>,
    #[serde(alias = "hash_filenames", alias = "hashed_filenames")]
    content_hash_filenames: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawSourceConfig {
    name: Option<String>,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    include: Vec<String>,
    exclude: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawPresetConfig {
    name: Option<String>,
    widths: Option<Vec<u32>>,
    formats: Option<Vec<FormatKind>>,
    quality: Option<u8>,
    fit: Option<FitMode>,
    #[serde(alias = "crop_anchor", alias = "crop_position", alias = "focal_point")]
    crop: Option<CropPosition>,
    aspect_ratio: Option<AspectRatio>,
    #[serde(alias = "upscale")]
    allow_upscale: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawPresetOverrideConfig {
    include: Vec<String>,
    exclude: Vec<String>,
    presets: Vec<String>,
    quality: Option<u8>,
    fit: Option<FitMode>,
    #[serde(alias = "crop_anchor", alias = "crop_position", alias = "focal_point")]
    crop: Option<CropPosition>,
    #[serde(alias = "upscale")]
    allow_upscale: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawBudgetConfig {
    #[serde(default, deserialize_with = "deserialize_optional_byte_size")]
    max_total_bytes: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_byte_size")]
    max_file_bytes: Option<u64>,
    fail_on_regression: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ByteSizeValue {
    Text(String),
    Number(u64),
}

fn deserialize_optional_byte_size<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<ByteSizeValue>::deserialize(deserializer)?;
    value.map(parse_byte_size_value).transpose()
}

fn parse_byte_size_value<E>(value: ByteSizeValue) -> std::result::Result<u64, E>
where
    E: serde::de::Error,
{
    match value {
        ByteSizeValue::Number(bytes) => Ok(bytes),
        ByteSizeValue::Text(raw) => parse_byte_size(&raw).map_err(E::custom),
    }
}

fn parse_byte_size(raw: &str) -> std::result::Result<u64, String> {
    let value = raw.trim().to_ascii_lowercase();
    let split = value
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    if number.is_empty() {
        return Err(format!("invalid byte size `{raw}`"));
    }
    let base = number
        .parse::<u64>()
        .map_err(|_| format!("invalid byte size `{raw}`"))?;
    let multiplier = match unit.trim() {
        "" | "b" => 1,
        "kb" | "kib" => 1024,
        "mb" | "mib" => 1024 * 1024,
        "gb" | "gib" => 1024 * 1024 * 1024,
        _ => return Err(format!("invalid byte size unit `{unit}`")),
    };
    Ok(base * multiplier)
}

fn required<T>(value: Option<T>, path: &Path, field: &str) -> Result<T> {
    value.ok_or_else(|| DevimgError::config(path, format!("missing {field}")))
}

fn reject_unknown_top_level(path: &Path, value: &toml::Value) -> Result<()> {
    let Some(table) = value.as_table() else {
        return Err(DevimgError::config(
            path,
            "config root must be a TOML table",
        ));
    };
    for key in table.keys() {
        if !matches!(
            key.as_str(),
            "project"
                | "sources"
                | "source"
                | "preset"
                | "presets"
                | "overrides"
                | "override"
                | "budgets"
        ) {
            return Err(DevimgError::config(
                path,
                format!("unsupported top-level section or key `{key}`"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{parse_config, CropPosition, FitMode, FormatKind};

    #[test]
    fn parses_minimal_config() {
        let raw = r#"
[project]
root = "."
manifest = "public/images/devimg-manifest.json"
report = "devimg-report.md"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"
include = ["**/*.png", "**/*.jpg"]

[[preset]]
name = "project-card"
widths = [640, 960]
formats = ["webp", "jpeg"]
quality = 82
fit = "cover"
aspect_ratio = "16:9"

[budgets]
max_total_bytes = "3mb"
max_file_bytes = "350kb"
fail_on_regression = true
"#;
        let config = parse_config(Path::new("devimg.toml"), raw).expect("config parses");
        assert_eq!(config.sources[0].name, "portfolio");
        assert_eq!(
            config.presets[0].formats,
            vec![FormatKind::Webp, FormatKind::Jpeg]
        );
        assert_eq!(config.presets[0].fit, FitMode::Cover);
        assert_eq!(config.presets[0].crop, CropPosition::CENTER);
        assert_eq!(config.budgets.max_file_bytes, Some(350 * 1024));
    }

    #[test]
    fn supports_aliases_and_defaults() {
        let raw = r#"
[project]
manifest = "public/images/devimg-manifest.json"
allow_overwrite = true

[[source]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"

[[presets]]
name = "project-card"
widths = [640]
formats = ["jpg"]
upscale = true
"#;
        let config = parse_config(Path::new("devimg.toml"), raw).expect("config parses");
        assert!(config.project.overwrite);
        assert!(!config.project.content_hash_filenames);
        assert!(config.presets[0].allow_upscale);
        assert_eq!(config.presets[0].quality, 82);
        assert_eq!(config.presets[0].formats, vec![FormatKind::Jpeg]);
    }

    #[test]
    fn parses_avif_as_output_only_format() {
        let raw = r#"
[project]
manifest = "public/images/devimg-manifest.json"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"

[[preset]]
name = "project-card"
widths = [640]
formats = ["webp", "avif"]
"#;
        let config = parse_config(Path::new("devimg.toml"), raw).expect("config parses");

        assert_eq!(
            config.presets[0].formats,
            vec![FormatKind::Webp, FormatKind::Avif]
        );
        assert!(FormatKind::Webp.supports_source_input());
        assert!(!FormatKind::Avif.supports_source_input());
    }

    #[test]
    fn parses_crop_anchor() {
        let raw = r#"
[project]
manifest = "public/images/devimg-manifest.json"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"

[[preset]]
name = "project-card"
widths = [640]
formats = ["webp"]
crop = "top-right"
"#;
        let config = parse_config(Path::new("devimg.toml"), raw).expect("config parses");

        assert_eq!(config.presets[0].crop, CropPosition { x: 1.0, y: 0.0 });
    }

    #[test]
    fn parses_crop_focal_point() {
        let raw = r#"
[project]
manifest = "public/images/devimg-manifest.json"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"

[[preset]]
name = "project-card"
widths = [640]
formats = ["webp"]
crop = { x = 0.5, y = 0.0 }
"#;
        let config = parse_config(Path::new("devimg.toml"), raw).expect("config parses");

        assert_eq!(config.presets[0].crop, CropPosition { x: 0.5, y: 0.0 });
    }

    #[test]
    fn rejects_invalid_crop_focal_point() {
        let raw = r#"
[project]
manifest = "public/images/devimg-manifest.json"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"

[[preset]]
name = "project-card"
widths = [640]
formats = ["webp"]
crop = { x = 0.5, y = 1.5 }
"#;
        let error = parse_config(Path::new("devimg.toml"), raw).expect_err("config fails");

        assert!(error.to_string().contains("crop x and y"));
    }

    #[test]
    fn parses_content_hash_filename_alias() {
        let raw = r#"
[project]
manifest = "public/images/devimg-manifest.json"
hash_filenames = true

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"

[[preset]]
name = "project-card"
widths = [640]
formats = ["webp"]
"#;
        let config = parse_config(Path::new("devimg.toml"), raw).expect("config parses");

        assert!(config.project.content_hash_filenames);
    }

    #[test]
    fn parses_preset_overrides() {
        let raw = r#"
[project]
manifest = "public/images/devimg-manifest.json"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"

[[preset]]
name = "project-card"
widths = [640]
formats = ["webp"]

[[overrides]]
include = ["cli_tools.png"]
presets = ["project-card"]
quality = 74
fit = "contain"
crop = "top"
upscale = true
"#;
        let config = parse_config(Path::new("devimg.toml"), raw).expect("config parses");

        assert_eq!(config.overrides.len(), 1);
        assert_eq!(config.overrides[0].include, vec!["cli_tools.png"]);
        assert_eq!(config.overrides[0].presets, vec!["project-card"]);
        assert_eq!(config.overrides[0].quality, Some(74));
        assert_eq!(config.overrides[0].fit, Some(FitMode::Contain));
        assert_eq!(
            config.overrides[0].crop,
            Some(CropPosition { x: 0.5, y: 0.0 })
        );
        assert_eq!(config.overrides[0].allow_upscale, Some(true));
    }

    #[test]
    fn rejects_invalid_override_quality() {
        let raw = r#"
[project]
manifest = "public/images/devimg-manifest.json"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"

[[preset]]
name = "project-card"
widths = [640]
formats = ["webp"]

[[overrides]]
include = ["card.png"]
quality = 101
"#;
        let error = parse_config(Path::new("devimg.toml"), raw).expect_err("config fails");

        assert!(error.to_string().contains("override quality"));
    }

    #[test]
    fn rejects_unknown_top_level_sections() {
        let raw = r#"
[project]
manifest = "public/images/devimg-manifest.json"

[unknown]
value = true

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"

[[preset]]
name = "project-card"
widths = [640]
formats = ["webp"]
"#;
        let error = parse_config(Path::new("devimg.toml"), raw).expect_err("config fails");
        assert!(error.to_string().contains("unsupported top-level"));
    }
}
