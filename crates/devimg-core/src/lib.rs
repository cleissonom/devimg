mod budget;
mod check;
mod config;
mod doctor;
mod hash;
mod manifest;
mod pipeline;
mod plan;
mod report;
mod scan;
mod transform;

pub use config::{
    load_config, AspectRatio, BudgetConfig, Config, CropPosition, FitMode, FormatKind,
    PresetConfig, PresetOverrideConfig, ProjectConfig, SourceConfig,
};
pub use doctor::{
    doctor, doctor_report_to_json, DoctorBudget, DoctorCheck, DoctorDiagnostic,
    DoctorManifestExportFormat, DoctorManifestExportOptions, DoctorOptions, DoctorReport,
};
pub use hash::{hash_bytes, hash_file};
pub use manifest::{
    export_manifest, manifest_export_to_json, manifest_export_to_typescript, read_manifest,
    write_manifest, Manifest, ManifestExport, ManifestExportOptions, ManifestExportSource,
    ManifestExportVariant, ManifestOutput,
};
pub use pipeline::{
    build_plan, check, inspect_image, optimize, scan_sources, CheckIssue, CheckOptions,
    CheckResult, ImageInspection, Operation, OptimizeOptions, OptimizeResult, Plan, SourceImage,
};
pub use report::{render_doctor_report, render_manifest_report, render_run_report};

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, DevimgError>;

#[derive(Debug, thiserror::Error)]
pub enum DevimgError {
    #[error("config error in {}: {message}", path.display())]
    Config { path: PathBuf, message: String },
    #[error("I/O error at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("image error at {}: {message}", path.display())]
    Image { path: PathBuf, message: String },
    #[error("refusing to overwrite unmanaged output {} without explicit overwrite", path.display())]
    UnsafeOverwrite { path: PathBuf },
    #[error("image check failed with {} issue(s)", .issues.len())]
    CheckFailed { issues: Vec<CheckIssue> },
}

impl DevimgError {
    pub fn config(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Config {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn image(path: impl Into<PathBuf>, message: impl Into<String>) -> Self {
        Self::Image {
            path: path.into(),
            message: message.into(),
        }
    }
}
