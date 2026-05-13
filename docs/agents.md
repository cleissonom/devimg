# AI Agent Workflow

Run `devimg doctor --config devimg.toml` before changing source images, `devimg.toml`, generated variants, manifests, reports, or app helper files. The doctor output is the project-state contract for Codex, Claude Code, and other coding agents.

The full agent contract lives in `docs/agent-contract.md`. Use it as the file ownership and safety policy when a project has DevImg-managed assets.

Roadmap planning for future AI-assisted features lives in `docs/roadmap-v0.2-ai.md`. Until those features exist, AI agents should rely on deterministic DevImg commands and treat any generated prose as reviewable suggestions, not source-of-truth pipeline behavior.

Recommended loop:

```bash
devimg doctor --config devimg.toml
devimg optimize --config devimg.toml --allow-overwrite
devimg manifest export --manifest public/images/devimg-manifest.json --strip-prefix public --url-prefix / --format typescript --output lib/devimg.generated.ts
devimg review --manifest public/images/devimg-manifest.json --output .devimg/review.html
devimg check --config devimg.toml
devimg doctor --config devimg.toml --export-output lib/devimg.generated.ts --export-format typescript --strip-prefix public --url-prefix /
```

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
