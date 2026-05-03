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
