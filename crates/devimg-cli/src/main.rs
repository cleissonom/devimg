use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand, ValueEnum};
use devimg_core::{
    check, doctor, doctor_report_to_json, inspect_image, load_config, manifest_export_to_json,
    manifest_export_to_typescript, optimize, read_manifest, render_doctor_report,
    render_manifest_report, render_run_report, DevimgError, DoctorManifestExportFormat,
    DoctorManifestExportOptions, DoctorOptions, ManifestExportOptions, OptimizeOptions,
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
        Command::Inspect(args) => command_inspect(args),
        Command::Manifest(args) => command_manifest(args),
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
    Inspect(InspectArgs),
    Manifest(ManifestArgs),
}

#[derive(Debug, Args)]
struct InitArgs {
    #[arg(long, default_value = "devimg.toml")]
    config: PathBuf,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    force: bool,
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
struct InspectArgs {
    #[arg(required = true, allow_hyphen_values = true)]
    files: Vec<PathBuf>,
}

#[derive(Debug, Args)]
struct ManifestArgs {
    #[command(subcommand)]
    command: ManifestCommand,
}

#[derive(Debug, Subcommand)]
enum ManifestCommand {
    Export(ManifestExportArgs),
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

fn command_init(args: InitArgs) -> Result<(), CliError> {
    let sample = starter_config();
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

fn starter_config() -> &'static str {
    r#"[project]
root = "."
manifest = "public/images/devimg-manifest.json"
report = "devimg-report.md"
overwrite = false
strip_metadata = true
content_hash_filenames = false

[[sources]]
name = "portfolio"
input = "assets/images"
output = "public/images/generated"
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
