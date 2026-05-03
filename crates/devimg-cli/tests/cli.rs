use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn help_and_usage_exit_codes_are_stable() {
    assert_code(run(["--help"]), 0);
    assert_code(run(["doctor", "--help"]), 0);
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

    let malformed = project.join("bad-manifest.json");
    fs::write(&malformed, "{").expect("write malformed manifest");
    let malformed = path_arg(malformed);
    assert_code(run(["report", "--manifest", malformed.as_str()]), 2);
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
fn init_stdout_refusal_and_force_are_stable() {
    let project = temp_project("init");
    let config_path = project.join("nested/devimg.toml");
    let config = path_arg(config_path.clone());

    let stdout = run(["init", "--config", config.as_str(), "--stdout"]);
    assert_status(&stdout, 0);
    assert!(String::from_utf8_lossy(&stdout.stdout).contains("[project]"));
    assert!(!config_path.exists());

    assert_code(run(["init", "--config", config.as_str()]), 0);
    assert!(config_path.exists());
    assert_code(run(["init", "--config", config.as_str()]), 4);
    assert_code(run(["init", "--config", config.as_str(), "--force"]), 0);
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

fn fixture_image() -> PathBuf {
    repo_root().join("fixtures/images/sample.png")
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

fn path_arg(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

fn cleanup(path: &Path) {
    if path.exists() {
        fs::remove_dir_all(path).expect("cleanup temp project");
    }
}
