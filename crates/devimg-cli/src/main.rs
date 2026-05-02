use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use devimg_core::{
    check, inspect_image, load_config, manifest_export_to_json, manifest_export_to_typescript,
    optimize, read_manifest, render_manifest_report, render_run_report, DevimgError,
    ManifestExportOptions, OptimizeOptions,
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
        Err(CliError::Core(error)) => {
            eprintln!("Error: {error}");
            match error {
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
    let config = load_config(args.config)?;
    let mut result = check(&config)?;
    if args.fail_on_warning && !result.result.warnings.is_empty() {
        result.passed = false;
    }
    let report = render_run_report(&result.result);
    if result.passed {
        println!("{report}");
        Ok(())
    } else {
        Err(CliError::CheckFailed(report))
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
        strip_prefix: args.strip_prefix,
        url_prefix: args.url_prefix,
    };
    let rendered = match args.format {
        ManifestExportFormat::Json => manifest_export_to_json(&manifest, &options),
        ManifestExportFormat::Typescript => manifest_export_to_typescript(&manifest, &options),
    };

    if args.check {
        let output = args.output.ok_or_else(|| {
            CliError::Core(DevimgError::config(
                &args.manifest,
                "--check requires --output",
            ))
        })?;
        let current = match fs::read(&output) {
            Ok(current) => current,
            Err(source) if source.kind() == ErrorKind::NotFound => {
                return Err(CliError::CheckFailed(format!(
                    "Manifest export is missing: {}",
                    output.display()
                )));
            }
            Err(source) => return Err(CliError::Core(DevimgError::io(&output, source))),
        };
        if current == rendered.as_bytes() {
            println!("Manifest export is up to date: {}", output.display());
            return Ok(());
        }
        return Err(CliError::CheckFailed(format!(
            "Manifest export is stale: {}",
            output.display()
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
}

impl From<DevimgError> for CliError {
    fn from(value: DevimgError) -> Self {
        Self::Core(value)
    }
}
