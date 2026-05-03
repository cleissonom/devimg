# AI Agent Workflow

Run `devimg doctor --config devimg.toml` before changing source images, `devimg.toml`, generated variants, manifests, reports, or app helper files. The doctor output is the project-state contract for Codex, Claude Code, and other coding agents.

Recommended loop:

```bash
devimg doctor --config devimg.toml
devimg optimize --config devimg.toml --allow-overwrite
devimg manifest export --manifest public/images/devimg-manifest.json --strip-prefix public --url-prefix / --format typescript --output lib/devimg.generated.ts
devimg check --config devimg.toml
devimg doctor --config devimg.toml --export-output lib/devimg.generated.ts --export-format typescript --strip-prefix public --url-prefix /
```

Use `devimg doctor --json` when an agent or CI job needs deterministic machine-readable state.

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
