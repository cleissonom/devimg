use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::Config;
use crate::pipeline::path_to_string;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameworkInspection {
    pub frameworks: Vec<String>,
    pub warnings: Vec<FrameworkWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameworkWarning {
    pub code: String,
    pub path: String,
    pub message: String,
    pub hint: String,
}

pub(crate) fn inspect_frameworks(
    config: &Config,
    manifest_export_configured: bool,
) -> FrameworkInspection {
    let detected = detect_frameworks(&config.project.root);
    let frameworks = detected.keys().cloned().collect::<Vec<_>>();
    let mut warnings = Vec::new();
    let public_output = has_public_output(config);

    if frameworks.len() > 1 {
        warnings.push(warning(
            "framework_multiple_detected",
            &config.path,
            format!(
                "multiple frontend frameworks were detected: {}",
                frameworks.join(", ")
            ),
            "Review generated image paths and manifest export usage for the actual app entrypoint.",
        ));
    }

    if frameworks.iter().any(|framework| framework == "next") && public_output {
        warnings.push(warning(
            "framework_next_image_double_optimization",
            &config.path,
            "Next.js was detected with generated assets under public/; next/image or hosting image optimization may reprocess already-generated variants",
            "Use generated variants through img/picture or configure Next image usage intentionally to avoid double optimization.",
        ));
    }

    if !frameworks.is_empty() && public_output && !config.project.content_hash_filenames {
        warnings.push(warning(
            "framework_cache_without_hash",
            &config.path,
            "frontend framework project outputs generated assets under public/ without content-hash filenames",
            "Enable [project].content_hash_filenames before using long-lived immutable caching for generated assets.",
        ));
    }

    if !frameworks.is_empty()
        && config.project.content_hash_filenames
        && !manifest_export_configured
    {
        warnings.push(warning(
            "framework_manifest_export_missing",
            &config.path,
            "content-hash filenames are enabled in a framework project, but doctor was not given --export-output to verify a checked-in manifest helper",
            "If the app consumes generated paths from a helper, pass --export-output with the same options used by devimg manifest export.",
        ));
    }

    FrameworkInspection {
        frameworks,
        warnings,
    }
}

fn detect_frameworks(root: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let mut detected = BTreeMap::<String, BTreeSet<String>>::new();
    detect_config_files(root, &mut detected);
    detect_package_json(root, &mut detected);
    detected
}

fn detect_config_files(root: &Path, detected: &mut BTreeMap<String, BTreeSet<String>>) {
    for file in [
        "next.config.js",
        "next.config.mjs",
        "next.config.cjs",
        "next.config.ts",
    ] {
        detect_file(root, file, "next", detected);
    }
    for file in ["astro.config.js", "astro.config.mjs", "astro.config.ts"] {
        detect_file(root, file, "astro", detected);
    }
    for file in [
        "vite.config.js",
        "vite.config.mjs",
        "vite.config.cjs",
        "vite.config.ts",
    ] {
        detect_file(root, file, "vite", detected);
    }
}

fn detect_file(
    root: &Path,
    file: &str,
    framework: &str,
    detected: &mut BTreeMap<String, BTreeSet<String>>,
) {
    if root.join(file).exists() {
        detected
            .entry(framework.to_string())
            .or_default()
            .insert(file.to_string());
    }
}

fn detect_package_json(root: &Path, detected: &mut BTreeMap<String, BTreeSet<String>>) {
    let path = root.join("package.json");
    let Ok(raw) = fs::read_to_string(&path) else {
        return;
    };
    let Ok(document) = serde_json::from_str::<Value>(&raw) else {
        return;
    };
    for section in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
    ] {
        let Some(deps) = document.get(section).and_then(Value::as_object) else {
            continue;
        };
        for (package, _value) in deps {
            match package.as_str() {
                "next" => add_dependency(detected, "next", package),
                "astro" => add_dependency(detected, "astro", package),
                "vite" => add_dependency(detected, "vite", package),
                _ => {}
            }
        }
    }
}

fn add_dependency(
    detected: &mut BTreeMap<String, BTreeSet<String>>,
    framework: &str,
    package: &str,
) {
    detected
        .entry(framework.to_string())
        .or_default()
        .insert(format!("package:{package}"));
}

fn has_public_output(config: &Config) -> bool {
    config.sources.iter().any(|source| {
        first_component(&source.output).is_some_and(|component| component == "public")
    })
}

fn first_component(path: &Path) -> Option<String> {
    path.components().find_map(|component| match component {
        std::path::Component::Normal(value) => Some(value.to_string_lossy().to_string()),
        _ => None,
    })
}

fn warning(
    code: &str,
    path: impl Into<PathBuf>,
    message: impl Into<String>,
    hint: impl Into<String>,
) -> FrameworkWarning {
    FrameworkWarning {
        code: code.to_string(),
        path: path_to_string(&path.into()),
        message: message.into(),
        hint: hint.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::config::parse_config;

    use super::inspect_frameworks;

    #[test]
    fn detects_frameworks_from_package_json_and_config_files() {
        let root = temp_project("framework_detect");
        fs::write(
            root.join("package.json"),
            r#"{"dependencies":{"next":"15.0.0"},"devDependencies":{"vite":"7.0.0"}}"#,
        )
        .expect("package writes");
        fs::write(root.join("astro.config.mjs"), "export default {}\n")
            .expect("astro config writes");
        let config = config(&root, "content_hash_filenames = false");

        let report = inspect_frameworks(&config, false);

        assert_eq!(report.frameworks, vec!["astro", "next", "vite"]);
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.code == "framework_multiple_detected"));
        cleanup(&root);
    }

    #[test]
    fn warns_for_hashed_framework_outputs_without_export_check() {
        let root = temp_project("framework_export");
        fs::write(root.join("vite.config.ts"), "export default {}\n").expect("vite config writes");
        let config = config(&root, "content_hash_filenames = true");

        let without_export = inspect_frameworks(&config, false);
        let with_export = inspect_frameworks(&config, true);

        assert!(without_export
            .warnings
            .iter()
            .any(|warning| warning.code == "framework_manifest_export_missing"));
        assert!(!with_export
            .warnings
            .iter()
            .any(|warning| warning.code == "framework_manifest_export_missing"));
        cleanup(&root);
    }

    #[test]
    fn no_framework_project_has_no_framework_warnings() {
        let root = temp_project("framework_none");
        let config = config(&root, "content_hash_filenames = false");

        let report = inspect_frameworks(&config, false);

        assert!(report.frameworks.is_empty());
        assert!(report.warnings.is_empty());
        cleanup(&root);
    }

    fn config(root: &Path, project_setting: &str) -> crate::Config {
        let raw = format!(
            r#"[project]
root = "."
manifest = "public/images/devimg-manifest.json"
report = "devimg-report.md"
{project_setting}

[[sources]]
name = "app"
input = "assets/images"
output = "public/images/generated"
include = ["**/*.png"]

[[preset]]
name = "project-card"
widths = [64]
formats = ["webp"]
quality = 82
fit = "cover"
aspect_ratio = "16:9"
"#
        );
        parse_config(&root.join("devimg.toml"), &raw).expect("config parses")
    }

    fn temp_project(label: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "devimg_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("project creates");
        path
    }

    fn cleanup(path: &Path) {
        if path.exists() {
            fs::remove_dir_all(path).expect("cleanup temp project");
        }
    }
}
