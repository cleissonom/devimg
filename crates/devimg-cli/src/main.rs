use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use clap::{Args, Parser, Subcommand, ValueEnum};
use devimg_core::{
    ai_alt_report_to_json, ai_consent_preview_to_json, ai_review_report_to_json,
    build_ai_alt_placeholder_report, build_ai_alt_report, build_ai_alt_request,
    build_ai_consent_preview, build_ai_review_dry_run_report, build_ai_review_report,
    build_ai_review_request, check_with_options, compare_manifests, doctor, doctor_report_to_json,
    inspect_image, load_config, manifest_compare_to_json, manifest_export_to_json,
    manifest_export_to_typescript_with_options, optimize, read_manifest, render_ai_alt_markdown,
    render_ai_review_markdown, render_doctor_report, render_manifest_compare_report,
    render_manifest_report, render_manifest_review, render_run_report, render_suggestion_markdown,
    suggest, suggestion_report_to_json, AiAltOptions, AiAltProviderPayload, AiAltRequest,
    AiConsentOptions, AiProvider, AiReviewOptions, AiReviewProviderPayload, AiReviewRequest,
    CheckOptions, DevimgError, DoctorManifestExportFormat, DoctorManifestExportOptions,
    DoctorOptions, ManifestCompareOptions, ManifestExportOptions, ManifestReviewOptions,
    ManifestTypescriptOptions, OptimizeOptions, SuggestOptions, SuggestionItem, SuggestionReport,
};
use serde_json::{json, Value};

const DEFAULT_CONFIG_PATH: &str = "devimg.toml";
const DEFAULT_AI_REVIEW_OUTPUT: &str = "devimg-ai-review.json";
const DEFAULT_AI_ALT_OUTPUT: &str = "devimg-alt.json";
const DEFAULT_AI_REVIEW_MAX_IMAGES: usize = 8;

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
        Err(CliError::SuggestCheckFailed(report)) => {
            eprintln!("{report}");
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
        Command::Review(args) => command_review(args),
        Command::Compare(args) => command_compare(args),
        Command::Inspect(args) => command_inspect(args),
        Command::Suggest(args) => command_suggest(args),
        Command::Alt(args) => command_alt(args),
        Command::Manifest(args) => command_manifest(args),
        Command::Agent(args) => command_agent(args),
        Command::Ai(args) => command_ai(args),
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "devimg",
    version,
    about = "Developer image pipeline",
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
    Review(ReviewArgs),
    Compare(CompareArgs),
    Inspect(InspectArgs),
    Suggest(SuggestArgs),
    Alt(AltArgs),
    Manifest(ManifestArgs),
    Agent(AgentArgs),
    Ai(AiArgs),
}

#[derive(Debug, Args)]
struct InitArgs {
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
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
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
    config: PathBuf,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    allow_overwrite: bool,
}

#[derive(Debug, Args)]
struct CheckArgs {
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
    config: PathBuf,
    #[arg(long)]
    fail_on_warning: bool,
    #[arg(long)]
    no_report: bool,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
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
    #[arg(long)]
    typescript_helpers: bool,
}

#[derive(Debug, Args)]
struct ReportArgs {
    #[arg(long)]
    manifest: PathBuf,
}

#[derive(Debug, Args)]
struct ReviewArgs {
    #[arg(long)]
    manifest: PathBuf,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    stdout: bool,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    ai: bool,
    #[arg(long = "ai-provider", value_enum)]
    ai_provider: Option<AiProviderArg>,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    metadata_only: bool,
    #[arg(long)]
    include_images: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long = "ai-output")]
    ai_output: Option<PathBuf>,
    #[arg(long)]
    markdown: Option<PathBuf>,
    #[arg(long, default_value_t = DEFAULT_AI_REVIEW_MAX_IMAGES)]
    max_images: usize,
    #[arg(long, value_enum, default_value = "low")]
    image_detail: AiImageDetailArg,
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
struct SuggestArgs {
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
    config: PathBuf,
    #[arg(long)]
    metadata_only: bool,
    #[arg(long)]
    check: bool,
    #[arg(long, value_enum)]
    fail_on_severity: Option<SuggestFailSeverity>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    markdown: Option<PathBuf>,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
struct AltArgs {
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
    config: PathBuf,
    #[arg(long = "ai-provider", value_enum)]
    ai_provider: AiProviderArg,
    #[arg(long)]
    model: String,
    #[arg(long)]
    metadata_only: bool,
    #[arg(long)]
    include_images: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    markdown: Option<PathBuf>,
    #[arg(long)]
    force: bool,
    #[arg(long, default_value_t = DEFAULT_AI_REVIEW_MAX_IMAGES)]
    max_images: usize,
    #[arg(long, value_enum, default_value = "low")]
    image_detail: AiImageDetailArg,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SuggestFailSeverity {
    Advisory,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum AiImageDetailArg {
    Low,
    High,
    Auto,
    Original,
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

#[derive(Debug, Args)]
struct AiArgs {
    #[command(subcommand)]
    command: AiCommand,
}

#[derive(Debug, Subcommand)]
enum ManifestCommand {
    Export(ManifestExportArgs),
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    Init(AgentInitArgs),
    Task(AgentTaskArgs),
}

#[derive(Debug, Subcommand)]
enum AiCommand {
    Consent(AiConsentArgs),
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
    #[arg(long)]
    typescript_helpers: bool,
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
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
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

#[derive(Debug, Args)]
struct AgentTaskArgs {
    #[arg(long, value_enum, default_value = "generic")]
    agent: AgentTaskAgent,
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
    config: PathBuf,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AgentTaskAgent {
    Codex,
    ClaudeCode,
    Generic,
}

#[derive(Debug, Args)]
struct AiConsentArgs {
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
    config: PathBuf,
    #[arg(long = "ai-provider", value_enum)]
    ai_provider: AiProviderArg,
    #[arg(long)]
    model: String,
    #[arg(long)]
    metadata_only: bool,
    #[arg(long)]
    include_images: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AiProviderArg {
    Openai,
    Anthropic,
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
    let mut result = check_with_options(
        &config,
        CheckOptions {
            write_report: !args.no_report,
        },
    )?;
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
    let manifest_export = if let Some(output) = args.export_output {
        if args.typescript_helpers
            && !matches!(args.export_format, ManifestExportFormat::Typescript)
        {
            return Err(CliError::Core(DevimgError::config(
                &args.config,
                "--typescript-helpers requires --export-format typescript",
            )));
        }
        Some(DoctorManifestExportOptions {
            output,
            format: match args.export_format {
                ManifestExportFormat::Json => DoctorManifestExportFormat::Json,
                ManifestExportFormat::Typescript => DoctorManifestExportFormat::Typescript,
            },
            strip_prefix: args.strip_prefix,
            url_prefix: args.url_prefix,
            typescript_helpers: args.typescript_helpers,
        })
    } else {
        if args.typescript_helpers {
            return Err(CliError::Core(DevimgError::config(
                &args.config,
                "--typescript-helpers requires --export-output",
            )));
        }
        None
    };
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

fn command_review(args: ReviewArgs) -> Result<(), CliError> {
    if args.ai {
        return command_review_ai(args);
    }
    if review_has_ai_only_args(&args) {
        return Err(CliError::Core(DevimgError::config(
            &args.manifest,
            "AI review flags require --ai",
        )));
    }
    if args.stdout == args.output.is_some() {
        return Err(CliError::Core(DevimgError::config(
            &args.manifest,
            "devimg review requires exactly one of --output or --stdout",
        )));
    }

    let manifest = read_manifest(&args.manifest)?;
    let project_root = review_project_root(&args.manifest, &manifest);
    let asset_path_prefix = args
        .output
        .as_deref()
        .map(|output| review_asset_path_prefix(output, &project_root))
        .transpose()?
        .unwrap_or_default();
    let rendered = render_manifest_review(
        &manifest,
        &ManifestReviewOptions {
            asset_path_prefix,
            ..ManifestReviewOptions::default()
        },
    );

    if args.stdout {
        print!("{rendered}");
        return Ok(());
    }

    let output = args
        .output
        .expect("output exists because stdout/output exclusivity is checked");
    if output.exists() && !args.force {
        return Err(CliError::Core(DevimgError::UnsafeOverwrite {
            path: output,
        }));
    }
    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| DevimgError::io(parent, source))?;
        }
    }
    fs::write(&output, rendered).map_err(|source| DevimgError::io(&output, source))?;
    println!("Created {}", output.display());
    Ok(())
}

fn review_has_ai_only_args(args: &ReviewArgs) -> bool {
    args.ai_provider.is_some()
        || args.model.is_some()
        || args.metadata_only
        || args.include_images
        || args.dry_run
        || args.ai_output.is_some()
        || args.markdown.is_some()
        || args.max_images != DEFAULT_AI_REVIEW_MAX_IMAGES
        || args.image_detail != AiImageDetailArg::Low
}

fn command_review_ai(args: ReviewArgs) -> Result<(), CliError> {
    if args.stdout {
        return Err(CliError::Core(DevimgError::config(
            &args.manifest,
            "--stdout cannot be combined with --ai; use --ai-output for AI review JSON",
        )));
    }
    if args.metadata_only && args.include_images {
        return Err(CliError::Core(DevimgError::config(
            &args.manifest,
            "--metadata-only cannot be combined with --include-images",
        )));
    }
    if args.max_images == 0 {
        return Err(CliError::Core(DevimgError::config(
            &args.manifest,
            "--max-images must be greater than 0",
        )));
    }

    let provider = args
        .ai_provider
        .ok_or_else(|| {
            CliError::Core(DevimgError::config(
                &args.manifest,
                "devimg review --ai requires --ai-provider openai",
            ))
        })?
        .into_core();
    if provider != AiProvider::Openai {
        return Err(CliError::Core(DevimgError::config(
            &args.manifest,
            "devimg review --ai supports openai only in this release; Anthropic AI review is deferred",
        )));
    }
    let model = args.model.ok_or_else(|| {
        CliError::Core(DevimgError::config(
            &args.manifest,
            "devimg review --ai requires --model",
        ))
    })?;
    if model.trim().is_empty() {
        return Err(CliError::Core(DevimgError::config(
            &args.manifest,
            "--model cannot be empty",
        )));
    }

    let manifest = read_manifest(&args.manifest)?;
    let project_root = review_project_root(&args.manifest, &manifest);
    let ai_output = args
        .ai_output
        .clone()
        .unwrap_or_else(|| project_root.join(DEFAULT_AI_REVIEW_OUTPUT));
    validate_distinct_review_outputs(
        &args.manifest,
        args.output.as_deref(),
        &ai_output,
        args.markdown.as_deref(),
    )?;
    let mut outputs = vec![ai_output.clone()];
    if let Some(markdown) = &args.markdown {
        outputs.push(markdown.clone());
    }
    if let Some(output) = &args.output {
        outputs.push(output.clone());
    }
    for output in &outputs {
        if output.exists() && !args.force {
            return Err(CliError::Core(DevimgError::UnsafeOverwrite {
                path: output.clone(),
            }));
        }
    }

    let request = build_ai_review_request(
        &manifest,
        &AiReviewOptions {
            provider,
            model: model.clone(),
            command: "devimg review --ai".to_string(),
            manifest_path: args.manifest.clone(),
            project_root: project_root.clone(),
            dry_run: args.dry_run,
            include_images: args.include_images,
            image_detail: args.image_detail.label().to_string(),
            max_images: args.max_images,
            ai_output_path: Some(ai_output.clone()),
            markdown_path: args.markdown.clone(),
        },
    );
    if args.include_images && request.selected_images.is_empty() && !manifest.outputs.is_empty() {
        return Err(CliError::Core(DevimgError::config(
            &args.manifest,
            "no OpenAI-supported image outputs were found; supported formats are png, jpeg, webp, and non-animated gif",
        )));
    }

    let report = if args.dry_run {
        build_ai_review_dry_run_report(&request)
    } else {
        let key = read_ai_provider_key(
            &args.manifest,
            provider,
            std::slice::from_ref(&project_root),
            "devimg review --ai",
        )?;
        let payload = call_openai_review(&request, &project_root, &key)?;
        build_ai_review_report(&request, payload, true)
    };

    write_text_file(&ai_output, &ai_review_report_to_json(&report))?;
    println!("Created {}", ai_output.display());

    if let Some(markdown) = args.markdown {
        write_text_file(&markdown, &render_ai_review_markdown(&report))?;
        println!("Created {}", markdown.display());
    }

    if let Some(output) = args.output {
        let asset_path_prefix = review_asset_path_prefix(&output, &project_root)?;
        let rendered = render_manifest_review(
            &manifest,
            &ManifestReviewOptions {
                asset_path_prefix,
                ..ManifestReviewOptions::default()
            },
        );
        write_text_file(&output, &rendered)?;
        println!("Created {}", output.display());
    }

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

fn command_suggest(args: SuggestArgs) -> Result<(), CliError> {
    if !args.metadata_only {
        return Err(CliError::Core(DevimgError::config(
            &args.config,
            "devimg suggest currently requires --metadata-only",
        )));
    }
    if args.fail_on_severity.is_some() && !args.check {
        return Err(CliError::Core(DevimgError::config(
            &args.config,
            "--fail-on-severity requires --check",
        )));
    }

    let config = load_config(&args.config)?;
    let threshold = args
        .fail_on_severity
        .unwrap_or(SuggestFailSeverity::Warning);
    let json_output = if args.check {
        args.output.clone()
    } else {
        Some(
            args.output
                .clone()
                .unwrap_or_else(|| config.project.root.join("devimg-suggestions.json")),
        )
    };
    if let (Some(markdown), Some(json_output)) = (&args.markdown, &json_output) {
        if comparable_output_path(markdown)? == comparable_output_path(json_output)? {
            return Err(CliError::Core(DevimgError::config(
                json_output,
                "--output and --markdown must use different paths",
            )));
        }
    }

    let mut outputs = Vec::new();
    if let Some(json_output) = &json_output {
        outputs.push(json_output.clone());
    }
    if let Some(markdown) = &args.markdown {
        outputs.push(markdown.clone());
    }
    for output in &outputs {
        if output.exists() && !args.force {
            return Err(CliError::Core(DevimgError::UnsafeOverwrite {
                path: output.clone(),
            }));
        }
    }

    let report = suggest(
        &config,
        SuggestOptions {
            metadata_only: args.metadata_only,
        },
    )?;
    if let Some(json_output) = &json_output {
        write_text_file(json_output, &suggestion_report_to_json(&report))?;
        println!("Created {}", json_output.display());
    }

    if let Some(markdown) = args.markdown {
        write_text_file(&markdown, &render_suggestion_markdown(&report))?;
        println!("Created {}", markdown.display());
    }

    let blocking_count = if args.check {
        blocking_suggestion_count(&report, threshold)
    } else {
        0
    };
    let summary =
        render_suggest_terminal_summary(&report, args.check, threshold, blocking_count, &outputs);
    if args.check && blocking_count > 0 {
        Err(CliError::SuggestCheckFailed(summary))
    } else {
        print!("{summary}");
        Ok(())
    }
}

fn command_alt(args: AltArgs) -> Result<(), CliError> {
    if args.model.trim().is_empty() {
        return Err(CliError::Core(DevimgError::config(
            &args.config,
            "--model cannot be empty",
        )));
    }
    if args.metadata_only && args.include_images {
        return Err(CliError::Core(DevimgError::config(
            &args.config,
            "--metadata-only cannot be combined with --include-images",
        )));
    }
    if args.max_images == 0 {
        return Err(CliError::Core(DevimgError::config(
            &args.config,
            "--max-images must be greater than 0",
        )));
    }

    let provider = args.ai_provider.into_core();
    if args.include_images && provider != AiProvider::Openai {
        return Err(CliError::Core(DevimgError::config(
            &args.config,
            "devimg alt supports OpenAI image-backed alt-text generation only in 0.2.5; Anthropic alt text is deferred",
        )));
    }

    let config = load_config(&args.config)?;
    let manifest_path = config.project.root.join(&config.project.manifest);
    let manifest = read_manifest(&manifest_path)?;
    let output = args
        .output
        .clone()
        .unwrap_or_else(|| config.project.root.join(DEFAULT_AI_ALT_OUTPUT));
    validate_distinct_alt_outputs(&args.config, &output, args.markdown.as_deref())?;
    let mut outputs = vec![output.clone()];
    if let Some(markdown) = &args.markdown {
        outputs.push(markdown.clone());
    }
    for output_path in &outputs {
        if output_path.exists() && !args.force {
            return Err(CliError::Core(DevimgError::UnsafeOverwrite {
                path: output_path.clone(),
            }));
        }
    }

    let request = build_ai_alt_request(
        &manifest,
        &AiAltOptions {
            provider,
            model: args.model.clone(),
            command: "devimg alt".to_string(),
            config_path: args.config.clone(),
            manifest_path: manifest_path.clone(),
            project_root: config.project.root.clone(),
            dry_run: args.dry_run,
            include_images: args.include_images,
            image_detail: args.image_detail.label().to_string(),
            max_images: args.max_images,
            output_path: Some(output.clone()),
            markdown_path: args.markdown.clone(),
        },
    );
    if args.include_images && request.selected_images.is_empty() && !request.sources.is_empty() {
        return Err(CliError::Core(DevimgError::config(
            &args.config,
            "no OpenAI-supported source or generated image inputs were found; supported formats are png, jpeg, webp, and non-animated gif",
        )));
    }

    let report = if args.include_images && !args.dry_run {
        let key = read_ai_provider_key(
            &args.config,
            provider,
            std::slice::from_ref(&config.project.root),
            "devimg alt",
        )?;
        let payload = call_openai_alt(&request, &config.project.root, &key)?;
        build_ai_alt_report(&request, payload, true)
    } else {
        build_ai_alt_placeholder_report(&request)
    };

    write_text_file(&output, &ai_alt_report_to_json(&report))?;
    println!("Created {}", output.display());

    if let Some(markdown) = args.markdown {
        write_text_file(&markdown, &render_ai_alt_markdown(&report))?;
        println!("Created {}", markdown.display());
    }

    Ok(())
}

fn blocking_suggestion_count(report: &SuggestionReport, threshold: SuggestFailSeverity) -> usize {
    report
        .items
        .iter()
        .filter(|item| suggestion_blocks(item, threshold))
        .count()
}

fn suggestion_blocks(item: &SuggestionItem, threshold: SuggestFailSeverity) -> bool {
    match threshold {
        SuggestFailSeverity::Error => item.severity == "error",
        SuggestFailSeverity::Warning => {
            item.severity == "error" || (item.severity == "warning" && !item.acknowledged)
        }
        SuggestFailSeverity::Advisory => true,
    }
}

fn render_suggest_terminal_summary(
    report: &SuggestionReport,
    check: bool,
    threshold: SuggestFailSeverity,
    blocking_count: usize,
    outputs: &[PathBuf],
) -> String {
    let mut out = String::new();
    out.push_str("DevImg Suggestions Summary\n");
    out.push_str(&format!(
        "- Suggestions: `{}`\n",
        report.summary.suggestion_count
    ));
    out.push_str(&format!("- Errors: `{}`\n", report.summary.error_count));
    out.push_str(&format!("- Warnings: `{}`\n", report.summary.warning_count));
    out.push_str(&format!(
        "- Advisories: `{}`\n",
        report.summary.advisory_count
    ));
    if check {
        out.push_str(&format!("- Check threshold: `{}`\n", threshold.label()));
        out.push_str(&format!("- Blocking suggestions: `{blocking_count}`\n"));
        if outputs.is_empty() {
            out.push_str("- Output: no output written (read-only check)\n");
        }
    }
    for output in outputs {
        out.push_str(&format!("- Output written: `{}`\n", output.display()));
    }
    out
}

fn command_manifest(args: ManifestArgs) -> Result<(), CliError> {
    match args.command {
        ManifestCommand::Export(args) => command_manifest_export(args),
    }
}

fn command_agent(args: AgentArgs) -> Result<(), CliError> {
    match args.command {
        AgentCommand::Init(args) => command_agent_init(args),
        AgentCommand::Task(args) => command_agent_task(args),
    }
}

fn command_ai(args: AiArgs) -> Result<(), CliError> {
    match args.command {
        AiCommand::Consent(args) => command_ai_consent(args),
    }
}

fn command_ai_consent(args: AiConsentArgs) -> Result<(), CliError> {
    if args.model.trim().is_empty() {
        return Err(CliError::Core(DevimgError::config(
            &args.config,
            "--model cannot be empty",
        )));
    }
    if args.metadata_only && args.include_images {
        return Err(CliError::Core(DevimgError::config(
            &args.config,
            "--metadata-only cannot be combined with --include-images",
        )));
    }

    let provider = args.ai_provider.into_core();
    let config = load_config(&args.config)?;
    if !args.dry_run {
        let _key = read_ai_provider_key(
            &args.config,
            provider,
            std::slice::from_ref(&config.project.root),
            "devimg ai consent",
        )?;
    }
    if let Some(output) = &args.output {
        if output.exists() && !args.force {
            return Err(CliError::Core(DevimgError::UnsafeOverwrite {
                path: output.clone(),
            }));
        }
    }

    let preview = build_ai_consent_preview(
        &config,
        &AiConsentOptions {
            provider,
            model: args.model,
            command: "devimg ai consent".to_string(),
            config_path: args.config.clone(),
            dry_run: args.dry_run,
            include_images: args.include_images,
            output_path: args.output.clone(),
        },
    )?;
    let rendered = ai_consent_preview_to_json(&preview);

    if let Some(output) = args.output {
        write_text_file(&output, &rendered)?;
        println!("Created {}", output.display());
    } else {
        print!("{rendered}");
    }

    Ok(())
}

fn validate_distinct_review_outputs(
    manifest_path: &Path,
    html_output: Option<&Path>,
    ai_output: &Path,
    markdown: Option<&Path>,
) -> Result<(), CliError> {
    let ai_comparable = comparable_output_path(ai_output)?;
    if let Some(html_output) = html_output {
        if comparable_output_path(html_output)? == ai_comparable {
            return Err(CliError::Core(DevimgError::config(
                manifest_path,
                "--output and --ai-output must use different paths",
            )));
        }
    }
    if let Some(markdown) = markdown {
        let markdown_comparable = comparable_output_path(markdown)?;
        if markdown_comparable == ai_comparable {
            return Err(CliError::Core(DevimgError::config(
                manifest_path,
                "--ai-output and --markdown must use different paths",
            )));
        }
        if let Some(html_output) = html_output {
            if comparable_output_path(html_output)? == markdown_comparable {
                return Err(CliError::Core(DevimgError::config(
                    manifest_path,
                    "--output and --markdown must use different paths",
                )));
            }
        }
    }
    Ok(())
}

fn validate_distinct_alt_outputs(
    config_path: &Path,
    output: &Path,
    markdown: Option<&Path>,
) -> Result<(), CliError> {
    if let Some(markdown) = markdown {
        if comparable_output_path(output)? == comparable_output_path(markdown)? {
            return Err(CliError::Core(DevimgError::config(
                config_path,
                "--output and --markdown must use different paths",
            )));
        }
    }
    Ok(())
}

fn read_ai_provider_key(
    error_path: &Path,
    provider: AiProvider,
    project_roots: &[PathBuf],
    command_name: &str,
) -> Result<String, CliError> {
    let env_var = provider.credential_env_var();
    if let Ok(value) = std::env::var(env_var) {
        if !value.trim().is_empty() {
            return Ok(value);
        }
    }

    for root in ai_env_roots(project_roots)? {
        if let Some(value) = read_env_file_value(&root.join(".env"), env_var)? {
            if !value.trim().is_empty() {
                return Ok(value);
            }
        }
    }

    Err(CliError::Core(DevimgError::config(
        error_path,
        format!("{env_var} is required for {command_name} unless --dry-run is passed"),
    )))
}

fn ai_env_roots(project_roots: &[PathBuf]) -> Result<Vec<PathBuf>, CliError> {
    let mut roots = Vec::new();
    for root in project_roots {
        push_unique_path(&mut roots, normalize_lexical(root));
    }
    let current_dir = std::env::current_dir().map_err(|source| DevimgError::io(".", source))?;
    push_unique_path(&mut roots, normalize_lexical(&current_dir));
    Ok(roots)
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn read_env_file_value(path: &Path, key: &str) -> Result<Option<String>, CliError> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(source) if source.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(CliError::Core(DevimgError::io(path, source))),
    };
    for line in raw.lines() {
        if let Some(value) = parse_env_line_value(line, key) {
            if !value.trim().is_empty() {
                return Ok(Some(value));
            }
        }
    }
    Ok(None)
}

fn parse_env_line_value(line: &str, key: &str) -> Option<String> {
    let line = line.trim_start();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
    let (name, value) = line.split_once('=')?;
    if name.trim() != key {
        return None;
    }
    let mut value = value.trim().to_string();
    if !value.starts_with('"') && !value.starts_with('\'') {
        if let Some((before_comment, _)) = value.split_once(" #") {
            value = before_comment.trim_end().to_string();
        }
    }
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        let quote = bytes[0];
        if (quote == b'"' || quote == b'\'') && bytes[value.len() - 1] == quote {
            value = value[1..value.len() - 1].to_string();
        }
    }
    Some(value)
}

fn call_openai_review(
    request: &AiReviewRequest,
    project_root: &Path,
    api_key: &str,
) -> Result<AiReviewProviderPayload, CliError> {
    let body = build_openai_review_body(request, project_root)?;
    let client = reqwest::blocking::Client::builder().build().map_err(|_| {
        CliError::Core(DevimgError::config(
            &request.manifest_path,
            "failed to construct OpenAI HTTP client",
        ))
    })?;
    let response = client
        .post("https://api.openai.com/v1/responses")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .map_err(|_| {
            CliError::Core(DevimgError::config(
                &request.manifest_path,
                "OpenAI Responses API request failed before a response was received",
            ))
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(CliError::Core(DevimgError::config(
            &request.manifest_path,
            format!(
                "OpenAI Responses API request failed with HTTP status {}",
                status.as_u16()
            ),
        )));
    }
    let value: Value = response.json().map_err(|_| {
        CliError::Core(DevimgError::config(
            &request.manifest_path,
            "OpenAI response was not valid JSON",
        ))
    })?;
    let output_text = extract_openai_output_text(&value).ok_or_else(|| {
        CliError::Core(DevimgError::config(
            &request.manifest_path,
            "OpenAI response did not include AI review JSON",
        ))
    })?;
    serde_json::from_str::<AiReviewProviderPayload>(output_text.trim()).map_err(|_| {
        CliError::Core(DevimgError::config(
            &request.manifest_path,
            "OpenAI response did not match the DevImg AI review schema",
        ))
    })
}

fn build_openai_review_body(
    request: &AiReviewRequest,
    project_root: &Path,
) -> Result<Value, CliError> {
    let mut user_content = vec![json!({
        "type": "input_text",
        "text": openai_review_user_text(request),
    })];
    if request.image_bytes_included {
        for image in &request.selected_images {
            let image_path = project_root.join(&image.output_path);
            let bytes =
                fs::read(&image_path).map_err(|source| DevimgError::io(&image_path, source))?;
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            user_content.push(json!({
                "type": "input_image",
                "image_url": format!("data:{};base64,{}", image.mime_type, encoded),
                "detail": openai_image_detail(&image.detail),
            }));
        }
    }

    Ok(json!({
        "model": request.model,
        "input": [
            {
                "role": "developer",
                "content": [
                    {
                        "type": "input_text",
                        "text": openai_review_developer_text(),
                    }
                ]
            },
            {
                "role": "user",
                "content": user_content
            }
        ],
        "text": {
            "format": openai_review_text_format()
        }
    }))
}

fn call_openai_alt(
    request: &AiAltRequest,
    project_root: &Path,
    api_key: &str,
) -> Result<AiAltProviderPayload, CliError> {
    let body = build_openai_alt_body(request, project_root)?;
    let client = reqwest::blocking::Client::builder().build().map_err(|_| {
        CliError::Core(DevimgError::config(
            &request.config_path,
            "failed to construct OpenAI HTTP client",
        ))
    })?;
    let response = client
        .post("https://api.openai.com/v1/responses")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .map_err(|_| {
            CliError::Core(DevimgError::config(
                &request.config_path,
                "OpenAI Responses API request failed before a response was received",
            ))
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(CliError::Core(DevimgError::config(
            &request.config_path,
            format!(
                "OpenAI Responses API request failed with HTTP status {}",
                status.as_u16()
            ),
        )));
    }
    let value: Value = response.json().map_err(|_| {
        CliError::Core(DevimgError::config(
            &request.config_path,
            "OpenAI response was not valid JSON",
        ))
    })?;
    let output_text = extract_openai_output_text(&value).ok_or_else(|| {
        CliError::Core(DevimgError::config(
            &request.config_path,
            "OpenAI response did not include alt-text JSON",
        ))
    })?;
    serde_json::from_str::<AiAltProviderPayload>(output_text.trim()).map_err(|_| {
        CliError::Core(DevimgError::config(
            &request.config_path,
            "OpenAI response did not match the DevImg alt-text schema",
        ))
    })
}

fn build_openai_alt_body(request: &AiAltRequest, project_root: &Path) -> Result<Value, CliError> {
    let mut user_content = vec![json!({
        "type": "input_text",
        "text": openai_alt_user_text(request),
    })];
    if request.image_bytes_included {
        for image in &request.selected_images {
            let image_path = project_root.join(&image.image_path);
            let bytes =
                fs::read(&image_path).map_err(|source| DevimgError::io(&image_path, source))?;
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            user_content.push(json!({
                "type": "input_image",
                "image_url": format!("data:{};base64,{}", image.mime_type, encoded),
                "detail": openai_image_detail(&image.detail),
            }));
        }
    }

    Ok(json!({
        "model": request.model,
        "input": [
            {
                "role": "developer",
                "content": [
                    {
                        "type": "input_text",
                        "text": openai_alt_developer_text(),
                    }
                ]
            },
            {
                "role": "user",
                "content": user_content
            }
        ],
        "text": {
            "format": openai_alt_text_format()
        }
    }))
}

fn openai_review_developer_text() -> &'static str {
    "You are reviewing DevImg-generated web image variants. Return JSON that matches the supplied schema. Treat every observation as advisory. Do not suggest editing generated image files by hand. Do not include secrets, API keys, data URLs, or raw image bytes. Focus only on crop risk, readability risk, excessive padding, low-resolution source, format-quality concern, and accessibility note."
}

fn openai_alt_developer_text() -> &'static str {
    "You draft concise web image alt text from DevImg metadata and image inputs. Return JSON that matches the supplied schema. Draft one record per source_path. Alt text is draft content for human review only. Do not include secrets, API keys, data URLs, raw image bytes, markdown, file names as descriptions, or instructions to edit application code. Prefer empty candidate_alt_text for decorative images. Warn for decorative images, text-heavy images, logos, screenshots, and uncertain descriptions."
}

fn openai_image_detail(detail: &str) -> &str {
    match detail {
        "low" | "high" | "auto" => detail,
        "original" => "high",
        _ => "low",
    }
}

fn openai_review_user_text(request: &AiReviewRequest) -> String {
    let metadata =
        serde_json::to_string_pretty(request).expect("AI review request serialization cannot fail");
    format!(
        "Review this DevImg manifest metadata and the optional selected image inputs. Return JSON only. Metadata:\n{metadata}"
    )
}

fn openai_alt_user_text(request: &AiAltRequest) -> String {
    let metadata =
        serde_json::to_string_pretty(request).expect("AI alt request serialization cannot fail");
    format!(
        "Draft alt text for these DevImg source images using the metadata and selected image inputs. Return JSON only. Metadata:\n{metadata}"
    )
}

fn openai_review_text_format() -> Value {
    json!({
        "type": "json_schema",
        "name": "devimg_ai_review",
        "strict": true,
        "schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "summary": { "type": "string" },
                "observations": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "category": {
                                "type": "string",
                                "enum": [
                                    "crop risk",
                                    "readability risk",
                                    "excessive padding",
                                    "low-resolution source",
                                    "format-quality concern",
                                    "accessibility note"
                                ]
                            },
                            "severity": {
                                "type": "string",
                                "enum": ["advisory"]
                            },
                            "source_path": { "type": "string" },
                            "preset": { "type": "string" },
                            "output_path": { "type": "string" },
                            "rationale": { "type": "string" },
                            "suggested_next_command": { "type": "string" }
                        },
                        "required": [
                            "category",
                            "severity",
                            "source_path",
                            "preset",
                            "output_path",
                            "rationale",
                            "suggested_next_command"
                        ]
                    }
                }
            },
            "required": ["summary", "observations"]
        }
    })
}

fn openai_alt_text_format() -> Value {
    json!({
        "type": "json_schema",
        "name": "devimg_alt_text",
        "strict": true,
        "schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "summary": { "type": "string" },
                "drafts": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "source_path": { "type": "string" },
                            "representative_image_path": { "type": "string" },
                            "candidate_alt_text": { "type": "string" },
                            "review_note": { "type": "string" },
                            "confidence": {
                                "type": "string",
                                "enum": ["low", "medium", "high"]
                            },
                            "image_category": {
                                "type": "string",
                                "enum": [
                                    "content-photo",
                                    "screenshot",
                                    "logo",
                                    "illustration",
                                    "diagram",
                                    "icon",
                                    "decorative",
                                    "text-heavy",
                                    "unknown"
                                ]
                            },
                            "warnings": {
                                "type": "array",
                                "items": {
                                    "type": "string",
                                    "enum": [
                                        "decorative",
                                        "text-heavy",
                                        "logo",
                                        "screenshot",
                                        "uncertain-description",
                                        "needs-human-review"
                                    ]
                                }
                            }
                        },
                        "required": [
                            "source_path",
                            "representative_image_path",
                            "candidate_alt_text",
                            "review_note",
                            "confidence",
                            "image_category",
                            "warnings"
                        ]
                    }
                }
            },
            "required": ["summary", "drafts"]
        }
    })
}

fn extract_openai_output_text(value: &Value) -> Option<String> {
    if let Some(text) = value.get("output_text").and_then(Value::as_str) {
        return Some(text.to_string());
    }

    let mut text = String::new();
    for output in value.get("output")?.as_array()? {
        let Some(content) = output.get("content").and_then(Value::as_array) else {
            continue;
        };
        for item in content {
            if let Some(part) = item.get("text").and_then(Value::as_str) {
                text.push_str(part);
            }
        }
    }
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn command_agent_task(args: AgentTaskArgs) -> Result<(), CliError> {
    let config = load_config(&args.config)?;
    let report = doctor(&config, DoctorOptions::default())?;
    let generated_artifacts = agent_task_generated_artifacts(&config);
    let rendered = render_agent_task(&args, &config, &report, &generated_artifacts);

    if let Some(output) = args.output {
        if is_protected_agent_instruction_path(&output) {
            return Err(CliError::Core(DevimgError::config(
                &output,
                "devimg agent task refuses to write task output to agent instruction paths",
            )));
        }
        if output.exists() && !args.force {
            return Err(CliError::Core(DevimgError::UnsafeOverwrite {
                path: output,
            }));
        }
        if let Some(parent) = output.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|source| DevimgError::io(parent, source))?;
            }
        }
        fs::write(&output, rendered).map_err(|source| DevimgError::io(&output, source))?;
        println!("Created {}", output.display());
    } else {
        print!("{rendered}");
    }

    Ok(())
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
    if args.typescript_helpers && !matches!(args.format, ManifestExportFormat::Typescript) {
        return Err(CliError::Core(DevimgError::config(
            &args.manifest,
            "--typescript-helpers requires --format typescript",
        )));
    }
    let manifest = read_manifest(&args.manifest)?;
    let options = ManifestExportOptions {
        strip_prefix: args.strip_prefix.clone(),
        url_prefix: args.url_prefix.clone(),
    };
    let rendered = match args.format {
        ManifestExportFormat::Json => manifest_export_to_json(&manifest, &options),
        ManifestExportFormat::Typescript => manifest_export_to_typescript_with_options(
            &manifest,
            &options,
            &ManifestTypescriptOptions {
                include_helpers: args.typescript_helpers,
            },
        ),
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
        "{report}\nHint: If outputs are missing or stale, regenerate them with `{}`. For budget failures, reduce image bytes or adjust budgets. If `--fail-on-warning` failed on quality diagnostics, tune quality, fit/crop, widths, or source assets in the config.\nNext: {}\n",
        optimize_command(config_path),
        doctor_command(config_path)
    )
}

fn render_core_error(error: &DevimgError) -> String {
    let mut out = format!("Error: {error}");
    match error {
        DevimgError::Config { message, .. }
            if message
                == "devimg agent task refuses to write task output to agent instruction paths" =>
        {
            out.push_str(
                "\nHint: choose a task output path such as `ai_tasks/devimg-agent-task.md` instead of an agent instruction file.",
            );
        }
        DevimgError::Config { message, .. }
            if message == "devimg suggest currently requires --metadata-only" =>
        {
            out.push_str("\nHint: run `devimg suggest --metadata-only` to generate deterministic local suggestions.");
        }
        DevimgError::Config { message, .. } if message == "--fail-on-severity requires --check" => {
            out.push_str("\nHint: run `devimg suggest --metadata-only --check --fail-on-severity warning` for a suggestion gate.");
        }
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
                "\nHint: devimg will not replace existing files unless you pass the command-specific overwrite flag (`--force` or `--allow-overwrite`).",
            );
        }
        DevimgError::Image { .. } => {
            out.push_str("\nHint: inspect the file with `devimg inspect <file>` or replace corrupt/mislabelled source images.");
        }
        DevimgError::Io { .. } | DevimgError::CheckFailed { .. } => {}
    }
    out
}

fn write_text_file(path: &Path, contents: &str) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| DevimgError::io(parent, source))?;
        }
    }
    fs::write(path, contents).map_err(|source| DevimgError::io(path, source))?;
    Ok(())
}

fn comparable_output_path(path: &Path) -> Result<PathBuf, CliError> {
    let current_dir = std::env::current_dir().map_err(|source| DevimgError::io(".", source))?;
    Ok(normalize_lexical(&if path.is_absolute() {
        path.to_path_buf()
    } else {
        current_dir.join(path)
    }))
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
    if args.typescript_helpers {
        command.push_str(" --typescript-helpers");
    }
    command.push_str(&format!(" --output {}", shell_arg_path(output)));
    command
}

fn review_project_root(manifest_path: &Path, manifest: &devimg_core::Manifest) -> PathBuf {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if !manifest.config_path.is_empty() {
        let config_path = PathBuf::from(&manifest.config_path);
        let config_path = if config_path.is_absolute() {
            config_path
        } else {
            current_dir.join(config_path)
        };
        if let Ok(config) = load_config(&config_path) {
            return config.project.root;
        }
        if let Some(parent) = config_path.parent() {
            return normalize_lexical(parent);
        }
    }
    if let Some(project_root) =
        infer_review_project_root_from_manifest_path(manifest_path, manifest)
    {
        return project_root;
    }
    if manifest_path.is_absolute() {
        if let Some(parent) = manifest_path.parent() {
            return normalize_lexical(parent);
        }
    }
    current_dir
}

fn infer_review_project_root_from_manifest_path(
    manifest_path: &Path,
    manifest: &devimg_core::Manifest,
) -> Option<PathBuf> {
    if !manifest_path.is_absolute() {
        return None;
    }
    let parent = manifest_path.parent()?;
    for ancestor in parent.ancestors() {
        if manifest.outputs.iter().any(|output| {
            ancestor.join(&output.output_path).exists()
                || ancestor.join(&output.source_path).exists()
        }) {
            return Some(normalize_lexical(ancestor));
        }
    }
    None
}

fn review_asset_path_prefix(output: &Path, project_root: &Path) -> Result<String, CliError> {
    let current_dir = std::env::current_dir().map_err(|source| DevimgError::io(".", source))?;
    let output = normalize_lexical(&if output.is_absolute() {
        output.to_path_buf()
    } else {
        current_dir.join(output)
    });
    let project_root = normalize_lexical(project_root);
    let output_parent = output.parent().unwrap_or(project_root.as_path());

    if output_parent == project_root {
        return Ok(String::new());
    }

    if let Ok(relative_parent) = output_parent.strip_prefix(&project_root) {
        let depth = relative_parent
            .components()
            .filter(|component| matches!(component, std::path::Component::Normal(_)))
            .count();
        if depth == 0 {
            Ok(String::new())
        } else {
            Ok((0..depth).map(|_| "..").collect::<Vec<_>>().join("/"))
        }
    } else if let Ok(relative_project_root) = project_root.strip_prefix(output_parent) {
        Ok(path_to_slash(relative_project_root))
    } else {
        Ok(path_to_slash(&project_root))
    }
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn path_to_slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn init_command(config_path: &Path) -> String {
    format!("devimg init{}", config_option(config_path))
}

fn optimize_command(config_path: &Path) -> String {
    format!(
        "devimg optimize{} --allow-overwrite",
        config_option(config_path)
    )
}

fn doctor_command(config_path: &Path) -> String {
    format!("devimg doctor{}", config_option(config_path))
}

fn check_command(config_path: &Path) -> String {
    format!("devimg check{}", config_option(config_path))
}

fn config_option(config_path: &Path) -> String {
    if config_path == Path::new(DEFAULT_CONFIG_PATH) {
        String::new()
    } else {
        format!(" --config {}", shell_arg_path(config_path))
    }
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

impl SuggestFailSeverity {
    fn label(self) -> &'static str {
        match self {
            Self::Advisory => "advisory",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

impl AiProviderArg {
    fn into_core(self) -> AiProvider {
        match self {
            Self::Openai => AiProvider::Openai,
            Self::Anthropic => AiProvider::Anthropic,
        }
    }
}

impl AiImageDetailArg {
    fn label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::High => "high",
            Self::Auto => "auto",
            Self::Original => "original",
        }
    }
}

struct AgentTaskGeneratedArtifacts {
    generated_variants: Vec<String>,
    manifest_read_error: Option<String>,
}

fn agent_task_generated_artifacts(config: &devimg_core::Config) -> AgentTaskGeneratedArtifacts {
    let manifest_path = agent_task_manifest_path(config);
    match read_manifest(&manifest_path) {
        Ok(manifest) => {
            let generated_variants = manifest
                .outputs
                .into_iter()
                .map(|output| output.output_path)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            AgentTaskGeneratedArtifacts {
                generated_variants,
                manifest_read_error: None,
            }
        }
        Err(_) if !manifest_path.exists() => AgentTaskGeneratedArtifacts {
            generated_variants: Vec::new(),
            manifest_read_error: None,
        },
        Err(error) => AgentTaskGeneratedArtifacts {
            generated_variants: Vec::new(),
            manifest_read_error: Some(error.to_string()),
        },
    }
}

fn agent_task_manifest_path(config: &devimg_core::Config) -> PathBuf {
    normalize_lexical(&if config.project.manifest.is_absolute() {
        config.project.manifest.clone()
    } else {
        config.project.root.join(&config.project.manifest)
    })
}

fn render_agent_task(
    args: &AgentTaskArgs,
    config: &devimg_core::Config,
    report: &devimg_core::DoctorReport,
    generated_artifacts: &AgentTaskGeneratedArtifacts,
) -> String {
    let commands = agent_instruction_commands(&args.config);
    let agent = args.agent.label();
    let frameworks = comma_or_none(&report.frameworks);
    let manifest_helpers = comma_or_none(&report.manifest_helpers);
    let source_outputs = config
        .sources
        .iter()
        .map(|source| path_to_slash(&source.output))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let mut out = String::new();
    out.push_str("# DevImg Agent Task\n\n");
    out.push_str("Use this Markdown as local task context for a coding agent. It is generated from deterministic DevImg state and does not call external AI providers.\n\n");
    out.push_str("## Scope\n\n");
    out.push_str(&format!("- Selected agent: `{agent}`\n"));
    out.push_str("- Mode: `local-only`\n");
    out.push_str(&format!("- Config: `{}`\n", report.config_path));
    out.push_str(&format!("- Project root: `{}`\n", report.project_root));
    out.push_str(&format!("- Doctor status: `{}`\n", report.status));
    out.push_str("- Privacy: do not send image bytes, filenames, paths, metadata, API keys, or task output to external services unless a human explicitly asks for a later provider-backed workflow.\n");
    out.push_str("- Provider calls: none. Do not read, print, or persist `OPENAI_API_KEY` or `ANTHROPIC_API_KEY` for this task.\n\n");

    out.push_str("## Deterministic State\n\n");
    out.push_str(&format!("- Manifest: `{}`\n", report.manifest_path));
    out.push_str(&format!("- Report: `{}`\n", report.report_path));
    out.push_str(&format!("- Frameworks: `{frameworks}`\n"));
    out.push_str(&format!("- Manifest helpers: `{manifest_helpers}`\n"));
    out.push_str(&format!(
        "- Source images: `{}`\n",
        report.source_image_count
    ));
    out.push_str(&format!(
        "- Variants planned: `{}`\n",
        report.planned_variant_count
    ));
    out.push_str(&format!(
        "- Variants generated: `{}`\n",
        report.generated_variant_count
    ));
    out.push_str(&format!("- Source bytes: `{}`\n", report.source_bytes));
    out.push_str(&format!("- Output bytes: `{}`\n", report.output_bytes));
    out.push_str(&format!("- Budget status: `{}`\n", report.budget.status));
    out.push_str(&format!("- Next command: `{}`\n\n", report.next_command));

    out.push_str("## Checks\n\n");
    if report.checks.is_empty() {
        out.push_str("No checks reported.\n");
    } else {
        for check in &report.checks {
            out.push_str(&format!(
                "- `{}` `{}`: {}\n",
                check.status, check.name, check.message
            ));
        }
    }

    out.push_str("\n## Issues\n\n");
    push_diagnostics(&mut out, &report.issues, "No issues reported.");

    out.push_str("\n## Warnings\n\n");
    push_diagnostics(&mut out, &report.warnings, "No warnings reported.");

    out.push_str("\n## Acknowledged Warnings\n\n");
    push_diagnostics(
        &mut out,
        &report.acknowledged_warnings,
        "No acknowledged warnings reported.",
    );

    out.push_str("\n## Generated Artifacts\n\n");
    out.push_str(&format!("- Manifest file: `{}`\n", report.manifest_path));
    out.push_str(&format!("- Markdown report: `{}`\n", report.report_path));
    push_named_paths(
        &mut out,
        "Source output directories",
        &source_outputs,
        "No source output directories were configured.",
    );
    push_named_paths(
        &mut out,
        "Manifest helper files",
        &report.manifest_helpers,
        "No manifest helper files were detected.",
    );
    out.push_str("- Review artifact convention: `.devimg/review.html`\n");
    if let Some(error) = &generated_artifacts.manifest_read_error {
        out.push_str(&format!("- Manifest read note: `{error}`\n"));
    }
    push_named_paths(
        &mut out,
        "Generated variant paths from the current manifest",
        &generated_artifacts.generated_variants,
        "No generated variant paths were read from the current manifest.",
    );

    out.push_str("\n## File Ownership\n\n");
    out.push_str("Agents may edit these when the task requires it:\n\n");
    out.push_str(&format!("- DevImg config: `{}`\n", report.config_path));
    for source in &config.sources {
        out.push_str(&format!(
            "- Source images for `{}`: `{}`\n",
            source.name,
            path_to_slash(&source.input)
        ));
    }
    out.push_str("- Application code that consumes generated manifest exports.\n");
    out.push_str("- Documentation or workflow files that describe/run DevImg.\n\n");
    out.push_str("Agents must not hand-edit these generated files:\n\n");
    out.push_str(&format!("- `{}`\n", report.manifest_path));
    out.push_str(&format!("- `{}`\n", report.report_path));
    for output in &source_outputs {
        out.push_str(&format!("- `{output}`\n"));
    }
    for helper in &report.manifest_helpers {
        out.push_str(&format!("- `{helper}`\n"));
    }
    out.push_str("- `.devimg/review.html`\n");
    out.push_str("- Existing agent instruction files such as `AGENTS.md`, `CLAUDE.md`, `.claude/**`, `.codex/**`, `.cursor/**`, and `.github/copilot-instructions.md`.\n");

    out.push_str("\n## Regeneration Commands\n\n");
    out.push_str("```bash\n");
    out.push_str(&format!("{}\n", commands.doctor));
    out.push_str(&format!("{}\n", commands.optimize));
    out.push_str(&format!("{}\n", commands.check));
    out.push_str(&format!("{}\n", commands.doctor));
    out.push_str("```\n\n");
    out.push_str("Regenerate checked-in manifest helpers with a project-specific `devimg manifest export` command that matches the helper's original `--format`, `--strip-prefix`, `--url-prefix`, and `--typescript-helpers` options.\n\n");
    out.push_str("Regenerate a local static review artifact when visual review is needed:\n\n");
    out.push_str("```bash\n");
    out.push_str(&format!(
        "devimg review --manifest {} --output .devimg/review.html\n",
        shell_arg(&report.manifest_path)
    ));
    out.push_str("```\n\n");

    out.push_str("## Next Commands\n\n");
    out.push_str(&format!(
        "- Immediate next command: `{}`\n",
        report.next_command
    ));
    out.push_str(&format!(
        "- If source images or config changed: `{}`\n",
        commands.optimize
    ));
    out.push_str(&format!("- Before finishing: `{}`\n", commands.check));
    out.push_str(&format!("- Confirm final state: `{}`\n\n", commands.doctor));

    out.push_str("## Final Response Guidance\n\n");
    out.push_str(args.agent.final_response_guidance());
    out.push('\n');

    out
}

fn push_diagnostics(out: &mut String, diagnostics: &[devimg_core::DoctorDiagnostic], empty: &str) {
    if diagnostics.is_empty() {
        out.push_str(empty);
        out.push('\n');
        return;
    }
    for diagnostic in diagnostics {
        out.push_str(&format!(
            "- `{}` at `{}`: {}\n  Hint: {}\n",
            diagnostic.code, diagnostic.path, diagnostic.message, diagnostic.hint
        ));
    }
}

fn push_named_paths(out: &mut String, label: &str, paths: &[String], empty: &str) {
    out.push_str(&format!("- {label}:\n"));
    if paths.is_empty() {
        out.push_str(&format!("  - {empty}\n"));
        return;
    }
    for path in paths {
        out.push_str(&format!("  - `{path}`\n"));
    }
}

fn comma_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn is_protected_agent_instruction_path(path: &Path) -> bool {
    if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
        let normalized = file_name.to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "agents.md"
                | "claude.md"
                | "gemini.md"
                | "copilot-instructions.md"
                | ".cursorrules"
                | ".windsurfrules"
                | ".clinerules"
        ) {
            return true;
        }
    }

    path.components().any(|component| match component {
        std::path::Component::Normal(part) => {
            matches!(
                part.to_str(),
                Some(".claude" | ".codex" | ".cursor" | ".windsurf")
            )
        }
        _ => false,
    })
}

impl AgentTaskAgent {
    fn label(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude-code",
            Self::Generic => "generic",
        }
    }

    fn final_response_guidance(self) -> &'static str {
        match self {
            Self::Codex => {
                "Codex final response: summarize changed source/config files, regenerated artifacts, verification commands with pass/fail results, intentionally deferred provider-backed behavior, and any remaining risks or follow-up items."
            }
            Self::ClaudeCode => {
                "Claude Code final response: use concise Markdown sections for Summary, Tests, Deferred Work, and Risks; mention that the task stayed local-only and that existing agent instruction files were not overwritten."
            }
            Self::Generic => {
                "Generic Markdown final response: list changes, generated files, commands run, pass/fail results, deferred later-version behavior, and follow-up items in plain Markdown."
            }
        }
    }
}

struct AgentInstructionFile {
    relative_path: PathBuf,
    contents: String,
}

struct AgentInstructionCommands {
    config_path: String,
    config_note: String,
    doctor: String,
    optimize: String,
    check: String,
    doctor_export: String,
}

fn agent_instruction_files(args: &AgentInitArgs) -> Vec<AgentInstructionFile> {
    let commands = agent_instruction_commands(&args.config);
    let codex = AgentInstructionFile {
        relative_path: PathBuf::from("AGENTS.md"),
        contents: codex_agent_instructions(&commands),
    };
    let claude_memory = AgentInstructionFile {
        relative_path: PathBuf::from("CLAUDE.md"),
        contents: claude_agent_instructions(&commands),
    };
    let claude_command = AgentInstructionFile {
        relative_path: PathBuf::from(".claude/commands/devimg-doctor.md"),
        contents: claude_devimg_command(&commands),
    };

    match args.target {
        AgentTarget::Codex => vec![codex],
        AgentTarget::Claude => vec![claude_memory, claude_command],
        AgentTarget::Both => vec![codex, claude_memory, claude_command],
    }
}

fn agent_instruction_commands(config_path: &Path) -> AgentInstructionCommands {
    let config_path_label = shell_arg_path(config_path);
    let config_note = if config_path == Path::new(DEFAULT_CONFIG_PATH) {
        "DevImg uses `devimg.toml` by default; pass `--config <path>` only when this project uses a custom config file.".to_string()
    } else {
        format!(
            "This project uses `{config_path_label}` as its DevImg config; keep passing `--config {config_path_label}`."
        )
    };
    let doctor = doctor_command(config_path);
    let optimize = optimize_command(config_path);
    let check = check_command(config_path);
    let doctor_export = format!(
        "{doctor} --export-output lib/devimg.generated.ts --export-format typescript --strip-prefix public --url-prefix /"
    );

    AgentInstructionCommands {
        config_path: config_path_label,
        config_note,
        doctor,
        optimize,
        check,
        doctor_export,
    }
}

fn codex_agent_instructions(commands: &AgentInstructionCommands) -> String {
    let config_path = &commands.config_path;
    let config_note = &commands.config_note;
    let doctor = &commands.doctor;
    let optimize = &commands.optimize;
    let check = &commands.check;
    let doctor_export = &commands.doctor_export;

    format!(
        r#"# DevImg Agent Instructions

## Image Pipeline Workflow

- {config_note}
- Run `{doctor}` before editing source images, DevImg config, generated variants, manifests, reports, or app image helper files.
- After image source or config changes, run `{optimize}`.
- If the project checks in a manifest helper, regenerate it with `devimg manifest export`.
- If that helper was generated with `--typescript-helpers`, use the same flag for regeneration and drift checks.
- When crop or quality needs visual review, run `devimg review --manifest public/images/devimg-manifest.json --output .devimg/review.html`.
- Run `{check}` before finishing.
- Run `{doctor}` again to confirm the project is healthy.
- Treat warning output such as `quality:cover-crop` or `quality:low-lossy-quality` as a review signal; do not silently auto-tune config without user approval.
- Use `[[warnings.acknowledge]]` only after visual review, scoped to the exact source/preset or output, with a human-readable reason.
- If this repository has `docs/agent-contract.md`, follow it as the DevImg file ownership policy.
- Commit generated image variants, `devimg-manifest.json`, `devimg-report.md`, and checked-in manifest helper files together.
- Do not edit generated image variants, manifests, reports, or generated helper modules by hand.
- Do not overwrite existing agent instruction files or skills without explicit user approval.

Recommended loop:

```bash
{doctor}
{optimize}
devimg manifest export --manifest public/images/devimg-manifest.json --strip-prefix public --url-prefix / --format typescript --output lib/devimg.generated.ts
devimg review --manifest public/images/devimg-manifest.json --output .devimg/review.html
{check}
{doctor_export}
```

If the project uses different manifest or helper paths, inspect `{config_path}` and adjust the manifest export command before running it. Keep `--typescript-helpers` only when the checked-in TypeScript helper uses the generated lookup functions.
"#
    )
}

fn claude_agent_instructions(commands: &AgentInstructionCommands) -> String {
    let config_path = &commands.config_path;
    let config_note = &commands.config_note;
    let doctor = &commands.doctor;
    let optimize = &commands.optimize;
    let check = &commands.check;
    let doctor_export = &commands.doctor_export;

    format!(
        r#"# DevImg Agent Instructions

Use these instructions when working with generated web image assets.

- {config_note}
- Start with `{doctor}` before changing source images, DevImg config, generated variants, manifests, reports, or app image helper files.
- Regenerate outputs with `{optimize}` after image source or config changes.
- Regenerate checked-in manifest helpers with `devimg manifest export` when the project uses them.
- Include `--typescript-helpers` when the checked-in TypeScript helper uses generated lookup functions.
- Run `devimg review --manifest public/images/devimg-manifest.json --output .devimg/review.html` when crop or quality needs visual review.
- Validate with `{check}` and then run `{doctor}` again.
- Treat warning output such as `quality:cover-crop` or `quality:low-lossy-quality` as a review signal; do not silently auto-tune config without user approval.
- Use `[[warnings.acknowledge]]` only after visual review, scoped to the exact source/preset or output, with a human-readable reason.
- If this repository has `docs/agent-contract.md`, follow it as the DevImg file ownership policy.
- Commit generated image variants, `devimg-manifest.json`, `devimg-report.md`, and checked-in manifest helper files together.
- Never hand-edit generated image variants, manifests, reports, or generated helper modules.
- Never overwrite existing agent instruction files, Claude commands, or Codex skills without explicit user approval.

Recommended loop:

```bash
{doctor}
{optimize}
devimg manifest export --manifest public/images/devimg-manifest.json --strip-prefix public --url-prefix / --format typescript --output lib/devimg.generated.ts
devimg review --manifest public/images/devimg-manifest.json --output .devimg/review.html
{check}
{doctor_export}
```

If this project uses a different manifest path or does not check in a generated helper, inspect `{config_path}` and adjust or skip the manifest export step. Keep `--typescript-helpers` only when the checked-in TypeScript helper uses the generated lookup functions.
"#
    )
}

fn claude_devimg_command(commands: &AgentInstructionCommands) -> String {
    let doctor = &commands.doctor;
    let optimize = &commands.optimize;
    let check = &commands.check;

    format!(
        r#"---
description: Diagnose and update DevImg generated image assets
argument-hint: [config path]
---

Run the DevImg image pipeline workflow. Use `$ARGUMENTS` as a custom config path when provided; otherwise use DevImg's default `devimg.toml`.

Steps:

1. Run `{doctor}`, or `devimg doctor --config <config>` when `$ARGUMENTS` supplies a custom path.
2. If source images or config changed, run `{optimize}`, or `devimg optimize --config <config> --allow-overwrite` for a custom path.
3. If the project checks in a manifest helper, run `devimg manifest export` with the project manifest/helper paths, including `--typescript-helpers` only when that helper uses generated lookup functions.
4. If crop or quality needs visual review, run `devimg review --manifest <manifest> --output .devimg/review.html`.
5. Run `{check}`, or `devimg check --config <config>` for a custom path.
6. Run `{doctor}` again, or `devimg doctor --config <config>` again for a custom path.

Rules:

- Do not hand-edit generated variants, manifests, reports, or helper modules.
- Do not overwrite existing agent instruction files, Claude commands, or Codex skills without explicit user approval.
- Treat warning output such as `quality:cover-crop` or `quality:low-lossy-quality` as a review signal; do not silently auto-tune config without user approval.
- Use `[[warnings.acknowledge]]` only after visual review, scoped to the exact source/preset or output, with a human-readable reason.
- If this repository has `docs/agent-contract.md`, follow it as the DevImg file ownership policy.
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

#[cfg(test)]
mod tests {
    use super::{extract_openai_output_text, parse_env_line_value};
    use serde_json::json;

    #[test]
    fn env_line_parser_extracts_key_without_comments() {
        assert_eq!(
            parse_env_line_value("OPENAI_API_KEY=test-secret # local", "OPENAI_API_KEY").as_deref(),
            Some("test-secret")
        );
        assert_eq!(
            parse_env_line_value("export OPENAI_API_KEY=\"quoted secret\"", "OPENAI_API_KEY")
                .as_deref(),
            Some("quoted secret")
        );
        assert_eq!(
            parse_env_line_value("ANTHROPIC_API_KEY=other", "OPENAI_API_KEY"),
            None
        );
    }

    #[test]
    fn openai_response_text_extraction_supports_responses_shape() {
        let value = json!({
            "output": [
                {
                    "type": "message",
                    "content": [
                        {
                            "type": "output_text",
                            "text": "{\"summary\":\"ok\",\"observations\":[]}"
                        }
                    ]
                }
            ]
        });

        assert_eq!(
            extract_openai_output_text(&value).as_deref(),
            Some("{\"summary\":\"ok\",\"observations\":[]}")
        );
    }
}

#[derive(Debug)]
enum CliError {
    Parse(clap::Error),
    Core(DevimgError),
    CheckFailed(String),
    DoctorFailed { report: String, json: bool },
    SuggestCheckFailed(String),
}

impl From<DevimgError> for CliError {
    fn from(value: DevimgError) -> Self {
        Self::Core(value)
    }
}
