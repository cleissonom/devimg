use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::budget::evaluate_budgets;
pub use crate::check::{check, check_with_options, CheckOptions};
use crate::config::{resolve_project_path_checked, Config, CropPosition, FitMode, FormatKind};
use crate::incremental::{IncrementalCache, IncrementalLookup};
use crate::manifest::{write_manifest, Manifest};
pub use crate::plan::build_plan;
use crate::quality::{append_unique, manifest_quality_warnings};
use crate::report::render_run_report;
pub use crate::scan::{inspect_image, scan_sources};
use crate::transform::execute_operation;
use crate::{DevimgError, Result};

#[derive(Debug, Clone)]
pub struct SourceImage {
    pub source_name: String,
    pub path: PathBuf,
    pub project_path: String,
    pub relative_path: PathBuf,
    pub output_root: PathBuf,
    pub output_root_project_path: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub hash: String,
    pub format: FormatKind,
}

#[derive(Debug, Clone)]
pub struct Operation {
    pub source: SourceImage,
    pub preset: String,
    pub fit: FitMode,
    pub crop: CropPosition,
    pub quality: u8,
    pub format: FormatKind,
    pub width: u32,
    pub height: u32,
    pub content_hash_filenames: bool,
    pub output_path: PathBuf,
    pub output_project_path: String,
    pub operation_hash: String,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub operations: Vec<Operation>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OptimizeOptions {
    pub dry_run: bool,
    pub allow_overwrite: bool,
}

#[derive(Debug, Clone)]
pub struct OptimizeResult {
    pub mode: String,
    pub source_count: usize,
    pub planned_count: usize,
    pub generated_count: usize,
    pub skipped_count: usize,
    pub stale_count: usize,
    pub source_bytes: u64,
    pub output_bytes: u64,
    pub warnings: Vec<String>,
    pub issues: Vec<CheckIssue>,
    pub budget_status: String,
    pub manifest: Manifest,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub passed: bool,
    pub result: OptimizeResult,
}

#[derive(Debug, Clone)]
pub struct CheckIssue {
    pub kind: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ImageInspection {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub bytes: u64,
    pub hash: String,
}

pub fn optimize(config: &Config, options: OptimizeOptions) -> Result<OptimizeResult> {
    let sources = scan_sources(config)?;
    let plan = build_plan(config, &sources)?;
    let source_bytes = unique_source_bytes(&sources);
    let mode = if options.dry_run {
        "dry-run"
    } else {
        "optimize"
    }
    .to_string();

    if options.dry_run {
        let result = OptimizeResult {
            mode,
            source_count: sources.len(),
            planned_count: plan.operations.len(),
            generated_count: 0,
            skipped_count: 0,
            stale_count: 0,
            source_bytes,
            output_bytes: 0,
            warnings: plan.warnings,
            issues: Vec::new(),
            budget_status: "not evaluated during dry-run".to_string(),
            manifest: Manifest::new(
                path_to_string(&config.path),
                config.config_hash.clone(),
                Vec::new(),
            ),
        };
        return Ok(result);
    }

    let manifest_path =
        resolve_project_path_checked(config, &config.project.manifest, "manifest path")?;
    let _report_path = resolve_project_path_checked(config, &config.project.report, "report path")?;

    let mut outputs = Vec::new();
    let mut warnings = plan.warnings;
    let incremental_cache = IncrementalCache::read(config, &manifest_path);
    let mut generated_count = 0usize;
    let mut skipped_count = 0usize;
    let mut stale_count = 0usize;
    for operation in &plan.operations {
        if let Some(cache) = &incremental_cache {
            match cache.lookup_current(config, operation)? {
                IncrementalLookup::Current(output) => {
                    skipped_count += 1;
                    outputs.push(*output);
                    continue;
                }
                IncrementalLookup::Stale => {
                    stale_count += 1;
                }
            }
        }
        outputs.push(execute_operation(
            operation,
            config.project.overwrite || options.allow_overwrite,
        )?);
        generated_count += 1;
    }

    let manifest = Manifest::new(
        path_to_string(&config.path),
        config.config_hash.clone(),
        outputs,
    );
    let (budget_status, issues) = evaluate_budgets(config, &manifest.outputs);
    let output_bytes = manifest.output_bytes_total();
    append_unique(&mut warnings, manifest_quality_warnings(&manifest));
    if !issues.is_empty() {
        warnings.push(format!(
            "budget status is fail with {} issue(s); `devimg check` will fail",
            issues.len()
        ));
    }

    let result = OptimizeResult {
        mode,
        source_count: sources.len(),
        planned_count: plan.operations.len(),
        generated_count,
        skipped_count,
        stale_count,
        source_bytes,
        output_bytes,
        warnings,
        issues,
        budget_status,
        manifest,
    };

    write_manifest(&manifest_path, &result.manifest)?;
    write_report(config, &result)?;
    Ok(result)
}

pub(crate) fn write_report(config: &Config, result: &OptimizeResult) -> Result<()> {
    let report_path = resolve_project_path_checked(config, &config.project.report, "report path")?;
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent).map_err(|source| DevimgError::io(parent, source))?;
    }
    fs::write(&report_path, render_run_report(result))
        .map_err(|source| DevimgError::io(&report_path, source))
}

pub(crate) fn unique_source_bytes(sources: &[SourceImage]) -> u64 {
    let mut by_path = BTreeMap::new();
    for source in sources {
        by_path.insert(source.project_path.as_str(), source.bytes);
    }
    by_path.values().sum()
}

pub(crate) fn path_to_string(path: &Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

    use crate::{
        build_plan, check, load_config, optimize, scan_sources, DevimgError, OptimizeOptions,
    };

    #[test]
    fn optimize_generates_manifest_and_checkable_outputs() {
        let project = temp_project("basic");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let result = optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");

        assert_eq!(result.manifest.outputs.len(), 2);
        assert!(project
            .join("public/images/generated/card.project-card.640.webp")
            .exists());
        assert!(project.join("public/images/devimg-manifest.json").exists());
        assert!(project.join("devimg-report.md").exists());
        cleanup(&project);
    }

    #[test]
    fn optimize_generates_opt_in_avif_outputs() {
        let project = temp_project("avif_output");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_avif_config(&project);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let result = optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");
        let output = project.join("public/images/generated/card.project-card.320.avif");
        let output_bytes = fs::read(&output).expect("avif output exists");

        assert_eq!(result.manifest.outputs.len(), 1);
        assert_eq!(result.manifest.outputs[0].format, "avif");
        assert_avif_container(&output_bytes);
        assert!(check(&config).expect("check runs").passed);
        cleanup(&project);
    }

    #[test]
    fn dry_run_does_not_write_outputs() {
        let project = temp_project("dry_run");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let result = optimize(
            &config,
            OptimizeOptions {
                dry_run: true,
                allow_overwrite: false,
            },
        )
        .expect("dry-run succeeds");

        assert_eq!(result.planned_count, 2);
        assert_eq!(result.generated_count, 0);
        assert_eq!(result.skipped_count, 0);
        assert_eq!(result.stale_count, 0);
        assert!(!project.join("public/images/devimg-manifest.json").exists());
        assert!(!project.join("devimg-report.md").exists());
        cleanup(&project);
    }

    #[test]
    fn optimize_skips_current_stable_outputs() {
        let project = temp_project("stable_incremental");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let first = optimize(&config, OptimizeOptions::default()).expect("first optimize succeeds");
        let second =
            optimize(&config, OptimizeOptions::default()).expect("second optimize succeeds");

        assert_eq!(first.generated_count, 2);
        assert_eq!(first.skipped_count, 0);
        assert_eq!(second.generated_count, 0);
        assert_eq!(second.skipped_count, 2);
        assert_eq!(second.stale_count, 0);
        assert!(check(&config).expect("check runs").passed);
        cleanup(&project);
    }

    #[test]
    fn optimize_skips_current_content_hash_outputs() {
        let project = temp_project("hash_incremental");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_hashed_config_with_width(&project, 640, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let first = optimize(&config, OptimizeOptions::default()).expect("first optimize succeeds");
        let second =
            optimize(&config, OptimizeOptions::default()).expect("second optimize succeeds");

        assert_eq!(first.generated_count, 2);
        assert_eq!(first.skipped_count, 0);
        assert_eq!(second.generated_count, 0);
        assert_eq!(second.skipped_count, 2);
        assert_eq!(second.stale_count, 0);
        assert!(check(&config).expect("check runs").passed);
        cleanup(&project);
    }

    #[test]
    fn optimize_regenerates_missing_output_and_skips_current_outputs() {
        let project = temp_project("missing_incremental");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        optimize(&config, OptimizeOptions::default()).expect("first optimize succeeds");
        fs::remove_file(project.join("public/images/generated/card.project-card.640.webp"))
            .expect("output removed");

        let result = optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");

        assert_eq!(result.generated_count, 1);
        assert_eq!(result.skipped_count, 1);
        assert_eq!(result.stale_count, 1);
        assert!(check(&config).expect("check runs").passed);
        cleanup(&project);
    }

    #[test]
    fn optimize_does_not_skip_after_config_hash_changes() {
        let project = temp_project("config_stale_incremental");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        optimize(&config, OptimizeOptions::default()).expect("first optimize succeeds");
        write_config(&project, r#"max_total_bytes = "4mb""#);
        let changed_config = load_config(project.join("devimg.toml")).expect("config reloads");

        let result =
            optimize(&changed_config, OptimizeOptions::default()).expect("optimize succeeds");

        assert_eq!(result.generated_count, 2);
        assert_eq!(result.skipped_count, 0);
        assert_eq!(result.stale_count, 2);
        assert!(check(&changed_config).expect("check runs").passed);
        cleanup(&project);
    }

    #[test]
    fn content_hash_optimize_regenerates_after_source_changes() {
        let project = temp_project("source_stale_incremental");
        let source = project.join("assets/images/card.png");
        write_image(&source, 800, 450);
        write_hashed_config_with_width(&project, 640, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        optimize(&config, OptimizeOptions::default()).expect("first optimize succeeds");
        write_image(&source, 900, 450);
        let changed_config = load_config(project.join("devimg.toml")).expect("config reloads");

        let result =
            optimize(&changed_config, OptimizeOptions::default()).expect("optimize succeeds");

        assert_eq!(result.generated_count, 2);
        assert_eq!(result.skipped_count, 0);
        assert_eq!(result.stale_count, 2);
        assert!(check(&changed_config).expect("check runs").passed);
        cleanup(&project);
    }

    #[test]
    fn scan_ignores_nested_output_directory() {
        let project = temp_project("overlap");
        write_image(&project.join("assets/images/source.png"), 800, 450);
        write_image(&project.join("assets/images/generated/old.png"), 800, 450);
        write_overlap_config(&project);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let sources = scan_sources(&config).expect("scan succeeds");

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].project_path, "assets/images/source.png");
        cleanup(&project);
    }

    #[test]
    fn scan_matches_root_and_nested_paths_case_insensitively() {
        let project = temp_project("glob_case");
        write_image(&project.join("assets/images/Card.PNG"), 800, 450);
        write_image(&project.join("assets/images/nested/Other.PNG"), 800, 450);
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let sources = scan_sources(&config).expect("scan succeeds");
        let paths = sources
            .iter()
            .map(|source| source.project_path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            paths,
            vec!["assets/images/Card.PNG", "assets/images/nested/Other.PNG"]
        );
        cleanup(&project);
    }

    #[test]
    fn plan_skips_upscale_by_default() {
        let project = temp_project("small");
        write_image(&project.join("assets/images/small.png"), 320, 180);
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let sources = scan_sources(&config).expect("scan succeeds");
        let plan = build_plan(&config, &sources).expect("plan succeeds");

        assert!(plan.operations.is_empty());
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.contains("upscaling")));
        cleanup(&project);
    }

    #[test]
    fn check_passes_after_optimize() {
        let project = temp_project("check_pass");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");
        let result = check(&config).expect("check runs");

        assert!(result.passed);
        cleanup(&project);
    }

    #[test]
    fn check_fails_when_output_is_deleted() {
        let project = temp_project("check_missing");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");
        fs::remove_file(project.join("public/images/generated/card.project-card.640.webp"))
            .expect("output removed");
        let result = check(&config).expect("check runs");

        assert!(!result.passed);
        assert!(result
            .result
            .issues
            .iter()
            .any(|issue| issue.kind == "missing"));
        cleanup(&project);
    }

    #[test]
    fn check_fails_when_budget_is_exceeded() {
        let project = temp_project("check_budget");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_config(&project, r#"max_total_bytes = "1b""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");
        let result = check(&config).expect("check runs");

        assert!(!result.passed);
        assert!(result
            .result
            .issues
            .iter()
            .any(|issue| issue.kind == "oversized_total"));
        cleanup(&project);
    }

    #[test]
    fn check_fails_when_config_changes_without_regeneration() {
        let project = temp_project("check_config_change");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");
        write_config_with_width(&project, 320, r#"max_total_bytes = "5mb""#);
        let changed_config = load_config(project.join("devimg.toml")).expect("config reloads");
        let result = check(&changed_config).expect("check runs");

        assert!(!result.passed);
        assert!(result
            .result
            .issues
            .iter()
            .any(|issue| issue.kind == "outdated_config"));
        cleanup(&project);
    }

    #[test]
    fn crop_config_changes_operation_hashes_and_check_status() {
        let project = temp_project("check_crop_change");
        write_image(&project.join("assets/images/card.png"), 800, 1200);
        write_config_with_crop(&project, r#"crop = "center""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let sources = scan_sources(&config).expect("scan succeeds");
        let center_hash = build_plan(&config, &sources)
            .expect("plan succeeds")
            .operations[0]
            .operation_hash
            .clone();
        optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");

        write_config_with_crop(&project, r#"crop = "top""#);
        let changed_config = load_config(project.join("devimg.toml")).expect("config reloads");
        let changed_sources = scan_sources(&changed_config).expect("scan succeeds");
        let top_hash = build_plan(&changed_config, &changed_sources)
            .expect("changed plan succeeds")
            .operations[0]
            .operation_hash
            .clone();
        let result = check(&changed_config).expect("check runs");

        assert_ne!(center_hash, top_hash);
        assert!(!result.passed);
        assert!(result
            .result
            .issues
            .iter()
            .any(|issue| issue.kind == "outdated_config"));
        cleanup(&project);
    }

    #[test]
    fn preset_overrides_apply_to_matching_sources_only() {
        let project = temp_project("preset_override");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_image(&project.join("assets/images/cli_tools.png"), 1731, 909);
        write_config_with_override(&project, true);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let sources = scan_sources(&config).expect("scan succeeds");
        let plan = build_plan(&config, &sources).expect("plan succeeds");

        let card = plan
            .operations
            .iter()
            .find(|operation| operation.source.project_path.ends_with("card.png"))
            .expect("card operation exists");
        let cli_tools = plan
            .operations
            .iter()
            .find(|operation| operation.source.project_path.ends_with("cli_tools.png"))
            .expect("cli tools operation exists");

        assert_eq!((card.width, card.height), (640, 360));
        assert_eq!(card.fit, crate::FitMode::Cover);
        assert_eq!((cli_tools.width, cli_tools.height), (640, 336));
        assert_eq!(cli_tools.fit, crate::FitMode::Contain);
        cleanup(&project);
    }

    #[test]
    fn preset_override_changes_make_check_stale() {
        let project = temp_project("override_stale");
        write_image(&project.join("assets/images/cli_tools.png"), 1731, 909);
        write_config_with_override(&project, true);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");

        write_config_with_override(&project, false);
        let changed_config = load_config(project.join("devimg.toml")).expect("config reloads");
        let result = check(&changed_config).expect("check runs");

        assert!(!result.passed);
        assert!(result
            .result
            .issues
            .iter()
            .any(|issue| issue.kind == "outdated_config"));
        cleanup(&project);
    }

    #[test]
    fn content_hash_filenames_use_generated_byte_hashes() {
        let project = temp_project("hash_filenames");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_hashed_config_with_width(&project, 640, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let result = optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");

        assert_eq!(result.manifest.outputs.len(), 2);
        assert!(!project
            .join("public/images/generated/card.project-card.640.webp")
            .exists());
        for output in &result.manifest.outputs {
            let fragment = output
                .hash
                .strip_prefix("blake3:")
                .expect("content hash uses blake3")
                .get(..12)
                .expect("content hash has a fragment");
            assert!(
                output.output_path.contains(&format!(".{fragment}.")),
                "output path should include content hash fragment: {}",
                output.output_path
            );
            assert!(project.join(&output.output_path).exists());
        }

        let check_result = check(&config).expect("check runs");
        assert!(check_result.passed);
        cleanup(&project);
    }

    #[test]
    fn content_hash_check_fails_when_output_is_deleted() {
        let project = temp_project("hash_missing");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_hashed_config_with_width(&project, 640, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let result = optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");
        fs::remove_file(project.join(&result.manifest.outputs[0].output_path))
            .expect("hashed output removed");
        let check_result = check(&config).expect("check runs");

        assert!(!check_result.passed);
        assert!(check_result
            .result
            .issues
            .iter()
            .any(|issue| issue.kind == "missing"));
        cleanup(&project);
    }

    #[test]
    fn content_hash_check_fails_when_config_changes_without_regeneration() {
        let project = temp_project("hash_stale");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_hashed_config_with_width(&project, 640, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");
        write_hashed_config_with_width(&project, 320, r#"max_total_bytes = "5mb""#);
        let changed_config = load_config(project.join("devimg.toml")).expect("config reloads");
        let check_result = check(&changed_config).expect("check runs");

        assert!(!check_result.passed);
        assert!(check_result
            .result
            .issues
            .iter()
            .any(|issue| issue.kind == "stale"));
        cleanup(&project);
    }

    #[test]
    fn scan_rejects_extension_magic_mismatch() {
        let project = temp_project("mismatch");
        write_image(&project.join("assets/images/not_jpeg.jpg"), 800, 450);
        write_jpeg_include_config(&project);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let error = scan_sources(&config).expect_err("scan should reject mismatch");

        assert!(matches!(error, DevimgError::Image { .. }));
        cleanup(&project);
    }

    #[test]
    fn scan_empty_source_directory_is_successful_noop() {
        let project = temp_project("empty");
        fs::create_dir_all(project.join("assets/images")).expect("source creates");
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let sources = scan_sources(&config).expect("scan succeeds");
        let plan = build_plan(&config, &sources).expect("plan succeeds");

        assert!(sources.is_empty());
        assert!(plan.operations.is_empty());
        cleanup(&project);
    }

    #[test]
    fn optimize_refuses_existing_unmanaged_output() {
        let project = temp_project("existing_output");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_config(&project, r#"max_total_bytes = "5mb""#);
        let unmanaged = project.join("public/images/generated/card.project-card.640.webp");
        fs::create_dir_all(unmanaged.parent().expect("parent")).expect("parent creates");
        fs::write(&unmanaged, b"unmanaged").expect("unmanaged output writes");

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let error = optimize(&config, OptimizeOptions::default())
            .expect_err("unmanaged output should be refused");

        assert!(matches!(error, DevimgError::UnsafeOverwrite { .. }));
        cleanup(&project);
    }

    #[test]
    fn optimize_rejects_manifest_path_outside_project_root() {
        let project = temp_project("manifest_escape");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_config_with_paths(
            &project,
            "../outside-devimg-manifest.json",
            "devimg-report.md",
            640,
            r#"max_total_bytes = "5mb""#,
        );

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let error =
            optimize(&config, OptimizeOptions::default()).expect_err("manifest path is rejected");

        assert!(matches!(error, DevimgError::Config { .. }));
        assert!(!project.join("../outside-devimg-manifest.json").exists());
        cleanup(&project);
    }

    #[test]
    fn check_rejects_report_path_outside_project_root() {
        let project = temp_project("report_escape");
        write_image(&project.join("assets/images/card.png"), 800, 450);
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        optimize(&config, OptimizeOptions::default()).expect("optimize succeeds");
        write_config_with_paths(
            &project,
            "public/images/devimg-manifest.json",
            "../outside-devimg-report.md",
            640,
            r#"max_total_bytes = "5mb""#,
        );
        let changed_config = load_config(project.join("devimg.toml")).expect("config reloads");
        let error = check(&changed_config).expect_err("report path is rejected");

        assert!(matches!(error, DevimgError::Config { .. }));
        assert!(!project.join("../outside-devimg-report.md").exists());
        cleanup(&project);
    }

    #[test]
    fn plan_preserves_relative_paths_for_same_basename_sources() {
        let project = temp_project("same_basename");
        write_image(&project.join("assets/images/a/card.png"), 800, 450);
        write_image(&project.join("assets/images/b/card.png"), 800, 450);
        write_config(&project, r#"max_total_bytes = "5mb""#);

        let config = load_config(project.join("devimg.toml")).expect("config loads");
        let sources = scan_sources(&config).expect("scan succeeds");
        let plan = build_plan(&config, &sources).expect("plan succeeds");

        assert!(plan
            .operations
            .iter()
            .any(|operation| operation.output_project_path.contains("generated/a/card")));
        assert!(plan
            .operations
            .iter()
            .any(|operation| operation.output_project_path.contains("generated/b/card")));
        cleanup(&project);
    }

    fn write_config(project: &Path, budget_line: &str) {
        write_config_with_width(project, 640, budget_line);
    }

    fn write_config_with_width(project: &Path, width: u32, budget_line: &str) {
        write_config_with_paths(
            project,
            "public/images/devimg-manifest.json",
            "devimg-report.md",
            width,
            budget_line,
        );
    }

    fn write_config_with_crop(project: &Path, crop_line: &str) {
        fs::write(
            project.join("devimg.toml"),
            format!(
                r#"[project]
root = "."
manifest = "public/images/devimg-manifest.json"
report = "devimg-report.md"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"
include = ["**/*.png"]

[[preset]]
name = "project-card"
widths = [640]
formats = ["webp"]
quality = 82
fit = "cover"
aspect_ratio = "16:9"
{crop_line}

[budgets]
max_total_bytes = "5mb"
"#
            ),
        )
        .expect("config writes");
    }

    fn write_config_with_override(project: &Path, include_override: bool) {
        let override_section = if include_override {
            r#"
[[overrides]]
include = ["cli_tools.png"]
fit = "contain"
"#
        } else {
            ""
        };
        fs::write(
            project.join("devimg.toml"),
            format!(
                r#"[project]
root = "."
manifest = "public/images/devimg-manifest.json"
report = "devimg-report.md"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"
include = ["**/*.png"]

[[preset]]
name = "project-card"
widths = [640]
formats = ["webp"]
quality = 82
fit = "cover"
aspect_ratio = "16:9"
{override_section}

[budgets]
max_total_bytes = "5mb"
"#
            ),
        )
        .expect("config writes");
    }

    fn write_config_with_paths(
        project: &Path,
        manifest: &str,
        report: &str,
        width: u32,
        budget_line: &str,
    ) {
        fs::write(
            project.join("devimg.toml"),
            format!(
                r#"[project]
root = "."
manifest = "{manifest}"
report = "{report}"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"
include = ["**/*.png"]

[[preset]]
name = "project-card"
widths = [{width}]
formats = ["webp", "jpeg"]
quality = 82
fit = "cover"
aspect_ratio = "16:9"

[budgets]
{budget_line}
"#
            ),
        )
        .expect("config writes");
    }

    fn write_hashed_config_with_width(project: &Path, width: u32, budget_line: &str) {
        fs::write(
            project.join("devimg.toml"),
            format!(
                r#"[project]
root = "."
manifest = "public/images/devimg-manifest.json"
report = "devimg-report.md"
content_hash_filenames = true

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"
include = ["**/*.png"]

[[preset]]
name = "project-card"
widths = [{width}]
formats = ["webp", "jpeg"]
quality = 82
fit = "cover"
aspect_ratio = "16:9"

[budgets]
{budget_line}
"#
            ),
        )
        .expect("config writes");
    }

    fn write_jpeg_include_config(project: &Path) {
        fs::write(
            project.join("devimg.toml"),
            r#"[project]
root = "."
manifest = "public/images/devimg-manifest.json"
report = "devimg-report.md"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"
include = ["**/*.jpg"]

[[preset]]
name = "project-card"
widths = [640]
formats = ["jpeg"]
quality = 82
fit = "cover"
aspect_ratio = "16:9"
"#,
        )
        .expect("config writes");
    }

    fn write_avif_config(project: &Path) {
        fs::write(
            project.join("devimg.toml"),
            r#"[project]
root = "."
manifest = "public/images/devimg-manifest.json"
report = "devimg-report.md"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"
include = ["**/*.png"]

[[preset]]
name = "project-card"
widths = [320]
formats = ["avif"]
quality = 72
fit = "cover"
aspect_ratio = "16:9"

[budgets]
max_total_bytes = "5mb"
"#,
        )
        .expect("config writes");
    }

    fn write_overlap_config(project: &Path) {
        fs::write(
            project.join("devimg.toml"),
            r#"[project]
root = "."
manifest = "assets/images/generated/devimg-manifest.json"
report = "devimg-report.md"

[[sources]]
name = "portfolio"
input = "assets/images"
output = "assets/images/generated"
include = ["**/*.png"]

[[preset]]
name = "project-card"
widths = [640]
formats = ["webp"]
quality = 82
fit = "cover"
aspect_ratio = "16:9"
"#,
        )
        .expect("config writes");
    }

    fn write_image(path: &Path, width: u32, height: u32) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent creates");
        }
        let mut image = RgbaImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = Rgba([(x % 255) as u8, (y % 255) as u8, 128, 255]);
        }
        DynamicImage::ImageRgba8(image)
            .save_with_format(path, ImageFormat::Png)
            .expect("image writes");
    }

    fn assert_avif_container(bytes: &[u8]) {
        assert!(bytes.len() > 16, "AVIF output should not be empty");
        assert_eq!(&bytes[4..8], b"ftyp");
        assert!(
            bytes.windows(4).any(|window| window == b"avif"),
            "AVIF output should contain the avif brand"
        );
    }

    fn temp_project(label: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("devimg_{label}_{}_{}", std::process::id(), now));
        fs::create_dir_all(&path).expect("project creates");
        path
    }

    fn cleanup(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("project cleanup");
        }
    }
}
