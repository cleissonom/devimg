# AI Agent Workflow

Run `devimg agent task` before changing source images, `devimg.toml`, generated variants, manifests, reports, or app helper files. The generated task output wraps `devimg doctor` state into a project-state contract for Codex, Claude Code, and other coding agents.

The full agent contract lives in `docs/agent-contract.md`. Use it as the file ownership and safety policy when a project has DevImg-managed assets.

Roadmap planning for future AI-assisted features lives in `docs/roadmap-v0.2-ai.md`. `devimg agent task` is local-only task context; later provider-backed features remain future work. AI agents should rely on deterministic DevImg commands and treat any generated prose as reviewable suggestions, not source-of-truth pipeline behavior.

Recommended loop:

```bash
devimg doctor
devimg optimize --allow-overwrite
devimg manifest export --manifest public/images/devimg-manifest.json --strip-prefix public --url-prefix / --format typescript --output lib/devimg.generated.ts
devimg review --manifest public/images/devimg-manifest.json --output .devimg/review.html
devimg check
devimg doctor --export-output lib/devimg.generated.ts --export-format typescript --strip-prefix public --url-prefix /
```

`devimg.toml` is the default config path. Use `--config <path>` only when a project keeps the DevImg config somewhere else.

If the generated TypeScript helper was created with `--typescript-helpers`, use the same flag in `doctor --export-output`, `manifest export --check`, and Action export drift checks.

Use `devimg doctor --json` when an agent or CI job needs deterministic machine-readable state.

Treat warning entries such as `quality:cover-crop` or `quality:low-lossy-quality` as prompts for review, not as permission to auto-tune images. Prefer proposing explicit `devimg.toml` changes such as raising `quality`, reducing `widths`, changing `fit`/`crop`, using `fit = "contain"`, or replacing a too-small source image.

Use `[[warnings.acknowledge]]` only when the warning is intentional after visual review. Keep acknowledgements scoped to the exact `source`/`preset` or `output`, include a human-readable `reason`, and do not add broad acknowledgements that silence future warnings across a project.

`devimg optimize` may report skipped variants on unchanged runs. Treat skipped variants as successfully reused outputs, not as missing work. If stale variants are reported, continue with `devimg check` and `devimg doctor` to confirm whether the regenerated outputs and manifest are now current.

When reviewing manifest diffs, treat `devimg compare` metadata-only output changes separately from real changed outputs. Metadata-only changes keep the same output path, bytes, and content hash, so they usually indicate config-only metadata or DevImg operation-hash normalization rather than an image quality change.

Use `devimg review --stdout` when an agent needs static HTML context without creating a file. Use `--output .devimg/review.html` for a browser-openable local review artifact, and do not overwrite an existing artifact unless the user explicitly approves `--force`.

In GitHub Actions, prefer the built-in `review-output` input plus `actions/upload-artifact` for CI visual review artifacts. Do not add PR comment bots or automatic commits unless the maintainer explicitly asks for that workflow.

Do not edit generated files by hand. Commit generated variants, `devimg-manifest.json`, `devimg-report.md`, and checked-in manifest helper files together when they change.

Do not overwrite existing agent instruction files such as `AGENTS.md`, `CLAUDE.md`, or `.claude/skills/**`. If project-specific image-pipeline instructions are needed, add a reviewed snippet or a new documented section instead of replacing existing guidance.

## Generated Task Context

Use `devimg agent task` when an agent needs a per-task Markdown contract generated from current deterministic DevImg state:

```bash
devimg agent task --agent codex
devimg agent task --agent claude-code
devimg agent task --agent generic
```

The command reads `devimg.toml` by default. Pass `--config <path>` only when the project uses a custom config path:

```bash
devimg agent task --config examples/portfolio/devimg.toml --agent codex
```

By default, the task contract prints to stdout. Use `--output <path>` to write a Markdown task file, and `--force` only when replacing that task file is intentional:

```bash
devimg agent task --agent codex --output ai_tasks/devimg-agent-task.md
devimg agent task --agent codex --output ai_tasks/devimg-agent-task.md --force
```

`agent task` includes doctor checks, issues, warnings, acknowledged warnings, detected frameworks, manifest helper paths, generated artifact paths, file ownership guidance, regeneration commands, next commands, and selected-agent final response guidance.

The command does not call OpenAI, Anthropic, or any external provider. It refuses to write task output to agent instruction paths such as `AGENTS.md`, `CLAUDE.md`, `.claude/**`, `.codex/**`, `.cursor/**`, and `.github/copilot-instructions.md`; use a task file instead.

## Deterministic Suggestions

Use `devimg suggest --metadata-only` when an agent needs diffable JSON suggestions derived from current DevImg diagnostics:

```bash
devimg suggest --metadata-only
devimg suggest --metadata-only --check
devimg suggest --metadata-only --check --fail-on-severity warning
devimg suggest --metadata-only --output devimg-suggestions.json
devimg suggest --metadata-only --markdown devimg-suggestions.md
devimg suggest --metadata-only --check --output /tmp/devimg-suggestions.json --markdown /tmp/devimg-suggestions.md
devimg suggest --metadata-only --output devimg-suggestions.json --markdown devimg-suggestions.md --force
```

The command writes `devimg-suggestions.json` under the configured project root by default. It supports `--config <path>`, refuses to overwrite existing JSON or Markdown output unless `--force` is passed, and does not rewrite `devimg.toml`.

`--check` is read-only unless `--output` or `--markdown` is supplied explicitly. It exits `0` when no suggestion meets the threshold and `3` when a suggestion blocks. The default threshold is `warning`; use `error` to block only errors and `advisory` to block every suggestion.

Suggestions include severity, affected source/output metadata when DevImg can prove it, `affected_path`, diagnostic rationale, structured `suggested_config` data, commands, and `next_command`. Treat suggestions as review inputs for explicit source or config changes, not as permission to hand-edit generated variants, manifests, reports, helper exports, or review artifacts.

`suggest --metadata-only` is local-only. It does not call OpenAI, Anthropic, or any external provider, and it must not print or persist `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`.

## Provider Consent Preview

Use `devimg ai consent` when an agent or CI workflow needs a reviewed provider setup preview before future AI-backed commands:

```bash
devimg ai consent --ai-provider openai --model openai-dry-run-model --dry-run
devimg ai consent --ai-provider anthropic --model anthropic-dry-run-model --dry-run
devimg ai consent --ai-provider openai --model review-model --output /tmp/devimg-ai-consent.json
```

The command is deterministic and timestamp-free. It previews provider, model, command, config path, project root, metadata mode, dry-run status, output path, source files, manifest/report paths, and generated outputs from a readable manifest.

In `0.2.3`, `ai consent` performs no provider call. Dry runs do not require API keys. Non-dry-run previews require `OPENAI_API_KEY` for OpenAI or `ANTHROPIC_API_KEY` for Anthropic, but DevImg must never print, persist, or include key values in artifacts. Metadata-only is the default; `--include-images` only changes preview metadata in this release and does not send bytes anywhere.

## Generated Instructions

Use `devimg agent init` when a project does not already have image-pipeline agent guidance:

```bash
devimg agent init --target codex
devimg agent init --target claude
devimg agent init --target both
```

Targets:

- `codex`: creates `AGENTS.md`.
- `claude`: creates `CLAUDE.md` and `.claude/commands/devimg-doctor.md`.
- `both`: creates all of the above.

The command refuses to overwrite existing files. Use `--stdout` to print the suggested snippets for manual review, or `--force` only when replacing the whole target file is intentional.

Claude Code uses `CLAUDE.md` as project memory and `.claude/commands/*.md` for project slash commands. The generated Claude command is a prompt template for running the DevImg workflow, not an automatic file mutator.

## Codex Skill

This repo ships a reusable Codex skill at `skills/devimg-image-pipeline/`. To install it for local Codex use, copy that folder into `${CODEX_HOME:-~/.codex}/skills/`.

After installation, invoke it with `$devimg-image-pipeline` when updating source images, generated variants, manifests, reports, or manifest helper files.
