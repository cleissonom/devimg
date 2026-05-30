use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn help_and_usage_exit_codes_are_stable() {
    assert_code(run(["--help"]), 0);
    assert_code(run(["doctor", "--help"]), 0);
    assert_code(run(["agent", "--help"]), 0);
    assert_code(run(["agent", "task", "--help"]), 0);
    assert_code(run(["ai", "--help"]), 0);
    assert_code(run(["ai", "consent", "--help"]), 0);
    assert_code(run(["suggest", "--help"]), 0);
    assert_code(run(["compare", "--help"]), 0);
    assert_code(run(["review", "--help"]), 0);
    let version = run(["--version"]);
    assert_status(&version, 0);
    assert!(String::from_utf8_lossy(&version.stdout).contains("0.2.5"));
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

    let optimize = run(["optimize", "--config", config.as_str()]);
    assert_status(&optimize, 0);
    assert!(!String::from_utf8_lossy(&optimize.stdout).contains("devimg suggest --metadata-only"));
    assert!(project
        .join("public/images/generated/sample.project-card.64.webp")
        .exists());
    assert!(project.join("public/images/devimg-manifest.json").exists());
    assert!(project.join("devimg-report.md").exists());

    assert_code(run(["check", "--config", config.as_str()]), 0);
    cleanup(&project);
}

#[test]
fn default_config_commands_work_from_project_root() {
    let project = fixture_project("default_config", "sample.png");

    assert_code(run_in_dir(&project, ["optimize"]), 0);
    assert!(project
        .join("public/images/generated/sample.project-card.64.webp")
        .exists());
    assert_code(run_in_dir(&project, ["check"]), 0);

    let doctor = run_in_dir(&project, ["doctor"]);
    assert_status(&doctor, 0);
    let stdout = String::from_utf8_lossy(&doctor.stdout);
    assert!(stdout.contains("Next: devimg check"));
    assert!(!stdout.contains("Next: devimg check --config devimg.toml"));
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
fn review_ai_dry_run_writes_json_and_markdown_without_keys() {
    let project = fixture_project("review_ai_dry_run", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let manifest = path_arg(project.join("public/images/devimg-manifest.json"));
    let ai_output = project.join("ai-review.json");
    let ai_output_arg = path_arg(ai_output.clone());
    let markdown = project.join("ai-review.md");
    let markdown_arg = path_arg(markdown.clone());
    let fake_key = "test-openai-secret-do-not-leak";

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    let output = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .args([
            "review",
            "--manifest",
            manifest.as_str(),
            "--ai",
            "--ai-provider",
            "openai",
            "--model",
            "dry-run-model",
            "--dry-run",
            "--ai-output",
            ai_output_arg.as_str(),
            "--markdown",
            markdown_arg.as_str(),
        ])
        .env("OPENAI_API_KEY", fake_key)
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");

    assert_status(&output, 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains(fake_key));
    assert!(!stderr.contains(fake_key));
    let document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&ai_output).expect("AI review JSON reads"))
            .expect("AI review JSON parses");
    assert_eq!(document["provider"], "openai");
    assert_eq!(document["model"], "dry-run-model");
    assert_eq!(document["mode"], "metadata-only");
    assert_eq!(document["dry_run"], true);
    assert_eq!(document["provider_called"], false);
    assert_eq!(document["image_bytes_included"], false);
    assert_eq!(
        document["outputs"][0]["output_path"],
        "public/images/generated/sample.project-card.64.webp"
    );
    let json = fs::read_to_string(&ai_output).expect("AI review JSON reads");
    assert!(!json.contains(fake_key));
    assert!(!json.contains("OPENAI_API_KEY"));
    let markdown = fs::read_to_string(markdown).expect("AI review Markdown reads");
    assert!(markdown.contains("# DevImg AI Review"));
    assert!(markdown.contains("Dry run: `true`"));
    assert!(!markdown.contains(fake_key));
    cleanup(&project);
}

#[test]
fn review_ai_include_images_default_output_and_overwrite_are_stable() {
    let project = fixture_project("review_ai_include_images", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    assert_code(run(["optimize", "--config", config.as_str()]), 0);

    let output = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "review",
            "--manifest",
            "public/images/devimg-manifest.json",
            "--ai",
            "--ai-provider",
            "openai",
            "--model",
            "dry-run-model",
            "--dry-run",
            "--include-images",
            "--max-images",
            "1",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");

    assert_status(&output, 0);
    let ai_output = project.join("devimg-ai-review.json");
    let document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&ai_output).expect("AI review JSON reads"))
            .expect("AI review JSON parses");
    assert_eq!(document["mode"], "include-images");
    assert_eq!(document["image_bytes_included"], true);
    assert_eq!(document["summary"]["selected_image_count"], 1);
    assert_eq!(document["selected_images"][0]["mime_type"], "image/webp");
    assert_eq!(document["selected_images"][0]["detail"], "low");
    assert!(!fs::read_to_string(&ai_output)
        .expect("AI review JSON reads")
        .contains("base64"));

    let refused = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "review",
            "--manifest",
            "public/images/devimg-manifest.json",
            "--ai",
            "--ai-provider",
            "openai",
            "--model",
            "dry-run-model",
            "--dry-run",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&refused, 4);
    cleanup(&project);
}

#[test]
fn review_ai_validation_and_missing_key_errors_are_stable() {
    let project = fixture_project("review_ai_validation", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    assert_code(run(["optimize", "--config", config.as_str()]), 0);

    let missing_key = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "review",
            "--manifest",
            "public/images/devimg-manifest.json",
            "--ai",
            "--ai-provider",
            "openai",
            "--model",
            "openai-model",
            "--ai-output",
            "/tmp/devimg-test-missing-key-ai-review.json",
            "--force",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&missing_key, 2);
    let stderr = String::from_utf8_lossy(&missing_key.stderr);
    assert!(stderr.contains("OPENAI_API_KEY"));
    assert!(!stderr.contains("ANTHROPIC_API_KEY"));

    let anthropic = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "review",
            "--manifest",
            "public/images/devimg-manifest.json",
            "--ai",
            "--ai-provider",
            "anthropic",
            "--model",
            "anthropic-model",
            "--dry-run",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&anthropic, 2);
    assert!(String::from_utf8_lossy(&anthropic.stderr).contains("supports openai only"));

    let invalid_mode = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "review",
            "--manifest",
            "public/images/devimg-manifest.json",
            "--ai",
            "--ai-provider",
            "openai",
            "--model",
            "openai-model",
            "--metadata-only",
            "--include-images",
            "--dry-run",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&invalid_mode, 2);
    assert!(String::from_utf8_lossy(&invalid_mode.stderr)
        .contains("--metadata-only cannot be combined with --include-images"));
    cleanup(&project);
}

#[test]
fn alt_metadata_only_writes_json_and_markdown_without_keys() {
    let project = fixture_project("alt_metadata_only", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let alt_output = project.join("alt.json");
    let alt_output_arg = path_arg(alt_output.clone());
    let markdown = project.join("alt.md");
    let markdown_arg = path_arg(markdown.clone());
    let fake_key = "test-openai-alt-secret-do-not-leak";

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    let output = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .args([
            "alt",
            "--config",
            config.as_str(),
            "--ai-provider",
            "openai",
            "--model",
            "metadata-model",
            "--output",
            alt_output_arg.as_str(),
            "--markdown",
            markdown_arg.as_str(),
        ])
        .env("OPENAI_API_KEY", fake_key)
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");

    assert_status(&output, 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains(fake_key));
    assert!(!stderr.contains(fake_key));
    let json = fs::read_to_string(&alt_output).expect("alt JSON reads");
    assert!(!json.contains(fake_key));
    assert!(!json.contains("OPENAI_API_KEY"));
    let document: serde_json::Value = serde_json::from_str(&json).expect("alt JSON parses");
    assert_eq!(document["provider"], "openai");
    assert_eq!(document["model"], "metadata-model");
    assert_eq!(document["command"], "devimg alt");
    assert_eq!(document["mode"], "metadata-only");
    assert_eq!(document["provider_called"], false);
    assert_eq!(document["image_bytes_included"], false);
    assert_eq!(document["summary"]["source_count"], 1);
    assert_eq!(
        document["drafts"][0]["source_path"],
        "assets/images/sample.png"
    );
    assert_eq!(document["drafts"][0]["candidate_alt_text"], "");
    assert_eq!(document["drafts"][0]["warnings"][0], "needs-human-review");
    let markdown = fs::read_to_string(markdown).expect("alt Markdown reads");
    assert!(markdown.contains("# DevImg Alt-Text Drafts"));
    assert!(markdown.contains("Provider called: `false`"));
    assert!(!markdown.contains(fake_key));
    cleanup(&project);
}

#[test]
fn alt_include_images_dry_run_default_output_and_overwrite_are_stable() {
    let project = fixture_project("alt_include_images", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    assert_code(run(["optimize", "--config", config.as_str()]), 0);

    let output = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "alt",
            "--ai-provider",
            "openai",
            "--model",
            "dry-run-model",
            "--dry-run",
            "--include-images",
            "--max-images",
            "1",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");

    assert_status(&output, 0);
    let alt_output = project.join("devimg-alt.json");
    let document: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&alt_output).expect("alt JSON reads"))
            .expect("alt JSON parses");
    assert_eq!(document["mode"], "include-images");
    assert_eq!(document["image_bytes_included"], true);
    assert_eq!(document["provider_called"], false);
    assert_eq!(document["summary"]["selected_image_count"], 1);
    assert_eq!(
        document["selected_images"][0]["image_path"],
        "assets/images/sample.png"
    );
    assert_eq!(document["selected_images"][0]["mime_type"], "image/png");
    assert_eq!(document["selected_images"][0]["detail"], "low");
    assert!(!fs::read_to_string(&alt_output)
        .expect("alt JSON reads")
        .contains("base64"));

    let refused = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "alt",
            "--ai-provider",
            "openai",
            "--model",
            "dry-run-model",
            "--dry-run",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&refused, 4);
    cleanup(&project);
}

#[test]
fn alt_validation_and_missing_key_errors_are_stable() {
    let project = fixture_project("alt_validation", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    assert_code(run(["optimize", "--config", config.as_str()]), 0);

    let missing_key = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "alt",
            "--ai-provider",
            "openai",
            "--model",
            "openai-model",
            "--include-images",
            "--output",
            "/tmp/devimg-test-missing-key-alt.json",
            "--force",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&missing_key, 2);
    let stderr = String::from_utf8_lossy(&missing_key.stderr);
    assert!(stderr.contains("OPENAI_API_KEY"));
    assert!(!stderr.contains("ANTHROPIC_API_KEY"));

    let anthropic = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "alt",
            "--ai-provider",
            "anthropic",
            "--model",
            "anthropic-model",
            "--include-images",
            "--dry-run",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&anthropic, 2);
    assert!(String::from_utf8_lossy(&anthropic.stderr)
        .contains("OpenAI image-backed alt-text generation only"));

    let metadata_only_anthropic = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "alt",
            "--ai-provider",
            "anthropic",
            "--model",
            "anthropic-model",
            "--dry-run",
            "--output",
            "/tmp/devimg-test-anthropic-alt-placeholder.json",
            "--force",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&metadata_only_anthropic, 0);

    let invalid_mode = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "alt",
            "--ai-provider",
            "openai",
            "--model",
            "openai-model",
            "--metadata-only",
            "--include-images",
            "--dry-run",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&invalid_mode, 2);
    assert!(String::from_utf8_lossy(&invalid_mode.stderr)
        .contains("--metadata-only cannot be combined with --include-images"));

    let invalid_max = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "alt",
            "--ai-provider",
            "openai",
            "--model",
            "openai-model",
            "--max-images",
            "0",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&invalid_max, 2);
    assert!(String::from_utf8_lossy(&invalid_max.stderr)
        .contains("--max-images must be greater than 0"));
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
    let optimize_stdout = String::from_utf8_lossy(&optimize.stdout);
    assert!(optimize_stdout.contains("quality:"));
    assert!(optimize_stdout.contains("devimg suggest --metadata-only --config"));
    assert!(optimize_stdout.contains("--check --fail-on-severity warning"));
    assert!(fs::read_to_string(project.join("devimg-report.md"))
        .expect("report reads")
        .contains("quality:"));
    assert!(fs::read_to_string(project.join("devimg-report.md"))
        .expect("report reads")
        .contains("devimg suggest --metadata-only --config"));

    let default_check = run(["check", "--config", config.as_str()]);
    assert_status(&default_check, 0);
    let check_stdout = String::from_utf8_lossy(&default_check.stdout);
    assert!(check_stdout.contains("quality:"));
    assert!(check_stdout.contains("devimg suggest --metadata-only --config"));

    let strict_check = run(["check", "--config", config.as_str(), "--fail-on-warning"]);
    assert_status(&strict_check, 3);
    assert!(String::from_utf8_lossy(&strict_check.stderr).contains("quality:"));
    assert!(String::from_utf8_lossy(&strict_check.stderr)
        .contains("devimg suggest --metadata-only --config"));

    let doctor_human = run(["doctor", "--config", config.as_str()]);
    assert_status(&doctor_human, 0);
    let doctor_stdout = String::from_utf8_lossy(&doctor_human.stdout);
    assert!(doctor_stdout.contains("Suggestions"));
    assert!(doctor_stdout.contains("devimg suggest --metadata-only --config"));

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
    assert!(agents_text.contains("DevImg uses `devimg.toml` by default"));
    assert!(agents_text.contains("devimg doctor"));
    assert!(agents_text.contains("devimg optimize --allow-overwrite"));
    assert!(agents_text.contains("devimg manifest export"));
    assert!(agents_text.contains("devimg check"));
    assert!(!agents_text.contains("--config devimg.toml"));
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

#[test]
fn agent_task_stdout_default_uses_generic_and_keeps_secrets_out() {
    let project = fixture_project("agent_task_stdout", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let output = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .args(["agent", "task", "--config", config.as_str()])
        .env("OPENAI_API_KEY", "test-openai-secret")
        .env("ANTHROPIC_API_KEY", "test-anthropic-secret")
        .output()
        .expect("devimg runs");

    assert_status(&output, 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("# DevImg Agent Task"));
    assert!(stdout.contains("- Selected agent: `generic`"));
    assert!(stdout.contains("- Mode: `local-only`"));
    assert!(stdout.contains("Provider calls: none"));
    assert!(stdout.contains("## Checks"));
    assert!(stdout.contains("## Issues"));
    assert!(stdout.contains("missing_manifest"));
    assert!(stdout.contains("## File Ownership"));
    assert!(stdout.contains("Generic Markdown final response"));
    assert!(!stdout.contains("test-openai-secret"));
    assert!(!stdout.contains("test-anthropic-secret"));
    assert!(!project.join("devimg-agent-task.md").exists());
    cleanup(&project);
}

#[test]
fn agent_task_output_refuses_existing_unless_forced_and_protects_instruction_files() {
    let project = fixture_project("agent_task_output", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let task = project.join("ai_tasks/devimg-agent-task.md");
    let task_arg = path_arg(task.clone());

    assert_code(
        run([
            "agent",
            "task",
            "--config",
            config.as_str(),
            "--output",
            task_arg.as_str(),
        ]),
        0,
    );
    assert!(fs::read_to_string(&task)
        .expect("task output reads")
        .contains("# DevImg Agent Task"));

    fs::write(&task, "existing task\n").expect("write existing task");
    let refused = run([
        "agent",
        "task",
        "--config",
        config.as_str(),
        "--output",
        task_arg.as_str(),
    ]);
    assert_status(&refused, 4);
    assert_eq!(
        fs::read_to_string(&task).expect("task output reads"),
        "existing task\n"
    );

    assert_code(
        run([
            "agent",
            "task",
            "--config",
            config.as_str(),
            "--output",
            task_arg.as_str(),
            "--force",
        ]),
        0,
    );
    assert!(fs::read_to_string(&task)
        .expect("forced task output reads")
        .contains("Generic Markdown final response"));

    let agents = project.join("AGENTS.md");
    fs::write(&agents, "existing instructions\n").expect("write existing AGENTS");
    let agents_arg = path_arg(agents.clone());
    let protected = run([
        "agent",
        "task",
        "--config",
        config.as_str(),
        "--output",
        agents_arg.as_str(),
        "--force",
    ]);
    assert_status(&protected, 2);
    assert!(String::from_utf8_lossy(&protected.stderr)
        .contains("refuses to write task output to agent instruction paths"));
    assert_eq!(
        fs::read_to_string(&agents).expect("AGENTS reads"),
        "existing instructions\n"
    );
    cleanup(&project);
}

#[test]
fn agent_task_custom_config_and_agent_guidance_are_distinct() {
    let project = temp_project("agent_task_custom_config");
    let source = project.join("assets/images/sample.png");
    fs::create_dir_all(source.parent().expect("source parent")).expect("create source parent");
    fs::copy(fixture_image(), &source).expect("copy fixture image");
    fs::create_dir_all(project.join("config")).expect("create config dir");
    let nested_config = config_text_with_width(64, "", r#"max_total_bytes = "5mb""#).replacen(
        r#"root = ".""#,
        r#"root = "..""#,
        1,
    );
    fs::write(project.join("config/devimg.toml"), nested_config).expect("write nested config");

    let codex = run_in_dir(
        &project,
        [
            "agent",
            "task",
            "--config",
            "config/devimg.toml",
            "--agent",
            "codex",
        ],
    );
    assert_status(&codex, 0);
    let codex_stdout = String::from_utf8_lossy(&codex.stdout);
    assert!(codex_stdout.contains("- Selected agent: `codex`"));
    assert!(codex_stdout.contains("devimg doctor --config config/devimg.toml"));
    assert!(codex_stdout.contains("Codex final response"));

    let claude = run_in_dir(
        &project,
        [
            "agent",
            "task",
            "--config",
            "config/devimg.toml",
            "--agent",
            "claude-code",
        ],
    );
    assert_status(&claude, 0);
    let claude_stdout = String::from_utf8_lossy(&claude.stdout);
    assert!(claude_stdout.contains("- Selected agent: `claude-code`"));
    assert!(claude_stdout.contains("Claude Code final response"));

    let generic = run_in_dir(
        &project,
        [
            "agent",
            "task",
            "--config",
            "config/devimg.toml",
            "--agent",
            "generic",
        ],
    );
    assert_status(&generic, 0);
    let generic_stdout = String::from_utf8_lossy(&generic.stdout);
    assert!(generic_stdout.contains("- Selected agent: `generic`"));
    assert!(generic_stdout.contains("Generic Markdown final response"));
    assert_ne!(codex_stdout, claude_stdout);
    assert_ne!(claude_stdout, generic_stdout);
    cleanup(&project);
}

#[test]
fn ai_consent_dry_run_works_without_keys_for_both_providers() {
    let project = fixture_project("ai_consent_dry_run", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    for (provider, model) in [
        ("openai", "openai-dry-run-model"),
        ("anthropic", "anthropic-dry-run-model"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_devimg"))
            .args([
                "ai",
                "consent",
                "--config",
                config.as_str(),
                "--ai-provider",
                provider,
                "--model",
                model,
                "--dry-run",
            ])
            .env_remove("OPENAI_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .output()
            .expect("devimg runs");

        assert_status(&output, 0);
        let document: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("consent JSON parses");
        assert_eq!(document["provider"], provider);
        assert_eq!(document["model"], model);
        assert_eq!(document["command"], "devimg ai consent");
        assert_eq!(document["mode"], "metadata-only");
        assert_eq!(document["dry_run"], true);
        assert_eq!(document["paths_included"], true);
        assert_eq!(document["image_bytes_included"], false);
        assert_eq!(document["manifest_readable"], false);
        assert_eq!(
            document["source_files"][0]["path"],
            "assets/images/sample.png"
        );
        assert_eq!(document["source_files"][0]["image_bytes_included"], false);
        assert!(document["generated_outputs"]
            .as_array()
            .expect("generated outputs array")
            .is_empty());
    }

    cleanup(&project);
}

#[test]
fn ai_consent_requires_provider_keys_when_not_dry_run() {
    let project = fixture_project("ai_consent_missing_keys", "sample.png");

    let openai = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "ai",
            "consent",
            "--ai-provider",
            "openai",
            "--model",
            "openai-model",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&openai, 2);
    let openai_stderr = String::from_utf8_lossy(&openai.stderr);
    assert!(openai_stderr.contains("OPENAI_API_KEY"));
    assert!(!openai_stderr.contains("ANTHROPIC_API_KEY"));

    let anthropic = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "ai",
            "consent",
            "--ai-provider",
            "anthropic",
            "--model",
            "anthropic-model",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&anthropic, 2);
    let anthropic_stderr = String::from_utf8_lossy(&anthropic.stderr);
    assert!(anthropic_stderr.contains("ANTHROPIC_API_KEY"));
    assert!(!anthropic_stderr.contains("OPENAI_API_KEY"));

    cleanup(&project);
}

#[test]
fn ai_consent_never_prints_or_writes_fake_keys() {
    let project = fixture_project("ai_consent_fake_key", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    for (provider, env_var, other_env_var, fake_key) in [
        (
            "openai",
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "test-openai-secret-do-not-leak",
        ),
        (
            "anthropic",
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "test-anthropic-secret-do-not-leak",
        ),
    ] {
        let preview = project.join(format!("{provider}-consent.json"));
        let preview_arg = path_arg(preview.clone());
        let output = Command::new(env!("CARGO_BIN_EXE_devimg"))
            .args([
                "ai",
                "consent",
                "--config",
                config.as_str(),
                "--ai-provider",
                provider,
                "--model",
                "fake-model",
                "--output",
                preview_arg.as_str(),
            ])
            .env(env_var, fake_key)
            .env_remove(other_env_var)
            .output()
            .expect("devimg runs");

        assert_status(&output, 0);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stdout.contains(fake_key));
        assert!(!stderr.contains(fake_key));
        let json = fs::read_to_string(&preview).expect("consent preview reads");
        assert!(!json.contains(fake_key));
        assert!(!json.contains(env_var));
        let document: serde_json::Value = serde_json::from_str(&json).expect("consent JSON parses");
        assert_eq!(document["provider"], provider);
        assert_eq!(document["dry_run"], false);
    }

    cleanup(&project);
}

#[test]
fn ai_consent_loads_project_env_without_leaking_key_values() {
    let project = fixture_project("ai_consent_project_env", "sample.png");
    fs::write(
        project.join(".env"),
        "OPENAI_API_KEY=test-openai-env-secret-do-not-leak\n",
    )
    .expect("write project env");

    let output = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "ai",
            "consent",
            "--ai-provider",
            "openai",
            "--model",
            "env-model",
            "--output",
            "ai-consent.json",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");

    assert_status(&output, 0);
    let fake_key = "test-openai-env-secret-do-not-leak";
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains(fake_key));
    assert!(!stderr.contains(fake_key));
    let json = fs::read_to_string(project.join("ai-consent.json")).expect("consent JSON reads");
    assert!(!json.contains(fake_key));
    assert!(!json.contains("OPENAI_API_KEY"));
    let document: serde_json::Value = serde_json::from_str(&json).expect("consent JSON parses");
    assert_eq!(document["provider"], "openai");
    assert_eq!(document["dry_run"], false);
    cleanup(&project);
}

#[test]
fn ai_consent_include_images_and_metadata_only_validation_are_stable() {
    let project = fixture_project("ai_consent_include_images", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    let include_images = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .args([
            "ai",
            "consent",
            "--config",
            config.as_str(),
            "--ai-provider",
            "anthropic",
            "--model",
            "vision-preview",
            "--include-images",
            "--dry-run",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&include_images, 0);
    let document: serde_json::Value =
        serde_json::from_slice(&include_images.stdout).expect("consent JSON parses");
    assert_eq!(document["mode"], "include-images");
    assert_eq!(document["image_bytes_included"], true);
    assert_eq!(document["source_files"][0]["image_bytes_included"], true);

    let invalid = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .args([
            "ai",
            "consent",
            "--config",
            config.as_str(),
            "--ai-provider",
            "openai",
            "--model",
            "invalid-mode",
            "--metadata-only",
            "--include-images",
            "--dry-run",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&invalid, 2);
    assert!(String::from_utf8_lossy(&invalid.stderr)
        .contains("--metadata-only cannot be combined with --include-images"));

    cleanup(&project);
}

#[test]
fn ai_consent_output_is_deterministic_and_refuses_overwrite() {
    let project = fixture_project("ai_consent_output", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let preview = project.join("ai-consent.json");
    let preview_arg = path_arg(preview.clone());

    assert_code(
        Command::new(env!("CARGO_BIN_EXE_devimg"))
            .args([
                "ai",
                "consent",
                "--config",
                config.as_str(),
                "--ai-provider",
                "openai",
                "--model",
                "dry-run-model",
                "--dry-run",
                "--output",
                preview_arg.as_str(),
            ])
            .env_remove("OPENAI_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .output()
            .expect("devimg runs"),
        0,
    );
    let first = fs::read_to_string(&preview).expect("consent preview reads");

    let refused = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .args([
            "ai",
            "consent",
            "--config",
            config.as_str(),
            "--ai-provider",
            "openai",
            "--model",
            "dry-run-model",
            "--dry-run",
            "--output",
            preview_arg.as_str(),
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");
    assert_status(&refused, 4);
    assert_eq!(
        fs::read_to_string(&preview).expect("consent preview reads"),
        first
    );

    assert_code(
        Command::new(env!("CARGO_BIN_EXE_devimg"))
            .args([
                "ai",
                "consent",
                "--config",
                config.as_str(),
                "--ai-provider",
                "openai",
                "--model",
                "dry-run-model",
                "--dry-run",
                "--output",
                preview_arg.as_str(),
                "--force",
            ])
            .env_remove("OPENAI_API_KEY")
            .env_remove("ANTHROPIC_API_KEY")
            .output()
            .expect("devimg runs"),
        0,
    );
    assert_eq!(
        fs::read_to_string(&preview).expect("forced consent preview reads"),
        first
    );
    cleanup(&project);
}

#[test]
fn ai_consent_preview_includes_generated_outputs_when_manifest_is_readable() {
    let project = fixture_project("ai_consent_manifest", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    let output = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .args([
            "ai",
            "consent",
            "--config",
            config.as_str(),
            "--ai-provider",
            "anthropic",
            "--model",
            "dry-run-model",
            "--dry-run",
        ])
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("devimg runs");

    assert_status(&output, 0);
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("consent JSON parses");
    assert_eq!(document["manifest_readable"], true);
    assert_eq!(
        document["generated_outputs"][0]["output_path"],
        "public/images/generated/sample.project-card.64.webp"
    );
    assert_eq!(
        document["manifest_path"],
        "public/images/devimg-manifest.json"
    );
    assert_eq!(document["report_path"], "devimg-report.md");
    cleanup(&project);
}

#[test]
fn deterministic_commands_keep_provider_secrets_out_of_output() {
    let project = fixture_project("deterministic_secret_env", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let openai_key = "test-openai-secret";
    let anthropic_key = "test-anthropic-secret";
    fs::write(
        project.join(".env"),
        "OPENAI_API_KEY=test-openai-env-secret\nANTHROPIC_API_KEY=test-anthropic-env-secret\n",
    )
    .expect("write project env");

    let output = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .args(["optimize", "--config", config.as_str(), "--dry-run"])
        .env("OPENAI_API_KEY", openai_key)
        .env("ANTHROPIC_API_KEY", anthropic_key)
        .output()
        .expect("devimg runs");

    assert_status(&output, 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains(openai_key));
    assert!(!stdout.contains(anthropic_key));
    assert!(!stderr.contains(openai_key));
    assert!(!stderr.contains(anthropic_key));
    assert!(!stdout.contains("test-openai-env-secret"));
    assert!(!stderr.contains("test-openai-env-secret"));
    assert!(!project.join("devimg-report.md").exists());

    cleanup(&project);
}

#[test]
fn agent_task_includes_warning_context_and_generated_paths() {
    let project = temp_project("agent_task_warnings");
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
    .expect("write warning config");
    let config = path_arg(project.join("devimg.toml"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    let task = run(["agent", "task", "--config", config.as_str()]);

    assert_status(&task, 0);
    let stdout = String::from_utf8_lossy(&task.stdout);
    assert!(stdout.contains("quality:low-lossy-quality"));
    assert!(stdout.contains("Tune preset quality"));
    assert!(stdout.contains("public/images/generated/hero-screenshot.hero.64.webp"));
    assert!(stdout.contains("## Generated Artifacts"));
    cleanup(&project);
}

#[test]
fn suggest_requires_metadata_only_and_writes_nothing_without_it() {
    let project = fixture_project("suggest_requires_metadata_only", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    let output = run(["suggest", "--config", config.as_str()]);

    assert_status(&output, 2);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("requires --metadata-only"));
    assert!(stderr.contains("devimg suggest --metadata-only"));
    assert!(!project.join("devimg-suggestions.json").exists());
    cleanup(&project);
}

#[test]
fn suggest_default_output_is_valid_empty_json_and_deterministic() {
    let project = fixture_project("suggest_default_output", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    let first = run(["suggest", "--metadata-only", "--config", config.as_str()]);
    assert_status(&first, 0);
    let suggestions = project.join("devimg-suggestions.json");
    assert!(String::from_utf8_lossy(&first.stdout).contains("Created"));
    assert!(suggestions.exists());
    let first_json = fs::read_to_string(&suggestions).expect("suggestions read");
    let document: serde_json::Value =
        serde_json::from_str(&first_json).expect("suggestions JSON parses");
    assert_eq!(document["version"], 1);
    assert_eq!(document["mode"], "metadata-only");
    assert_eq!(document["summary"]["suggestion_count"], 0);
    assert_eq!(document["items"].as_array().expect("items array").len(), 0);

    let refused = run(["suggest", "--metadata-only", "--config", config.as_str()]);
    assert_status(&refused, 4);
    assert_eq!(
        fs::read_to_string(&suggestions).expect("suggestions read"),
        first_json
    );

    assert_code(
        run([
            "suggest",
            "--metadata-only",
            "--config",
            config.as_str(),
            "--force",
        ]),
        0,
    );
    assert_eq!(
        fs::read_to_string(&suggestions).expect("forced suggestions read"),
        first_json
    );
    cleanup(&project);
}

#[test]
fn suggest_check_is_read_only_when_no_suggestions_block() {
    let project = fixture_project("suggest_check_read_only", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    let output = run([
        "suggest",
        "--metadata-only",
        "--check",
        "--config",
        config.as_str(),
    ]);

    assert_status(&output, 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("DevImg Suggestions Summary"));
    assert!(stdout.contains("- Suggestions: `0`"));
    assert!(stdout.contains("- Check threshold: `warning`"));
    assert!(stdout.contains("- Blocking suggestions: `0`"));
    assert!(stdout.contains("no output written (read-only check)"));
    assert!(!project.join("devimg-suggestions.json").exists());
    cleanup(&project);
}

#[test]
fn suggest_check_thresholds_handle_warnings_errors_and_advisories() {
    let warning_project = fixture_project("suggest_check_warning", "sample.png");
    let warning_config = path_arg(warning_project.join("devimg.toml"));
    assert_code(run(["optimize", "--config", warning_config.as_str()]), 0);
    fs::remove_file(warning_project.join("devimg-report.md")).expect("remove report");

    let warning_gate = run([
        "suggest",
        "--metadata-only",
        "--check",
        "--config",
        warning_config.as_str(),
    ]);
    assert_status(&warning_gate, 3);
    let stderr = String::from_utf8_lossy(&warning_gate.stderr);
    assert!(stderr.contains("- Check threshold: `warning`"));
    assert!(stderr.contains("- Blocking suggestions: `1`"));
    assert!(stderr.contains("no output written (read-only check)"));
    assert!(!warning_project.join("devimg-suggestions.json").exists());

    let error_only_gate = run([
        "suggest",
        "--metadata-only",
        "--check",
        "--fail-on-severity",
        "error",
        "--config",
        warning_config.as_str(),
    ]);
    assert_status(&error_only_gate, 0);
    assert!(
        String::from_utf8_lossy(&error_only_gate.stdout).contains("- Blocking suggestions: `0`")
    );
    cleanup(&warning_project);

    let advisory_project = temp_project("suggest_check_advisory");
    let source = advisory_project.join("assets/images/hero-screenshot.png");
    fs::create_dir_all(source.parent().expect("source parent")).expect("create source parent");
    fs::copy(fixture_image(), &source).expect("copy fixture image");
    fs::write(
        advisory_project.join("devimg.toml"),
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
    .expect("write advisory config");
    let advisory_config = path_arg(advisory_project.join("devimg.toml"));
    assert_code(run(["optimize", "--config", advisory_config.as_str()]), 0);

    let default_gate = run([
        "suggest",
        "--metadata-only",
        "--check",
        "--config",
        advisory_config.as_str(),
    ]);
    assert_status(&default_gate, 0);
    assert!(String::from_utf8_lossy(&default_gate.stdout).contains("- Blocking suggestions: `0`"));

    let advisory_gate = run([
        "suggest",
        "--metadata-only",
        "--check",
        "--fail-on-severity",
        "advisory",
        "--config",
        advisory_config.as_str(),
    ]);
    assert_status(&advisory_gate, 3);
    assert!(
        String::from_utf8_lossy(&advisory_gate.stderr).contains("- Check threshold: `advisory`")
    );
    cleanup(&advisory_project);
}

#[test]
fn suggest_fail_on_severity_requires_check() {
    let project = fixture_project("suggest_fail_on_severity_requires_check", "sample.png");
    let config = path_arg(project.join("devimg.toml"));

    let output = run([
        "suggest",
        "--metadata-only",
        "--fail-on-severity",
        "warning",
        "--config",
        config.as_str(),
    ]);

    assert_status(&output, 2);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--fail-on-severity requires --check"));
    assert!(!project.join("devimg-suggestions.json").exists());
    cleanup(&project);
}

#[test]
fn suggest_check_explicit_outputs_write_and_preflight_are_safe() {
    let project = fixture_project("suggest_check_explicit_outputs", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let json = project.join("out/check-suggestions.json");
    let markdown = project.join("out/check-suggestions.md");
    let json_arg = path_arg(json.clone());
    let markdown_arg = path_arg(markdown.clone());

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    assert_code(
        run([
            "suggest",
            "--metadata-only",
            "--check",
            "--config",
            config.as_str(),
            "--output",
            json_arg.as_str(),
            "--markdown",
            markdown_arg.as_str(),
        ]),
        0,
    );
    assert!(fs::read_to_string(&json)
        .expect("check json reads")
        .contains("\"items\": []"));
    assert!(fs::read_to_string(&markdown)
        .expect("check markdown reads")
        .contains("No suggestions."));
    assert!(!project.join("devimg-suggestions.json").exists());

    fs::write(&json, "existing json\n").expect("write existing json");
    let refused = run([
        "suggest",
        "--metadata-only",
        "--check",
        "--config",
        config.as_str(),
        "--output",
        json_arg.as_str(),
        "--markdown",
        markdown_arg.as_str(),
    ]);
    assert_status(&refused, 4);
    assert_eq!(
        fs::read_to_string(&json).expect("json reads"),
        "existing json\n"
    );

    fs::remove_file(project.join("devimg-report.md")).expect("remove report");
    let failing_json = project.join("out/failing-check.json");
    let failing_json_arg = path_arg(failing_json.clone());
    let failed_gate = run([
        "suggest",
        "--metadata-only",
        "--check",
        "--config",
        config.as_str(),
        "--output",
        failing_json_arg.as_str(),
    ]);
    assert_status(&failed_gate, 3);
    assert!(failing_json.exists());
    cleanup(&project);
}

#[test]
fn suggest_custom_output_markdown_force_and_preflight_are_safe() {
    let project = fixture_project("suggest_custom_output", "sample.png");
    let config = path_arg(project.join("devimg.toml"));
    let json = project.join("out/devimg-suggestions.json");
    let markdown = project.join("out/devimg-suggestions.md");
    let json_arg = path_arg(json.clone());
    let markdown_arg = path_arg(markdown.clone());

    assert_code(run(["optimize", "--config", config.as_str()]), 0);
    assert_code(
        run([
            "suggest",
            "--metadata-only",
            "--config",
            config.as_str(),
            "--output",
            json_arg.as_str(),
            "--markdown",
            markdown_arg.as_str(),
        ]),
        0,
    );
    assert!(fs::read_to_string(&json)
        .expect("suggestions read")
        .contains("\"mode\": \"metadata-only\""));
    assert!(fs::read_to_string(&markdown)
        .expect("markdown reads")
        .contains("# DevImg Suggestions"));

    let same_path = project.join("out/same-path");
    let same_path_arg = path_arg(same_path);
    let same_path_output = run([
        "suggest",
        "--metadata-only",
        "--config",
        config.as_str(),
        "--output",
        same_path_arg.as_str(),
        "--markdown",
        same_path_arg.as_str(),
    ]);
    assert_status(&same_path_output, 2);
    assert!(String::from_utf8_lossy(&same_path_output.stderr)
        .contains("--output and --markdown must use different paths"));

    fs::write(&json, "existing json\n").expect("write existing json");
    fs::write(&markdown, "existing markdown\n").expect("write existing markdown");
    let refused_json = run([
        "suggest",
        "--metadata-only",
        "--config",
        config.as_str(),
        "--output",
        json_arg.as_str(),
        "--markdown",
        markdown_arg.as_str(),
    ]);
    assert_status(&refused_json, 4);
    assert_eq!(
        fs::read_to_string(&json).expect("json reads"),
        "existing json\n"
    );
    assert_eq!(
        fs::read_to_string(&markdown).expect("markdown reads"),
        "existing markdown\n"
    );

    let json_preflight = project.join("out/preflight.json");
    let json_preflight_arg = path_arg(json_preflight.clone());
    let refused_markdown = run([
        "suggest",
        "--metadata-only",
        "--config",
        config.as_str(),
        "--output",
        json_preflight_arg.as_str(),
        "--markdown",
        markdown_arg.as_str(),
    ]);
    assert_status(&refused_markdown, 4);
    assert!(!json_preflight.exists());

    assert_code(
        run([
            "suggest",
            "--metadata-only",
            "--config",
            config.as_str(),
            "--output",
            json_arg.as_str(),
            "--markdown",
            markdown_arg.as_str(),
            "--force",
        ]),
        0,
    );
    assert!(fs::read_to_string(&json)
        .expect("forced json reads")
        .contains("\"items\": []"));
    assert!(fs::read_to_string(&markdown)
        .expect("forced markdown reads")
        .contains("No suggestions."));
    cleanup(&project);
}

#[test]
fn suggest_custom_config_warning_context_and_secrets_are_stable() {
    let project = temp_project("suggest_warning_context");
    let source = project.join("assets/images/hero-screenshot.png");
    fs::create_dir_all(source.parent().expect("source parent")).expect("create source parent");
    fs::copy(fixture_image(), &source).expect("copy fixture image");
    fs::create_dir_all(project.join("config")).expect("create config dir");
    let nested_config = r#"[project]
root = ".."
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
"#;
    fs::write(project.join("config/devimg.toml"), nested_config).expect("write nested config");
    let output_path = project.join("suggestions.json");
    let output_arg = path_arg(output_path.clone());

    assert_code(
        run_in_dir(&project, ["optimize", "--config", "config/devimg.toml"]),
        0,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(&project)
        .args([
            "suggest",
            "--metadata-only",
            "--config",
            "config/devimg.toml",
            "--output",
            output_arg.as_str(),
        ])
        .env("OPENAI_API_KEY", "test-openai-secret")
        .env("ANTHROPIC_API_KEY", "test-anthropic-secret")
        .output()
        .expect("devimg runs");

    assert_status(&output, 0);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stdout.contains("test-openai-secret"));
    assert!(!stderr.contains("test-anthropic-secret"));
    let json = fs::read_to_string(&output_path).expect("suggestions read");
    assert!(!json.contains("test-openai-secret"));
    assert!(!json.contains("test-anthropic-secret"));
    let document: serde_json::Value = serde_json::from_str(&json).expect("suggestions JSON parses");
    assert_eq!(document["config_path"], "config/devimg.toml");
    let items = document["items"].as_array().expect("items array");
    let quality = items
        .iter()
        .find(|item| item["warning_code"] == "quality:low-lossy-quality")
        .expect("quality suggestion exists");
    assert_eq!(quality["severity"], "advisory");
    assert_eq!(quality["source_path"], "assets/images/hero-screenshot.png");
    assert_eq!(quality["preset"], "hero");
    assert_eq!(quality["format"], "webp");
    assert_eq!(
        quality["affected_path"],
        "assets/images/hero-screenshot.png"
    );
    assert_eq!(
        quality["next_command"],
        "devimg optimize --config config/devimg.toml --allow-overwrite"
    );
    assert_eq!(quality["suggested_config"]["target"], "preset");
    assert_eq!(quality["suggested_config"]["changes"]["quality"], 82);
    cleanup(&project);
}

#[test]
fn suggest_missing_config_keeps_existing_hint() {
    let project = temp_project("suggest_missing_config");
    let missing_config = path_arg(project.join("missing.toml"));

    let output = run([
        "suggest",
        "--metadata-only",
        "--config",
        missing_config.as_str(),
    ]);

    assert_status(&output, 2);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing.toml"));
    assert!(stderr.contains("devimg init"));
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

fn run_in_dir<I, S>(dir: &Path, args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_devimg"))
        .current_dir(dir)
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
