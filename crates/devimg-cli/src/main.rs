use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use devimg_core::{
    check, compare_manifests, doctor, doctor_report_to_json, inspect_image, load_config,
    manifest_compare_to_json, manifest_export_to_json, manifest_export_to_typescript, optimize,
    read_manifest, render_doctor_report, render_manifest_compare_report, render_manifest_report,
    render_run_report, DevimgError, DoctorManifestExportFormat, DoctorManifestExportOptions,
    DoctorOptions, ManifestCompareOptions, ManifestExportOptions, OptimizeOptions,
};

fn main() {
    let code = match run(std::env::args_os()) {
        Ok(()) => 0,
        Err(CliError::Parse(error)) => {
            let is_help = matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            );
            if is_help {
                print!("{error}");
                0
            } else {
                eprint!("{error}");
                2
            }
        }
        Err(CliError::CheckFailed(report)) => {
            eprintln!("{report}");
            3
        }
        Err(CliError::DoctorFailed { report, json }) => {
            if json {
                print!("{report}");
            } else {
                eprintln!("{report}");
            }
            3
        }
        Err(CliError::Core(error)) => {
            eprintln!("{}", render_core_error(&error));
            match &error {
                DevimgError::Config { .. } => 2,
                DevimgError::UnsafeOverwrite { .. } => 4,
                _ => 1,
            }
        }
    };
    std::process::exit(code);
}

fn run<I, T>(args: I) -> Result<(), CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::try_parse_from(args).map_err(CliError::Parse)?;
    match cli.command {
        Command::Init(args) => command_init(args),
        Command::Optimize(args) => command_optimize(args),
        Command::Check(args) => command_check(args),
        Command::Doctor(args) => command_doctor(args),
        Command::Report(args) => command_report(args),
        Command::Compare(args) => command_compare(args),
        Command::Inspect(args) => command_inspect(args),
        Command::Manifest(args) => command_manifest(args),
        Command::Agent(args) => command_agent(args),
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "devimg",
    about = "Developer image pipeline",
    disable_version_flag = true,
    subcommand_required = true,
    arg_required_else_help = false,
    color = clap::ColorChoice::Never
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init(InitArgs),
    Optimize(OptimizeArgs),
    Check(CheckArgs),
    Doctor(DoctorArgs),
    Report(ReportArgs),
    Compare(CompareArgs),
    Inspect(InspectArgs),
    Manifest(ManifestArgs),
    Agent(AgentArgs),
}

#[derive(Debug, Args)]
struct InitArgs {
    #[arg(long, default_value = "devimg.toml")]
    config: PathBuf,
    #[arg(long, value_enum)]
    profile: Option<InitProfile>,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InitProfile {
    Next,
    Astro,
    Vite,
}

#[derive(Debug, Args)]
struct OptimizeArgs {
    #[arg(long, default_value = "devimg.toml")]
    config: PathBuf,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    allow_overwrite: bool,
}

#[derive(Debug, Args)]
struct CheckArgs {
    #[arg(long, default_value = "devimg.toml")]
    config: PathBuf,
    #[arg(long)]
    fail_on_warning: bool,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    #[arg(long, default_value = "devimg.toml")]
    config: PathBuf,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    export_output: Option<PathBuf>,
    #[arg(long, value_enum, default_value = "json")]
    export_format: ManifestExportFormat,
    #[arg(long)]
    strip_prefix: Option<String>,
    #[arg(long, default_value = "")]
    url_prefix: String,
}

#[derive(Debug, Args)]
struct ReportArgs {
    #[arg(long)]
    manifest: PathBuf,
}

#[derive(Debug, Args)]
struct CompareArgs {
    #[arg(long)]
    base: PathBuf,
    #[arg(long)]
    head: PathBuf,
    #[arg(long)]
    json: bool,
    #[arg(long, default_value_t = 5)]
    top: usize,
}

#[derive(Debug, Args)]
struct InspectArgs {
    #[arg(required = true, allow_hyphen_values = true)]
    files: Vec<PathBuf>,
}

#[derive(Debug, Args)]
struct ManifestArgs {
    #[command(subcommand)]
    command: ManifestCommand,
}

#[derive(Debug, Args)]
struct AgentArgs {
    #[command(subcommand)]
    command: AgentCommand,
}

#[derive(Debug, Subcommand)]
enum ManifestCommand {
    Export(ManifestExportArgs),
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    Init(AgentInitArgs),
}

#[derive(Debug, Args)]
struct ManifestExportArgs {
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long, value_enum, default_value = "json")]
    format: ManifestExportFormat,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    strip_prefix: Option<String>,
    #[arg(long, default_value = "")]
    url_prefix: String,
    #[arg(long)]
    check: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ManifestExportFormat {
    Json,
    Typescript,
}

#[derive(Debug, Args)]
struct AgentInitArgs {
    #[arg(long, value_enum)]
    target: AgentTarget,
    #[arg(long, default_value = ".")]
    output_dir: PathBuf,
    #[arg(long, default_value = "devimg.toml")]
    config: PathBuf,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AgentTarget {
    Codex,
    Claude,
    Both,
}

fn command_init(args: InitArgs) -> Result<(), CliError> {
    let sample = starter_config(args.profile);
    if args.stdout {
        print!("{sample}");
        return Ok(());
    }
    if args.config.exists() && !args.force {
        return Err(CliError::Core(DevimgError::UnsafeOverwrite {
            path: args.config,
        }));
    }
    if let Some(parent) = args.config.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| DevimgError::io(parent, source))?;
        }
    }
    fs::write(&args.config, sample).map_err(|source| DevimgError::io(&args.config, source))?;
    println!("Created {}", args.config.display());
    Ok(())
}

fn command_optimize(args: OptimizeArgs) -> Result<(), CliError> {
    let config = load_config(args.config)?;
    let result = optimize(
        &config,
        OptimizeOptions {
            dry_run: args.dry_run,
            allow_overwrite: args.allow_overwrite,
        },
    )?;
    println!("{}", render_run_report(&result));
    Ok(())
}

fn command_check(args: CheckArgs) -> Result<(), CliError> {
    let config = load_config(&args.config)?;
    let mut result = check(&config)?;
    if args.fail_on_warning && !result.result.warnings.is_empty() {
        result.passed = false;
    }
    let report = render_run_report(&result.result);
    if result.passed {
        println!("{report}");
        Ok(())
    } else {
        Err(CliError::CheckFailed(with_check_hint(report, &args.config)))
    }
}

fn command_doctor(args: DoctorArgs) -> Result<(), CliError> {
    let config = load_config(&args.config)?;
    let manifest_export = args
        .export_output
        .map(|output| DoctorManifestExportOptions {
            output,
            format: match args.export_format {
                ManifestExportFormat::Json => DoctorManifestExportFormat::Json,
                ManifestExportFormat::Typescript => DoctorManifestExportFormat::Typescript,
            },
            strip_prefix: args.strip_prefix,
            url_prefix: args.url_prefix,
        });
    let report = doctor(&config, DoctorOptions { manifest_export })?;
    let rendered = if args.json {
        doctor_report_to_json(&report)
    } else {
        render_doctor_report(&report)
    };
    if report.passed() {
        print!("{rendered}");
        Ok(())
    } else {
        Err(CliError::DoctorFailed {
            report: rendered,
            json: args.json,
        })
    }
}

fn command_report(args: ReportArgs) -> Result<(), CliError> {
    let manifest = read_manifest(&args.manifest)?;
    println!("{}", render_manifest_report(&manifest));
    Ok(())
}

fn command_compare(args: CompareArgs) -> Result<(), CliError> {
    let base = read_manifest(&args.base)?;
    let head = read_manifest(&args.head)?;
    let compare = compare_manifests(
        &base,
        &head,
        ManifestCompareOptions {
            top_limit: args.top,
        },
    );
    if args.json {
        print!("{}", manifest_compare_to_json(&compare));
    } else {
        println!("{}", render_manifest_compare_report(&compare));
    }
    Ok(())
}

fn command_inspect(args: InspectArgs) -> Result<(), CliError> {
    for file in args.files {
        let info = inspect_image(&file)?;
        println!("{}:", info.path);
        println!("  format: {}", info.format);
        println!("  dimensions: {}x{}", info.width, info.height);
        println!("  bytes: {}", info.bytes);
        println!("  hash: {}", info.hash);
    }
    Ok(())
}

fn command_manifest(args: ManifestArgs) -> Result<(), CliError> {
    match args.command {
        ManifestCommand::Export(args) => command_manifest_export(args),
    }
}

fn command_agent(args: AgentArgs) -> Result<(), CliError> {
    match args.command {
        AgentCommand::Init(args) => command_agent_init(args),
    }
}

fn command_agent_init(args: AgentInitArgs) -> Result<(), CliError> {
    let files = agent_instruction_files(&args);
    if args.stdout {
        for (index, file) in files.iter().enumerate() {
            if index > 0 {
                println!();
            }
            println!("# {}", file.relative_path.display());
            print!("{}", file.contents);
        }
        return Ok(());
    }

    if !args.force {
        for file in &files {
            let path = args.output_dir.join(&file.relative_path);
            if path.exists() {
                return Err(CliError::Core(DevimgError::UnsafeOverwrite { path }));
            }
        }
    }

    for file in files {
        let path = args.output_dir.join(&file.relative_path);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|source| DevimgError::io(parent, source))?;
            }
        }
        fs::write(&path, file.contents).map_err(|source| DevimgError::io(&path, source))?;
        println!("Created {}", path.display());
    }

    Ok(())
}

fn command_manifest_export(args: ManifestExportArgs) -> Result<(), CliError> {
    let manifest = read_manifest(&args.manifest)?;
    let options = ManifestExportOptions {
        strip_prefix: args.strip_prefix.clone(),
        url_prefix: args.url_prefix.clone(),
    };
    let rendered = match args.format {
        ManifestExportFormat::Json => manifest_export_to_json(&manifest, &options),
        ManifestExportFormat::Typescript => manifest_export_to_typescript(&manifest, &options),
    };

    if args.check {
        let output = args.output.clone().ok_or_else(|| {
            CliError::Core(DevimgError::config(
                &args.manifest,
                "--check requires --output",
            ))
        })?;
        let current = match fs::read(&output) {
            Ok(current) => current,
            Err(source) if source.kind() == ErrorKind::NotFound => {
                return Err(CliError::CheckFailed(format!(
                    "Manifest export is missing: {}\nHint: update it with `{}`.",
                    output.display(),
                    manifest_export_write_command(&args, &output)
                )));
            }
            Err(source) => return Err(CliError::Core(DevimgError::io(&output, source))),
        };
        if current == rendered.as_bytes() {
            println!("Manifest export is up to date: {}", output.display());
            return Ok(());
        }
        return Err(CliError::CheckFailed(format!(
            "Manifest export is stale: {}\nHint: update it with `{}`.",
            output.display(),
            manifest_export_write_command(&args, &output)
        )));
    }

    if let Some(output) = args.output {
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|source| DevimgError::io(parent, source))?;
            }
        }
        fs::write(&output, rendered).map_err(|source| DevimgError::io(&output, source))?;
    } else {
        print!("{rendered}");
    }

    Ok(())
}

fn with_check_hint(report: String, config_path: &Path) -> String {
    format!(
        "{report}\nHint: If outputs are missing or stale, regenerate them with `{}`. For budget failures, reduce image bytes or adjust budgets.\nNext: {}\n",
        optimize_command(config_path),
        doctor_command(config_path)
    )
}

fn render_core_error(error: &DevimgError) -> String {
    let mut out = format!("Error: {error}");
    match error {
        DevimgError::Config { path, message } if message == "config file not found" => {
            out.push_str(&format!(
                "\nHint: create a starter config with `{}` or pass the right `--config` path.",
                init_command(path)
            ));
        }
        DevimgError::Config { path, .. } => {
            out.push_str(&format!(
                "\nHint: fix the config, then inspect it with `{}`.",
                doctor_command(path)
            ));
        }
        DevimgError::UnsafeOverwrite { .. } => {
            out.push_str(
                "\nHint: devimg will not replace unmanaged outputs unless you rerun optimize with `--allow-overwrite` or set `[project].overwrite = true`.",
            );
        }
        DevimgError::Image { .. } => {
            out.push_str("\nHint: inspect the file with `devimg inspect <file>` or replace corrupt/mislabelled source images.");
        }
        DevimgError::Io { .. } | DevimgError::CheckFailed { .. } => {}
    }
    out
}

fn manifest_export_write_command(args: &ManifestExportArgs, output: &Path) -> String {
    let mut command = format!(
        "devimg manifest export --manifest {} --format {}",
        shell_arg_path(&args.manifest),
        args.format.label()
    );
    if let Some(strip_prefix) = &args.strip_prefix {
        command.push_str(&format!(" --strip-prefix {}", shell_arg(strip_prefix)));
    }
    if !args.url_prefix.is_empty() {
        command.push_str(&format!(" --url-prefix {}", shell_arg(&args.url_prefix)));
    }
    command.push_str(&format!(" --output {}", shell_arg_path(output)));
    command
}

fn init_command(config_path: &Path) -> String {
    format!("devimg init --config {}", shell_arg_path(config_path))
}

fn optimize_command(config_path: &Path) -> String {
    format!(
        "devimg optimize --config {} --allow-overwrite",
        shell_arg_path(config_path)
    )
}

fn doctor_command(config_path: &Path) -> String {
    format!("devimg doctor --config {}", shell_arg_path(config_path))
}

fn shell_arg_path(path: &Path) -> String {
    shell_arg(&path.to_string_lossy())
}

fn shell_arg(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '@'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

impl ManifestExportFormat {
    fn label(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Typescript => "typescript",
        }
    }
}

struct AgentInstructionFile {
    relative_path: PathBuf,
    contents: String,
}

fn agent_instruction_files(args: &AgentInitArgs) -> Vec<AgentInstructionFile> {
    let config = shell_arg_path(&args.config);
    let codex = AgentInstructionFile {
        relative_path: PathBuf::from("AGENTS.md"),
        contents: codex_agent_instructions(&config),
    };
    let claude_memory = AgentInstructionFile {
        relative_path: PathBuf::from("CLAUDE.md"),
        contents: claude_agent_instructions(&config),
    };
    let claude_command = AgentInstructionFile {
        relative_path: PathBuf::from(".claude/commands/devimg-doctor.md"),
        contents: claude_devimg_command(&config),
    };

    match args.target {
        AgentTarget::Codex => vec![codex],
        AgentTarget::Claude => vec![claude_memory, claude_command],
        AgentTarget::Both => vec![codex, claude_memory, claude_command],
    }
}

fn codex_agent_instructions(config: &str) -> String {
    format!(
        r#"# DevImg Agent Instructions

## Image Pipeline Workflow

- Run `devimg doctor --config {config}` before editing source images, `devimg.toml`, generated variants, manifests, reports, or app image helper files.
- After image source or config changes, run `devimg optimize --config {config} --allow-overwrite`.
- If the project checks in a manifest helper, regenerate it with `devimg manifest export`.
- Run `devimg check --config {config}` before finishing.
- Run `devimg doctor --config {config}` again to confirm the project is healthy.
- Commit generated image variants, `devimg-manifest.json`, `devimg-report.md`, and checked-in manifest helper files together.
- Do not edit generated image variants, manifests, reports, or generated helper modules by hand.
- Do not overwrite existing agent instruction files or skills without explicit user approval.

Recommended loop:

```bash
devimg doctor --config {config}
devimg optimize --config {config} --allow-overwrite
devimg manifest export --manifest public/images/devimg-manifest.json --strip-prefix public --url-prefix / --format typescript --output lib/devimg.generated.ts
devimg check --config {config}
devimg doctor --config {config} --export-output lib/devimg.generated.ts --export-format typescript --strip-prefix public --url-prefix /
```

If the project uses different manifest or helper paths, inspect `{config}` and adjust the manifest export command before running it.
"#
    )
}

fn claude_agent_instructions(config: &str) -> String {
    format!(
        r#"# DevImg Agent Instructions

Use these instructions when working with generated web image assets.

- Start with `devimg doctor --config {config}` before changing source images, `devimg.toml`, generated variants, manifests, reports, or app image helper files.
- Regenerate outputs with `devimg optimize --config {config} --allow-overwrite` after image source or config changes.
- Regenerate checked-in manifest helpers with `devimg manifest export` when the project uses them.
- Validate with `devimg check --config {config}` and then run `devimg doctor --config {config}` again.
- Commit generated image variants, `devimg-manifest.json`, `devimg-report.md`, and checked-in manifest helper files together.
- Never hand-edit generated image variants, manifests, reports, or generated helper modules.
- Never overwrite existing agent instruction files, Claude commands, or Codex skills without explicit user approval.

Recommended loop:

```bash
devimg doctor --config {config}
devimg optimize --config {config} --allow-overwrite
devimg manifest export --manifest public/images/devimg-manifest.json --strip-prefix public --url-prefix / --format typescript --output lib/devimg.generated.ts
devimg check --config {config}
devimg doctor --config {config} --export-output lib/devimg.generated.ts --export-format typescript --strip-prefix public --url-prefix /
```

If this project uses a different manifest path or does not check in a generated helper, inspect `{config}` and adjust or skip the manifest export step.
"#
    )
}

fn claude_devimg_command(config: &str) -> String {
    format!(
        r#"---
description: Diagnose and update DevImg generated image assets
argument-hint: [config path]
---

Run the DevImg image pipeline workflow. Use `$ARGUMENTS` as the config path when provided; otherwise use `{config}`.

Steps:

1. Run `devimg doctor --config <config>`.
2. If source images or config changed, run `devimg optimize --config <config> --allow-overwrite`.
3. If the project checks in a manifest helper, run `devimg manifest export` with the project manifest/helper paths.
4. Run `devimg check --config <config>`.
5. Run `devimg doctor --config <config>` again.

Rules:

- Do not hand-edit generated variants, manifests, reports, or helper modules.
- Do not overwrite existing agent instruction files, Claude commands, or Codex skills without explicit user approval.
- Report changed generated files and verification results.
"#
    )
}

fn starter_config(profile: Option<InitProfile>) -> String {
    let (source_name, input, output) = match profile {
        None => ("portfolio", "assets/images", "public/images/generated"),
        Some(InitProfile::Next) => ("next", "public/images/source", "public/images/generated"),
        Some(InitProfile::Astro) => ("astro", "src/assets/images", "public/images/generated"),
        Some(InitProfile::Vite) => ("vite", "src/assets/images", "public/images/generated"),
    };

    format!(
        r#"[project]
root = "."
manifest = "public/images/devimg-manifest.json"
report = "devimg-report.md"
overwrite = false
strip_metadata = true
content_hash_filenames = false

[[sources]]
name = "{source_name}"
input = "{input}"
output = "{output}"
include = ["**/*.png", "**/*.jpg", "**/*.jpeg", "**/*.webp"]
exclude = ["**/generated/**"]

[[preset]]
name = "project-card"
widths = [640, 960, 1280]
formats = ["webp", "jpeg"]
quality = 82
fit = "cover"
aspect_ratio = "16:9"
crop = "center"

[[preset]]
name = "open-graph"
widths = [1200]
formats = ["png", "webp"]
quality = 90
fit = "cover"
aspect_ratio = "1200:630"
crop = "center"

[[preset]]
name = "avatar"
widths = [256, 512]
formats = ["webp", "jpeg"]
quality = 86
fit = "cover"
aspect_ratio = "1:1"
crop = "center"

[[preset]]
name = "hero"
widths = [1280, 1920]
formats = ["webp", "jpeg"]
quality = 84
fit = "cover"
aspect_ratio = "21:9"
crop = "center"

# Optional source-specific transform override. Paths are relative to the source input.
# [[overrides]]
# include = ["diagrams/**"]
# fit = "contain"

[budgets]
max_total_bytes = "3mb"
max_file_bytes = "350kb"
fail_on_regression = true
"#
    )
}

#[derive(Debug)]
enum CliError {
    Parse(clap::Error),
    Core(DevimgError),
    CheckFailed(String),
    DoctorFailed { report: String, json: bool },
}

impl From<DevimgError> for CliError {
    fn from(value: DevimgError) -> Self {
        Self::Core(value)
    }
}
