use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn help_and_usage_exit_codes_are_stable() {
    assert_code(run(["--help"]), 0);
    assert_code(run(["doctor", "--help"]), 0);
    assert_code(run(["compare", "--help"]), 0);
    assert_code(run(["review", "--help"]), 0);
    let version = run(["--version"]);
    assert_status(&version, 0);
    assert!(String::from_utf8_lossy(&version.stdout).contains("0.1.14"));
    assert_code(run([] as [&str; 0]), 2);
    assert_code(run(["unknown"]), 2);
}

#[test]
fn missing_config_exits_2() {
    let project = temp_project("missing_config");
    let missing_config = path_arg(project.join("missing.toml"));
    let output = run(["optimize", "--config", missing_config.as_str()]);
    assert_status(&output, 2);
    assert!(String::from_utf8_lossy(&output.stderr).contains("Hint:"));

    let output = run(["doctor", "--config", missing_config.as_str()]);
    assert_status(&output, 2);
    assert!(String::from_utf8_lossy(&output.stderr).contains("Hint:"));
    cleanup(&project);
}

#[test]
fn dry_run_writes_nothing() {
    let project = fixture_project("dry_run", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    let output = run(["optimize", "--config", config.as_str(), "--dry-run"]);

    assert_code(output, 0);
    assert!(!project.join("public/images/devimg-manifest.json").exists());
    assert!(!project.join("devimg-report.md").exists());
    assert!(!project.join("public/images/generated").exists());
    cleanup(&project);
}

#[test]
fn optimize_and_check_success() {
    let project = fixture_project("success", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    assert!(project
        .join("public/images/generated/sample.project-card.64.webp")
        .exists());
    assert!(project.join("public/images/devimg-manifest.json").exists());
    assert!(project.join("devimg-report.md").exists());

    assert_code(run(["check", "--config", config.as_str()]), 0);
    cleanup(&project);
}

#[test]
fn check_no_report_keeps_report_path_read_only() {
    let project = fixture_project("check_no_report", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let report = project.join("devimg-report.md");

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    fs::remove_file(&report).expect("remove optimize report");

    let output = run(["check", "--config", config.as_str(), "--no-report"]);

    assert_status(&output, 0);
    assert!(String::from_utf8_lossy(&output.stdout).contains("Dev Image Pipeline Report"));
    assert!(!report.exists());
    cleanup(&project);
}

#[test]
fn optimize_reports_incremental_skips_and_stale_work() {
    let project = fixture_project("incremental_report", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    let first = run(["optimize", "--config", config.as_str()]);
    assert_status(&first, 0);
    let first_stdout = String::from_utf8_lossy(&first.stdout);
    assert!(first_stdout.contains("- Variants generated: `1`"));
    assert!(!first_stdout.contains("- Variants skipped:"));

    let second = run(["optimize", "--config", config.as_str()]);
    assert_status(&second, 0);
    let second_stdout = String::from_utf8_lossy(&second.stdout);
    assert!(second_stdout.contains("- Variants generated: `0`"));
    assert!(second_stdout.contains("- Variants skipped: `1`"));
    assert!(second_stdout.contains("- Manifest variants: `1`"));
    let report = fs::read_to_string(project.join("devimg-report.md")).expect("report reads");
    assert!(report.contains("- Variants skipped: `1`"));

    write_project_config(&project, 64, "", r#"max_total_bytes = "4mb""#);
    let metadata_refresh = run(["optimize", "--config", config.as_str()]);
    assert_status(&metadata_refresh, 0);
    let metadata_refresh_stdout = String::from_utf8_lossy(&metadata_refresh.stdout);
    assert!(metadata_refresh_stdout.contains("- Variants generated: `0`"));
    assert!(metadata_refresh_stdout.contains("- Variants skipped: `1`"));
    assert!(metadata_refresh_stdout.contains("- Manifest variants: `1`"));
    assert!(!metadata_refresh_stdout.contains("- Variants stale:"));

    cleanup(&project);
}

#[test]
fn doctor_passes_after_optimize_and_emits_json() {
    let project = fixture_project("doctor_success", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);

    let human = run(["doctor", "--config", config.as_str()]);
    assert_status(&human, 0);
    let stdout = String::from_utf8_lossy(&human.stdout);
    assert!(stdout.contains("DevImg Doctor"));
    assert!(stdout.contains("Status: pass"));
    assert!(stdout.contains("Next: devimg check --config"));

    let json = run(["doctor", "--config", config.as_str(), "--json"]);
    assert_status(&json, 0);
    let document: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("doctor JSON parses");
    assert_eq!(document["status"], "pass");
    assert_eq!(document["source_image_count"], 1);
    assert_eq!(document["planned_variant_count"], 1);
    assert_eq!(document["generated_variant_count"], 1);
    assert!(document["issues"]
        .as_array()
        .expect("issues array")
        .is_empty());
    cleanup(&project);
}

#[test]
fn doctor_reports_missing_manifest_without_writing() {
    let project = fixture_project("doctor_missing_manifest", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    let output = run(["doctor", "--config", config.as_str(), "--json"]);

    assert_status(&output, 3);
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("doctor JSON parses");
    assert_eq!(document["status"], "fail");
    assert!(document["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .any(|issue| issue["code"] == "missing_manifest"));
    assert!(!project.join("public/images/devimg-manifest.json").exists());
    assert!(!project.join("devimg-report.md").exists());
    assert!(!project.join("public/images/generated").exists());
    cleanup(&project);
}

#[test]
fn doctor_reports_empty_and_missing_source_directories() {
    let empty = temp_project("doctor_empty_source");
    fs::create_dir_all(empty.join("assets/images")).expect("create empty source");
    write_project_config(&empty, 64, "", r#"max_total_bytes = "5mb""#);
    let empty_config = path_arg(empty.join("devimg.toml"));

    let output = run(["doctor", "--config", empty_config.as_str()]);

    assert_status(&output, 3);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("empty_sources"));
    cleanup(&empty);

    let missing = temp_project("doctor_missing_source");
    write_project_config(&missing, 64, "", r#"max_total_bytes = "5mb""#);
    let missing_config = path_arg(missing.join("devimg.toml"));

    let output = run(["doctor", "--config", missing_config.as_str()]);

    assert_status(&output, 3);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing_source_dir"));
    cleanup(&missing);
}

#[test]
fn doctor_reports_stale_output_and_budget_failure() {
    let stale = fixture_project("doctor_stale", "sample.png");
    let stale_config = path_arg(stale.join("devimg.toml"));

    assert_code(run(["optimize", "--config", stale_config.as_str()]), 0);
    write_project_config(&stale, 32, "", r#"max_total_bytes = "5mb""#);

    let output = run(["doctor", "--config", stale_config.as_str()]);

    assert_status(&output, 3);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("outdated_config"));
    assert!(stderr.contains("stale"));
    assert!(stderr.contains("devimg optimize --config"));
    cleanup(&stale);

    let budget = fixture_project_with_budget(
        "doctor_budget_failure",
        "sample.png",
        r#"max_total_bytes = "1b""#,
    );
    let budget_config = path_arg(budget.join("devimg.toml"));

    assert_code(run(["optimize", "--config", budget_config.as_str()]), 0);

    let output = run(["doctor", "--config", budget_config.as_str()]);

    assert_status(&output, 3);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("oversized_total"));
    cleanup(&budget);
}

#[test]
fn doctor_detects_manifest_export_drift() {
    let project = fixture_project("doctor_export_drift", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let manifest = path_arg(project.join("public/images/devimg-manifest.json"));
    let generated = project.join("lib/devimg.generated.ts");
    let generated_arg = path_arg(generated.clone());

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    assert_code(
        run([
            "manifest",
            "export",
            "--manifest",
            manifest.as_str(),
            "--format",
            "typescript",
            "--strip-prefix",
            "public",
            "--url-prefix",
            "/",
            "--typescript-helpers",
            "--output",
            generated_arg.as_str(),
        ]),
        0,
    );

    assert_code(
        run([
            "doctor",
            "--config",
            config.as_str(),
            "--export-output",
            generated_arg.as_str(),
            "--export-format",
            "typescript",
            "--strip-prefix",
            "public",
            "--url-prefix",
            "/",
            "--typescript-helpers",
        ]),
        0,
    );

    fs::write(&generated, "stale\n").expect("write stale generated module");
    let output = run([
        "doctor",
        "--config",
        config.as_str(),
        "--json",
        "--export-output",
        generated_arg.as_str(),
        "--export-format",
        "typescript",
        "--strip-prefix",
        "public",
        "--url-prefix",
        "/",
        "--typescript-helpers",
    ]);

    assert_status(&output, 3);
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("doctor JSON parses");
    assert!(document["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .any(|issue| issue["code"] == "manifest_export_stale"));
    assert_eq!(
        fs::read_to_string(&generated).expect("stale module reads"),
        "stale\n"
    );
    cleanup(&project);
}

#[test]
fn doctor_reports_framework_diagnostics() {
    let next = fixture_project("doctor_framework_next", "sample.png");
    write_package_json(&next, r#"{"dependencies":{"next":"15.0.0"}}"#);
    fs::write(next.join("next.config.js"), "module.exports = {}\n").expect("next config writes");
    let next_config = path_arg(next.join("devimg.toml"));

    assert_code(run(["optimize", "--config", next_config.as_str()]), 0);
    let next_output = run(["doctor", "--config", next_config.as_str(), "--json"]);
    assert_status(&next_output, 0);
    let next_document: serde_json::Value =
        serde_json::from_slice(&next_output.stdout).expect("next doctor JSON parses");
    assert_json_array_contains(&next_document["frameworks"], "next");
    assert_diagnostic_code(
        &next_document["warnings"],
        "framework_next_image_double_optimization",
    );
    assert_diagnostic_code(&next_document["warnings"], "framework_cache_without_hash");
    cleanup(&next);

    let astro = fixture_project_with_project_settings(
        "doctor_framework_astro",
        "sample.png",
        "content_hash_filenames = true",
        r#"max_total_bytes = "5mb""#,
    );
    write_package_json(&astro, r#"{"devDependencies":{"astro":"5.0.0"}}"#);
    let astro_config = path_arg(astro.join("devimg.toml"));

    assert_code(run(["optimize", "--config", astro_config.as_str()]), 0);
    let astro_output = run(["doctor", "--config", astro_config.as_str(), "--json"]);
    assert_status(&astro_output, 0);
    let astro_document: serde_json::Value =
        serde_json::from_slice(&astro_output.stdout).expect("astro doctor JSON parses");
    assert_json_array_contains(&astro_document["frameworks"], "astro");
    assert_diagnostic_code(
        &astro_document["warnings"],
        "framework_manifest_export_missing",
    );
    cleanup(&astro);

    let vite = fixture_project("doctor_framework_vite", "sample.png");
    fs::write(vite.join("vite.config.ts"), "export default {}\n").expect("vite config writes");
    let vite_config = path_arg(vite.join("devimg.toml"));

    assert_code(run(["optimize", "--config", vite_config.as_str()]), 0);
    let vite_output = run(["doctor", "--config", vite_config.as_str(), "--json"]);
    assert_status(&vite_output, 0);
    let vite_document: serde_json::Value =
        serde_json::from_slice(&vite_output.stdout).expect("vite doctor JSON parses");
    assert_json_array_contains(&vite_document["frameworks"], "vite");
    cleanup(&vite);
}

#[test]
fn doctor_reports_framework_consumption_helpers() {
    let project = fixture_project_with_project_settings(
        "doctor_framework_helpers",
        "sample.png",
        "content_hash_filenames = true",
        r#"max_total_bytes = "5mb""#,
    );
    write_package_json(&project, r#"{"dependencies":{"next":"15.0.0"}}"#);
    fs::write(project.join("next.config.mjs"), "export default {}\n").expect("next config writes");
    fs::create_dir_all(project.join("lib")).expect("lib creates");
    fs::write(
        project.join("lib/devimg.generated.ts"),
        "stale generated helper\n",
    )
    .expect("helper writes");
    let config = path_arg(project.join("devimg.toml"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    let json_output = run(["doctor", "--config", config.as_str(), "--json"]);
    assert_status(&json_output, 0);
    let document: serde_json::Value =
        serde_json::from_slice(&json_output.stdout).expect("doctor JSON parses");
    assert_json_array_contains(&document["frameworks"], "next");
    assert_json_array_contains(&document["manifest_helpers"], "lib/devimg.generated.ts");
    assert_diagnostic_code(&document["warnings"], "framework_manifest_helper_unchecked");
    assert!(!document["warnings"]
        .as_array()
        .expect("warnings array")
        .iter()
        .any(|warning| warning["code"] == "framework_manifest_export_missing"));
    assert!(document["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .any(|check| {
            check["name"] == "framework_consumption"
                && check["message"]
                    .as_str()
                    .expect("check message")
                    .contains("img/picture")
                && check["message"]
                    .as_str()
                    .expect("check message")
                    .contains("unoptimized")
                && check["message"]
                    .as_str()
                    .expect("check message")
                    .contains("intentionally layer")
        }));

    let human_output = run(["doctor", "--config", config.as_str()]);
    assert_status(&human_output, 0);
    let stdout = String::from_utf8_lossy(&human_output.stdout);
    assert!(stdout.contains("Manifest helpers: `lib/devimg.generated.ts`"));
    assert!(stdout.contains("img/picture"));
    assert!(stdout.contains("unoptimized"));
    assert!(stdout.contains("intentionally layer"));
    cleanup(&project);
}

#[test]
fn doctor_export_output_suppresses_framework_helper_warning() {
    let project = fixture_project_with_project_settings(
        "doctor_framework_export_verified",
        "sample.png",
        "content_hash_filenames = true",
        r#"max_total_bytes = "5mb""#,
    );
    write_package_json(&project, r#"{"devDependencies":{"vite":"7.0.0"}}"#);
    fs::create_dir_all(project.join("lib")).expect("lib creates");
    let config = path_arg(project.join("devimg.toml"));
    let manifest = path_arg(project.join("public/images/devimg-manifest.json"));
    let generated = path_arg(project.join("lib/devimg.generated.ts"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    assert_code(
        run([
            "manifest",
            "export",
            "--manifest",
            manifest.as_str(),
            "--format",
            "typescript",
            "--strip-prefix",
            "public",
            "--url-prefix",
            "/",
            "--typescript-helpers",
            "--output",
            generated.as_str(),
        ]),
        0,
    );
    let output = run([
        "doctor",
        "--config",
        config.as_str(),
        "--json",
        "--export-output",
        generated.as_str(),
        "--export-format",
        "typescript",
        "--strip-prefix",
        "public",
        "--url-prefix",
        "/",
        "--typescript-helpers",
    ]);
    assert_status(&output, 0);
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("doctor JSON parses");
    assert_json_array_contains(&document["manifest_helpers"], "lib/devimg.generated.ts");
    assert!(!document["warnings"]
        .as_array()
        .expect("warnings array")
        .iter()
        .any(|warning| warning["code"] == "framework_manifest_helper_unchecked"));
    cleanup(&project);
}

#[test]
fn doctor_framework_detection_handles_none_and_mixed_projects() {
    let plain = fixture_project("doctor_framework_none", "sample.png");
    let plain_config = path_arg(plain.join("devimg.toml"));

    assert_code(run(["optimize", "--config", plain_config.as_str()]), 0);
    let plain_output = run(["doctor", "--config", plain_config.as_str(), "--json"]);
    assert_status(&plain_output, 0);
    let plain_document: serde_json::Value =
        serde_json::from_slice(&plain_output.stdout).expect("plain doctor JSON parses");
    assert!(plain_document["frameworks"]
        .as_array()
        .expect("frameworks array")
        .is_empty());
    assert!(!plain_document["warnings"]
        .as_array()
        .expect("warnings array")
        .iter()
        .any(|warning| warning["code"]
            .as_str()
            .expect("warning code")
            .starts_with("framework_")));
    cleanup(&plain);

    let mixed = fixture_project("doctor_framework_mixed", "sample.png");
    write_package_json(
        &mixed,
        r#"{"dependencies":{"next":"15.0.0"},"devDependencies":{"vite":"7.0.0"}}"#,
    );
    let mixed_config = path_arg(mixed.join("devimg.toml"));

    assert_code(run(["optimize", "--config", mixed_config.as_str()]), 0);
    let mixed_output = run(["doctor", "--config", mixed_config.as_str(), "--json"]);
    assert_status(&mixed_output, 0);
    let mixed_document: serde_json::Value =
        serde_json::from_slice(&mixed_output.stdout).expect("mixed doctor JSON parses");
    assert_json_array_contains(&mixed_document["frameworks"], "next");
    assert_json_array_contains(&mixed_document["frameworks"], "vite");
    assert_diagnostic_code(&mixed_document["warnings"], "framework_multiple_detected");
    cleanup(&mixed);
}

#[test]
fn content_hash_filename_mode_is_checkable() {
    let project = fixture_project_with_project_settings(
        "content_hash_filenames",
        "sample.png",
        "content_hash_filenames = true",
        r#"max_total_bytes = "5mb""#,
    );
    let config = path_arg(project.join("devimg.toml"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    let output_path = single_generated_webp(&project);
    let output_name = output_path
        .file_name()
        .expect("output filename")
        .to_string_lossy();
    assert!(output_name.starts_with("sample.project-card.64."));
    assert_ne!(output_name.as_ref(), "sample.project-card.64.webp");
    let manifest = fs::read_to_string(project.join("public/images/devimg-manifest.json"))
        .expect("manifest reads");
    assert!(manifest.contains(output_name.as_ref()));

    assert_code(run(["check", "--config", config.as_str()]), 0);
    fs::remove_file(&output_path).expect("remove hashed output");
    assert_code(run(["check", "--config", config.as_str()]), 3);
    cleanup(&project);
}

#[test]
fn check_fails_with_exit_3_when_output_is_deleted() {
    let project = fixture_project("deleted_output", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    fs::remove_file(project.join("public/images/generated/sample.project-card.64.webp"))
        .expect("remove output");

    let output = run(["check", "--config", config.as_str()]);
    assert_status(&output, 3);
    assert!(String::from_utf8_lossy(&output.stderr).contains("Hint:"));
    cleanup(&project);
}

#[test]
fn unsafe_overwrite_exits_4_unless_explicitly_allowed() {
    let project = fixture_project("unsafe_overwrite", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let output_path = project.join("public/images/generated/sample.project-card.64.webp");
    fs::create_dir_all(output_path.parent().expect("output parent")).expect("create output parent");
    fs::write(&output_path, b"unmanaged").expect("write unmanaged output");

    assert_code(run(["optimize", "--config", config.as_str()]), 4);

    assert_code(
        run(["optimize", "--config", config.as_str(), "--allow-overwrite"]),
        0,
    );
    cleanup(&project);
}

#[test]
fn check_budget_failure_exits_3() {
    let project =
        fixture_project_with_budget("budget_failure", "sample.png", r#"max_total_bytes = "1b""#);
    let config = path_arg(project.join("devimg.toml"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    assert_code(run(["check", "--config", config.as_str()]), 3);
    cleanup(&project);
}

#[test]
fn check_config_change_exits_3_for_stale_outputs() {
    let project = fixture_project("stale_config", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    write_project_config(&project, 32, "", r#"max_total_bytes = "5mb""#);

    let output = run(["check", "--config", config.as_str()]);
    assert_status(&output, 3);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("outdated_config"));
    assert!(stderr.contains("stale"));
    cleanup(&project);
}

#[test]
fn report_manifest_errors_use_stable_exit_codes() {
    let project = temp_project("report_errors");
    let missing = path_arg(project.join("missing-manifest.json"));
    assert_code(run(["report", "--manifest", missing.as_str()]), 1);
    assert_code(
        run(["review", "--manifest", missing.as_str(), "--stdout"]),
        1,
    );

    let malformed = project.join("bad-manifest.json");
    fs::write(&malformed, "{").expect("write malformed manifest");
    let malformed = path_arg(malformed);
    assert_code(run(["report", "--manifest", malformed.as_str()]), 2);
    assert_code(
        run(["review", "--manifest", malformed.as_str(), "--stdout"]),
        2,
    );
    cleanup(&project);
}

#[test]
fn manifest_export_outputs_app_mapping() {
    let project = fixture_project("manifest_export", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let manifest = path_arg(project.join("public/images/devimg-manifest.json"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);

    let json = run([
        "manifest",
        "export",
        "--manifest",
        manifest.as_str(),
        "--strip-prefix",
        "public",
        "--url-prefix",
        "/",
    ]);
    assert_status(&json, 0);
    let stdout = String::from_utf8_lossy(&json.stdout);
    assert!(stdout.contains("\"sources\""));
    assert!(stdout.contains("\"source_path\": \"assets/images/sample.png\""));
    assert!(stdout.contains("\"src\": \"/images/generated/sample.project-card.64.webp\""));
    assert!(stdout.contains("\"fit\": \"cover\""));

    let check_without_output = run([
        "manifest",
        "export",
        "--manifest",
        manifest.as_str(),
        "--check",
    ]);
    assert_status(&check_without_output, 2);
    assert!(
        String::from_utf8_lossy(&check_without_output.stderr).contains("--check requires --output")
    );

    let generated = project.join("lib/devimg.generated.ts");
    let generated_arg = path_arg(generated.clone());
    assert_code(
        run([
            "manifest",
            "export",
            "--manifest",
            manifest.as_str(),
            "--format",
            "typescript",
            "--strip-prefix",
            "public",
            "--url-prefix",
            "/",
            "--output",
            generated_arg.as_str(),
        ]),
        0,
    );
    let typescript = fs::read_to_string(&generated).expect("generated module reads");
    assert!(typescript.starts_with("// Generated by devimg."));
    assert!(typescript.contains("export const DEVIMG_MANIFEST = {"));
    assert!(typescript.contains("src: \"/images/generated/sample.project-card.64.webp\""));
    assert!(!typescript.contains("findDevimgVariant"));

    let helpers = project.join("lib/devimg.helpers.ts");
    let helpers_arg = path_arg(helpers.clone());
    assert_code(
        run([
            "manifest",
            "export",
            "--manifest",
            manifest.as_str(),
            "--format",
            "typescript",
            "--strip-prefix",
            "public",
            "--url-prefix",
            "/",
            "--typescript-helpers",
            "--output",
            helpers_arg.as_str(),
        ]),
        0,
    );
    let helper_typescript = fs::read_to_string(&helpers).expect("generated helpers read");
    assert!(helper_typescript.contains("export type DevimgVariantSelector = {"));
    assert!(helper_typescript.contains("export function findDevimgSource"));
    assert!(helper_typescript.contains("export function listDevimgVariants"));
    assert!(helper_typescript.contains("export function findDevimgVariant"));

    let check_current = run([
        "manifest",
        "export",
        "--manifest",
        manifest.as_str(),
        "--format",
        "typescript",
        "--strip-prefix",
        "public",
        "--url-prefix",
        "/",
        "--output",
        generated_arg.as_str(),
        "--check",
    ]);
    assert_status(&check_current, 0);
    assert!(String::from_utf8_lossy(&check_current.stdout).contains("is up to date"));

    let check_helpers_current = run([
        "manifest",
        "export",
        "--manifest",
        manifest.as_str(),
        "--format",
        "typescript",
        "--strip-prefix",
        "public",
        "--url-prefix",
        "/",
        "--typescript-helpers",
        "--output",
        helpers_arg.as_str(),
        "--check",
    ]);
    assert_status(&check_helpers_current, 0);

    let helper_json_error = run([
        "manifest",
        "export",
        "--manifest",
        manifest.as_str(),
        "--typescript-helpers",
    ]);
    assert_status(&helper_json_error, 2);
    assert!(String::from_utf8_lossy(&helper_json_error.stderr)
        .contains("--typescript-helpers requires --format typescript"));

    fs::write(&generated, "stale\n").expect("write stale generated module");
    let check_stale = run([
        "manifest",
        "export",
        "--manifest",
        manifest.as_str(),
        "--format",
        "typescript",
        "--strip-prefix",
        "public",
        "--url-prefix",
        "/",
        "--output",
        generated_arg.as_str(),
        "--check",
    ]);
    assert_status(&check_stale, 3);
    assert!(String::from_utf8_lossy(&check_stale.stderr).contains("is stale"));
    assert!(String::from_utf8_lossy(&check_stale.stderr).contains("Hint:"));
    assert_eq!(
        fs::read_to_string(&generated).expect("stale module reads"),
        "stale\n"
    );

    fs::remove_file(&generated).expect("remove generated module");
    let check_missing = run([
        "manifest",
        "export",
        "--manifest",
        manifest.as_str(),
        "--format",
        "typescript",
        "--strip-prefix",
        "public",
        "--url-prefix",
        "/",
        "--output",
        generated_arg.as_str(),
        "--check",
    ]);
    assert_status(&check_missing, 3);
    assert!(String::from_utf8_lossy(&check_missing.stderr).contains("is missing"));
    assert!(String::from_utf8_lossy(&check_missing.stderr).contains("Hint:"));
    cleanup(&project);
}

#[test]
fn review_writes_visual_artifact_and_refuses_unsafe_overwrite() {
    let project = fixture_project("review_artifact", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let manifest = path_arg(project.join("public/images/devimg-manifest.json"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);

    let missing_output_mode = run(["review", "--manifest", manifest.as_str()]);
    assert_status(&missing_output_mode, 2);
    assert!(String::from_utf8_lossy(&missing_output_mode.stderr)
        .contains("exactly one of --output or --stdout"));

    let stdout = run(["review", "--manifest", manifest.as_str(), "--stdout"]);
    assert_status(&stdout, 0);
    let stdout_html = String::from_utf8_lossy(&stdout.stdout);
    assert!(stdout_html.starts_with("<!doctype html>"));
    assert!(stdout_html.contains("DevImg visual review"));
    assert!(stdout_html.contains("assets/images/sample.png"));
    assert!(stdout_html.contains("public/images/generated/sample.project-card.64.webp"));
    assert!(!project.join(".devimg/review.html").exists());

    let review = project.join(".devimg/review.html");
    let review_arg = path_arg(review.clone());
    assert_code(
        run([
            "review",
            "--manifest",
            manifest.as_str(),
            "--output",
            review_arg.as_str(),
        ]),
        0,
    );
    let html = fs::read_to_string(&review).expect("review artifact reads");
    assert!(html.contains("src=\"../public/images/generated/sample.project-card.64.webp\""));
    assert!(html.contains("href=\"../assets/images/sample.png\""));

    fs::write(&review, "existing\n").expect("write existing review artifact");
    let refused = run([
        "review",
        "--manifest",
        manifest.as_str(),
        "--output",
        review_arg.as_str(),
    ]);
    assert_status(&refused, 4);
    assert_eq!(
        fs::read_to_string(&review).expect("review artifact reads"),
        "existing\n"
    );

    assert_code(
        run([
            "review",
            "--manifest",
            manifest.as_str(),
            "--output",
            review_arg.as_str(),
            "--force",
        ]),
        0,
    );
    assert!(fs::read_to_string(&review)
        .expect("forced review artifact reads")
        .contains("Generated image variants"));
    cleanup(&project);
}

#[test]
fn dogfood_example_flow_covers_frontend_assets() {
    let project = temp_project("dogfood_example");
    copy_dir_all(&repo_root().join("examples/dogfood"), &project);
    remove_dir_if_exists(&project.join("public"));
    remove_dir_if_exists(&project.join("lib"));
    remove_dir_if_exists(&project.join(".devimg"));
    remove_file_if_exists(&project.join("devimg-report.md"));
    let config = path_arg(project.join("devimg.toml"));
    let manifest_path = project.join("public/images/devimg-manifest.json");
    let manifest = path_arg(manifest_path.clone());
    let helper = path_arg(project.join("lib/devimg.generated.ts"));
    let review = project.join(".devimg/review.html");
    let review_arg = path_arg(review.clone());

    let missing = run(["doctor", "--config", config.as_str(), "--json"]);
    assert_status(&missing, 3);
    let missing_json: serde_json::Value =
        serde_json::from_slice(&missing.stdout).expect("missing doctor JSON parses");
    assert_diagnostic_code(&missing_json["issues"], "missing_manifest");
    assert!(!manifest_path.exists());

    assert_code(
        run(["optimize", "--config", config.as_str(), "--dry-run"]),
        0,
    );
    assert!(!manifest_path.exists());

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    assert_code(run(["check", "--config", config.as_str()]), 0);

    let manifest_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).expect("manifest reads"))
            .expect("manifest JSON parses");
    let outputs = manifest_json["outputs"]
        .as_array()
        .expect("manifest outputs are an array");
    assert!(outputs.iter().any(|output| {
        output["source_path"] == "assets/images/logos/devimg-mark.png"
            && output["preset"] == "logo-contain"
            && output["fit"] == "contain"
    }));
    assert!(outputs.iter().any(|output| {
        output["output_path"]
            .as_str()
            .expect("output path is string")
            .contains(".project-card.640.")
    }));

    assert_code(
        run([
            "manifest",
            "export",
            "--manifest",
            manifest.as_str(),
            "--format",
            "typescript",
            "--strip-prefix",
            "public",
            "--url-prefix",
            "/",
            "--typescript-helpers",
            "--output",
            helper.as_str(),
        ]),
        0,
    );
    assert_code(
        run([
            "manifest",
            "export",
            "--manifest",
            manifest.as_str(),
            "--format",
            "typescript",
            "--strip-prefix",
            "public",
            "--url-prefix",
            "/",
            "--typescript-helpers",
            "--output",
            helper.as_str(),
            "--check",
        ]),
        0,
    );
    assert_code(
        run([
            "doctor",
            "--config",
            config.as_str(),
            "--export-output",
            helper.as_str(),
            "--export-format",
            "typescript",
            "--strip-prefix",
            "public",
            "--url-prefix",
            "/",
            "--typescript-helpers",
        ]),
        0,
    );
    assert_code(
        run([
            "review",
            "--manifest",
            manifest.as_str(),
            "--output",
            review_arg.as_str(),
        ]),
        0,
    );
    let html = fs::read_to_string(&review).expect("review artifact reads");
    assert!(html.contains("assets/images/social/open-graph.png"));
    assert!(html.contains("logo-contain"));
    cleanup(&project);
}

#[test]
fn compare_reports_manifest_diffs_for_people_and_json() {
    let project = temp_project("compare");
    let base = project.join("base-manifest.json");
    let head = project.join("head-manifest.json");
    fs::write(
        &base,
        compare_manifest_json(
            "unix:1",
            "blake3:base",
            &[
                compare_output_json((
                    "assets/card.png",
                    "project-card",
                    640,
                    360,
                    "webp",
                    100,
                    "same",
                    "public/images/generated/card.project-card.640.webp",
                )),
                compare_output_json((
                    "assets/card.png",
                    "project-card",
                    960,
                    540,
                    "webp",
                    200,
                    "old",
                    "public/images/generated/card.project-card.960.webp",
                )),
                compare_output_json((
                    "assets/card.png",
                    "project-card",
                    1280,
                    720,
                    "webp",
                    300,
                    "removed",
                    "public/images/generated/card.project-card.1280.webp",
                )),
            ],
        ),
    )
    .expect("write base manifest");
    fs::write(
        &head,
        compare_manifest_json(
            "unix:2",
            "blake3:head",
            &[
                compare_output_json((
                    "assets/card.png",
                    "project-card",
                    640,
                    360,
                    "webp",
                    100,
                    "same",
                    "public/images/generated/card.project-card.640.webp",
                )),
                compare_output_json((
                    "assets/card.png",
                    "project-card",
                    960,
                    540,
                    "webp",
                    260,
                    "new",
                    "public/images/generated/card.project-card.960.newhash.webp",
                )),
                compare_output_json((
                    "assets/avatar.png",
                    "avatar",
                    256,
                    256,
                    "jpeg",
                    50,
                    "added",
                    "public/images/generated/avatar.avatar.256.jpeg",
                )),
            ],
        ),
    )
    .expect("write head manifest");
    let base_arg = path_arg(base);
    let head_arg = path_arg(head);

    let human = run([
        "compare",
        "--base",
        base_arg.as_str(),
        "--head",
        head_arg.as_str(),
    ]);
    assert_status(&human, 0);
    let stdout = String::from_utf8_lossy(&human.stdout);
    assert!(stdout.contains("# Dev Image Pipeline Compare Report"));
    assert!(stdout.contains("- Variants: `3` -> `3` (`0`)"));
    assert!(stdout.contains("- Output bytes: `600` -> `410` (`-190`)"));
    assert!(stdout.contains("- Added outputs: `1`"));
    assert!(stdout.contains("- Removed outputs: `1`"));
    assert!(stdout.contains("- Changed outputs: `1`"));
    assert!(stdout.contains("- Metadata-only output changes: `0`"));
    assert!(stdout.contains("- Unchanged outputs: `1`"));
    assert!(stdout.contains("card.project-card.960.webp` -> `public/images/generated/card.project-card.960.newhash.webp"));
    assert!(stdout.contains("## Top Byte Contributors"));

    let json = run([
        "compare",
        "--base",
        base_arg.as_str(),
        "--head",
        head_arg.as_str(),
        "--json",
        "--top",
        "1",
    ]);
    assert_status(&json, 0);
    let document: serde_json::Value =
        serde_json::from_slice(&json.stdout).expect("compare JSON parses");
    assert_eq!(document["summary"]["output_bytes_delta"], -190);
    assert_eq!(document["summary"]["added_count"], 1);
    assert_eq!(document["summary"]["removed_count"], 1);
    assert_eq!(document["summary"]["changed_count"], 1);
    assert_eq!(document["summary"]["metadata_changed_count"], 0);
    assert_eq!(document["summary"]["unchanged_count"], 1);
    let top = document["top_byte_contributors"]
        .as_array()
        .expect("top contributors array");
    assert_eq!(top.len(), 1);
    assert_eq!(top[0]["bytes"], 260);
    cleanup(&project);
}

#[test]
fn inspect_exit_codes_and_multiple_files_are_stable() {
    let sample = path_arg(fixture_image());
    let card = path_arg(repo_root().join("examples/portfolio/assets/images/card.png"));

    let single = run(["inspect", sample.as_str()]);
    assert_status(&single, 0);
    let stdout = String::from_utf8_lossy(&single.stdout);
    assert!(stdout.contains("dimensions: 640x360"));
    assert!(stdout.contains("hash: blake3:"));

    let multiple = run(["inspect", sample.as_str(), card.as_str()]);
    assert_status(&multiple, 0);
    let stdout = String::from_utf8_lossy(&multiple.stdout);
    assert!(stdout.contains("fixtures/images/sample.png:"));
    assert!(stdout.contains("examples/portfolio/assets/images/card.png:"));

    assert_code(run(["inspect"]), 2);

    let missing = path_arg(repo_root().join("fixtures/images/missing.png"));
    assert_code(run(["inspect", missing.as_str()]), 1);
}

#[test]
fn check_fail_on_warning_turns_warning_into_exit_3() {
    let project = fixture_project_with_project_settings(
        "fail_on_warning",
        "sample.png",
        "strip_metadata = false",
        r#"max_total_bytes = "5mb""#,
    );
    let config = path_arg(project.join("devimg.toml"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    assert_code(run(["check", "--config", config.as_str()]), 0);
    assert_code(
        run(["check", "--config", config.as_str(), "--fail-on-warning"]),
        3,
    );
    cleanup(&project);
}

#[test]
fn quality_diagnostics_are_reported_and_can_fail_strict_check() {
    let project = temp_project("quality_diagnostics");
    let source = project.join("assets/images/hero-screenshot.png");
    fs::create_dir_all(source.parent().expect("source parent")).expect("create source parent");
    fs::copy(fixture_image(), &source).expect("copy fixture image");
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
name = "hero"
widths = [64]
formats = ["webp"]
quality = 74
fit = "cover"
aspect_ratio = "16:9"

[budgets]
max_total_bytes = "5mb"
"#,
    )
    .expect("write quality config");
    let config = path_arg(project.join("devimg.toml"));

    let optimize = run(["optimize", "--config", config.as_str()]);
    assert_status(&optimize, 0);
    assert!(String::from_utf8_lossy(&optimize.stdout).contains("quality:"));
    assert!(fs::read_to_string(project.join("devimg-report.md"))
        .expect("report reads")
        .contains("quality:"));

    let default_check = run(["check", "--config", config.as_str()]);
    assert_status(&default_check, 0);
    assert!(String::from_utf8_lossy(&default_check.stdout).contains("quality:"));

    let strict_check = run(["check", "--config", config.as_str(), "--fail-on-warning"]);
    assert_status(&strict_check, 3);
    assert!(String::from_utf8_lossy(&strict_check.stderr).contains("quality:"));

    let doctor = run(["doctor", "--config", config.as_str(), "--json"]);
    assert_status(&doctor, 0);
    let document: serde_json::Value =
        serde_json::from_slice(&doctor.stdout).expect("doctor JSON parses");
    assert!(document["warnings"]
        .as_array()
        .expect("warnings array")
        .iter()
        .any(|warning| warning["code"] == "quality:low-lossy-quality"));
    cleanup(&project);
}

#[test]
fn acknowledged_crop_warnings_remain_visible_but_do_not_fail_strict_check() {
    let project = temp_project("acknowledged_crop_warning");
    let source = project.join("assets/images/accesstrace.png");
    fs::create_dir_all(source.parent().expect("source parent")).expect("create source parent");
    fs::copy(fixture_image(), &source).expect("copy fixture image");
    let config_path = project.join("devimg.toml");
    fs::write(&config_path, crop_warning_config("")).expect("write unacknowledged config");
    let config = path_arg(config_path.clone());

    let optimize = run(["optimize", "--config", config.as_str()]);
    assert_status(&optimize, 0);
    assert!(String::from_utf8_lossy(&optimize.stdout).contains("quality:cover-crop"));
    let strict_check = run(["check", "--config", config.as_str(), "--fail-on-warning"]);
    assert_status(&strict_check, 3);
    assert!(String::from_utf8_lossy(&strict_check.stderr).contains("quality:cover-crop"));
    let doctor = run(["doctor", "--config", config.as_str(), "--json"]);
    assert_status(&doctor, 0);
    let document: serde_json::Value =
        serde_json::from_slice(&doctor.stdout).expect("doctor JSON parses");
    assert_diagnostic_code(&document["warnings"], "quality:cover-crop");
    assert!(document["acknowledged_warnings"]
        .as_array()
        .expect("acknowledged warnings array")
        .is_empty());

    fs::write(
        &config_path,
        crop_warning_config(
            r#"
[[warnings.acknowledge]]
code = "quality:cover-crop"
source = "assets/images/accesstrace.png"
preset = "project-card"
reason = "Intentional project-card framing for this asset."
"#,
        ),
    )
    .expect("write acknowledged config");
    assert_status(&run(["optimize", "--config", config.as_str()]), 0);
    let strict_check = run(["check", "--config", config.as_str(), "--fail-on-warning"]);
    assert_status(&strict_check, 0);
    let stdout = String::from_utf8_lossy(&strict_check.stdout);
    assert!(stdout.contains("Acknowledged Warnings"));
    assert!(stdout.contains("quality:cover-crop"));

    let doctor = run(["doctor", "--config", config.as_str(), "--json"]);
    assert_status(&doctor, 0);
    let document: serde_json::Value =
        serde_json::from_slice(&doctor.stdout).expect("doctor JSON parses");
    assert!(!document["warnings"]
        .as_array()
        .expect("warnings array")
        .iter()
        .any(|warning| warning["code"] == "quality:cover-crop"));
    assert_diagnostic_code(&document["acknowledged_warnings"], "quality:cover-crop");
    cleanup(&project);
}

#[test]
fn init_stdout_refusal_and_force_are_stable() {
    let project = temp_project("init");
    let config_path = project.join("nested/devimg.toml");
    let config = path_arg(config_path.clone());

    let stdout = run(["init", "--config", config.as_str(), "--stdout"]);
    assert_status(&stdout, 0);
    let default_config = String::from_utf8_lossy(&stdout.stdout);
    assert!(default_config.contains("[project]"));
    assert!(default_config.contains("name = \"portfolio\""));
    assert!(default_config.contains("input = \"assets/images\""));
    assert!(!config_path.exists());

    assert_code(run(["init", "--config", config.as_str()]), 0);
    assert!(config_path.exists());
    assert_code(run(["init", "--config", config.as_str()]), 4);
    assert_code(run(["init", "--config", config.as_str(), "--force"]), 0);
    cleanup(&project);
}

#[test]
fn init_profiles_emit_framework_paths_and_parse() {
    let cases = [
        ("next", "next", "public/images/source"),
        ("astro", "astro", "src/assets/images"),
        ("vite", "vite", "src/assets/images"),
    ];

    for (profile, source_name, input_dir) in cases {
        let project = temp_project(&format!("init_profile_{profile}"));
        let config_path = project.join("devimg.toml");
        let config = path_arg(config_path.clone());

        let stdout = run([
            "init",
            "--profile",
            profile,
            "--config",
            config.as_str(),
            "--stdout",
        ]);
        assert_status(&stdout, 0);
        let rendered = String::from_utf8_lossy(&stdout.stdout);
        assert!(rendered.contains(&format!("name = \"{source_name}\"")));
        assert!(rendered.contains(&format!("input = \"{input_dir}\"")));
        assert!(rendered.contains("output = \"public/images/generated\""));
        assert!(!config_path.exists());

        assert_code(
            run(["init", "--profile", profile, "--config", config.as_str()]),
            0,
        );
        let source_path = project.join(input_dir).join("sample.png");
        fs::create_dir_all(source_path.parent().expect("source parent"))
            .expect("create profile source dir");
        fs::copy(fixture_image(), &source_path).expect("copy fixture image");

        assert_code(
            run(["optimize", "--config", config.as_str(), "--dry-run"]),
            0,
        );
        cleanup(&project);
    }
}

#[test]
fn init_profile_refusal_force_and_unknown_profile_are_stable() {
    let project = temp_project("init_profile_write");
    let config_path = project.join("nested/devimg.toml");
    let config = path_arg(config_path.clone());

    assert_code(
        run(["init", "--profile", "next", "--config", config.as_str()]),
        0,
    );
    assert!(config_path.exists());
    let written = fs::read_to_string(&config_path).expect("profile config reads");
    assert!(written.contains("input = \"public/images/source\""));

    assert_code(
        run(["init", "--profile", "next", "--config", config.as_str()]),
        4,
    );
    assert_code(
        run([
            "init",
            "--profile",
            "next",
            "--config",
            config.as_str(),
            "--force",
        ]),
        0,
    );

    let invalid = run(["init", "--profile", "rails", "--stdout"]);
    assert_status(&invalid, 2);
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid value"));
    cleanup(&project);
}

#[test]
fn agent_init_creates_codex_claude_and_both_targets_safely() {
    let codex_project = temp_project("agent_codex");
    let codex_output = path_arg(codex_project.clone());
    assert_code(
        run([
            "agent",
            "init",
            "--target",
            "codex",
            "--output-dir",
            codex_output.as_str(),
        ]),
        0,
    );
    let agents = codex_project.join("AGENTS.md");
    assert!(agents.exists());
    let agents_text = fs::read_to_string(&agents).expect("AGENTS.md reads");
    assert!(agents_text.contains("devimg doctor --config devimg.toml"));
    assert!(agents_text.contains("devimg optimize --config devimg.toml --allow-overwrite"));
    assert!(agents_text.contains("devimg manifest export"));
    assert!(agents_text.contains("devimg check --config devimg.toml"));
    assert!(agents_text.contains("docs/agent-contract.md"));
    assert!(agents_text.contains("Do not edit generated"));
    cleanup(&codex_project);

    let claude_project = temp_project("agent_claude");
    let claude_output = path_arg(claude_project.clone());
    assert_code(
        run([
            "agent",
            "init",
            "--target",
            "claude",
            "--output-dir",
            claude_output.as_str(),
        ]),
        0,
    );
    assert!(claude_project.join("CLAUDE.md").exists());
    assert!(claude_project
        .join(".claude/commands/devimg-doctor.md")
        .exists());
    cleanup(&claude_project);

    let both_project = temp_project("agent_both");
    let both_output = path_arg(both_project.clone());
    assert_code(
        run([
            "agent",
            "init",
            "--target",
            "both",
            "--output-dir",
            both_output.as_str(),
        ]),
        0,
    );
    assert!(both_project.join("AGENTS.md").exists());
    assert!(both_project.join("CLAUDE.md").exists());
    assert!(both_project
        .join(".claude/commands/devimg-doctor.md")
        .exists());
    cleanup(&both_project);
}

#[test]
fn agent_init_refuses_existing_files_unless_forced() {
    let project = temp_project("agent_existing");
    fs::write(project.join("AGENTS.md"), "existing\n").expect("write existing agent file");
    let output_dir = path_arg(project.clone());

    let refused = run([
        "agent",
        "init",
        "--target",
        "both",
        "--output-dir",
        output_dir.as_str(),
    ]);
    assert_status(&refused, 4);
    assert_eq!(
        fs::read_to_string(project.join("AGENTS.md")).expect("AGENTS reads"),
        "existing\n"
    );
    assert!(!project.join("CLAUDE.md").exists());

    assert_code(
        run([
            "agent",
            "init",
            "--target",
            "both",
            "--output-dir",
            output_dir.as_str(),
            "--force",
        ]),
        0,
    );
    assert!(fs::read_to_string(project.join("AGENTS.md"))
        .expect("AGENTS reads")
        .contains("DevImg Agent Instructions"));
    assert!(project.join("CLAUDE.md").exists());
    cleanup(&project);
}

#[test]
fn agent_init_stdout_and_invalid_target_are_stable() {
    let project = temp_project("agent_stdout");
    let output_dir = path_arg(project.clone());

    let output = run([
        "agent",
        "init",
        "--target",
        "both",
        "--output-dir",
        output_dir.as_str(),
        "--config",
        "config/devimg.toml",
        "--stdout",
    ]);
    assert_status(&output, 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# AGENTS.md"));
    assert!(stdout.contains("# CLAUDE.md"));
    assert!(stdout.contains("# .claude/commands/devimg-doctor.md"));
    assert!(stdout.contains("devimg doctor --config config/devimg.toml"));
    assert!(!project.join("AGENTS.md").exists());
    assert!(!project.join("CLAUDE.md").exists());

    let invalid = run(["agent", "init", "--target", "cursor", "--stdout"]);
    assert_status(&invalid, 2);
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid value"));
    cleanup(&project);
}

fn run<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_devimg"))
        .args(args)
        .output()
        .expect("devimg runs")
}

fn assert_code(output: Output, expected: i32) {
    assert_status(&output, expected);
}

fn assert_status(output: &Output, expected: i32) {
    let actual = output.status.code();
    assert_eq!(
        actual,
        Some(expected),
        "expected exit {expected}, got {actual:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_json_array_contains(value: &serde_json::Value, expected: &str) {
    assert!(
        value
            .as_array()
            .expect("JSON value is an array")
            .iter()
            .any(|entry| entry == expected),
        "expected array to contain {expected}, got {value}"
    );
}

fn assert_diagnostic_code(value: &serde_json::Value, expected: &str) {
    assert!(
        value
            .as_array()
            .expect("diagnostics value is an array")
            .iter()
            .any(|entry| entry["code"] == expected),
        "expected diagnostics to contain {expected}, got {value}"
    );
}

fn fixture_project(label: &str, source_name: &str) -> PathBuf {
    fixture_project_with_budget(label, source_name, r#"max_total_bytes = "5mb""#)
}

fn fixture_project_with_budget(label: &str, source_name: &str, budget_line: &str) -> PathBuf {
    fixture_project_with_project_settings(label, source_name, "", budget_line)
}

fn fixture_project_with_project_settings(
    label: &str,
    source_name: &str,
    project_settings: &str,
    budget_line: &str,
) -> PathBuf {
    let project = temp_project(label);
    let source_path = project.join("assets/images").join(source_name);
    fs::create_dir_all(source_path.parent().expect("source parent")).expect("create source parent");
    fs::copy(fixture_image(), &source_path).expect("copy fixture image");
    write_project_config(&project, 64, project_settings, budget_line);
    project
}

fn config_text_with_width(width: u32, project_settings: &str, budget_line: &str) -> String {
    format!(
        r#"[project]
root = "."
manifest = "public/images/devimg-manifest.json"
report = "devimg-report.md"
{project_settings}

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"
include = ["**/*.png"]

[[preset]]
name = "project-card"
widths = [{width}]
formats = ["webp"]
quality = 82
fit = "cover"
aspect_ratio = "16:9"

[budgets]
{budget_line}
"#
    )
}

fn write_project_config(project: &Path, width: u32, project_settings: &str, budget_line: &str) {
    fs::write(
        project.join("devimg.toml"),
        config_text_with_width(width, project_settings, budget_line),
    )
    .expect("write config");
}

fn crop_warning_config(warning_settings: &str) -> String {
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
widths = [120]
formats = ["webp"]
quality = 90
fit = "cover"
aspect_ratio = "1:1"

[budgets]
max_total_bytes = "5mb"
{warning_settings}
"#
    )
}

fn fixture_image() -> PathBuf {
    repo_root().join("fixtures/images/sample.png")
}

fn write_package_json(project: &Path, contents: &str) {
    fs::write(project.join("package.json"), contents).expect("write package.json");
}

fn single_generated_webp(project: &Path) -> PathBuf {
    let generated = project.join("public/images/generated");
    fs::read_dir(generated)
        .expect("generated dir exists")
        .map(|entry| entry.expect("generated entry").path())
        .find(|path| path.extension() == Some(OsStr::new("webp")))
        .expect("generated webp exists")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn temp_project(label: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("devimg_cli_{label}_{}_{}", std::process::id(), now));
    fs::create_dir_all(&path).expect("create temp project");
    path
}

fn copy_dir_all(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create destination dir");
    for entry in fs::read_dir(from).expect("read source dir") {
        let entry = entry.expect("source dir entry");
        let file_type = entry.file_type().expect("entry file type");
        let destination = to.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).expect("copy file");
        }
    }
}

fn remove_dir_if_exists(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).expect("remove dir");
    }
}

fn remove_file_if_exists(path: &Path) {
    if path.exists() {
        fs::remove_file(path).expect("remove file");
    }
}

fn path_arg(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

fn compare_manifest_json(generated_at: &str, config_hash: &str, outputs: &[String]) -> String {
    format!(
        r#"{{
  "version": 1,
  "generated_at": "{generated_at}",
  "config_path": "devimg.toml",
  "config_hash": "{config_hash}",
  "outputs": [
    {}
  ]
}}
"#,
        outputs.join(",\n    ")
    )
}

fn compare_output_json(output: (&str, &str, u32, u32, &str, u64, &str, &str)) -> String {
    let (source_path, preset, width, height, format, bytes, hash_suffix, output_path) = output;
    format!(
        r#"{{
      "source_path": "{source_path}",
      "source_hash": "blake3:source-{hash_suffix}",
      "source_width": 1600,
      "source_height": 900,
      "source_bytes": 1000,
      "output_path": "{output_path}",
      "preset": "{preset}",
      "fit": "cover",
      "width": {width},
      "height": {height},
      "format": "{format}",
      "bytes": {bytes},
      "hash": "blake3:{hash_suffix}",
      "operation_hash": "blake3:operation-{hash_suffix}"
    }}"#
    )
}

fn cleanup(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).expect("cleanup temp project");
    }
}
