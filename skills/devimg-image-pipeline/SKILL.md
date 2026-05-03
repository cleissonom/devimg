---
name: devimg-image-pipeline
description: Run DevImg image pipeline workflows safely in frontend repositories. Use when Codex needs to inspect, regenerate, verify, or review DevImg-managed source images, generated variants, manifests, Markdown reports, manifest exports, or project agent instructions.
---

# DevImg Image Pipeline

## Workflow

1. Locate the config, usually `devimg.toml`.
2. Run `devimg doctor --config <config>` before editing image sources, config, generated variants, manifests, reports, or app helper files.
3. If image sources or config changed, run `devimg optimize --config <config> --allow-overwrite`.
4. If the project checks in a manifest helper, regenerate it with `devimg manifest export`.
5. Run `devimg check --config <config>`.
6. Run `devimg doctor --config <config>` again before finishing.

## Rules

- Treat `devimg doctor` as the project-state contract.
- Use `devimg doctor --json` when deterministic machine-readable state helps.
- Do not hand-edit generated image variants, manifests, Markdown reports, or generated helper modules.
- Commit generated image variants, `devimg-manifest.json`, `devimg-report.md`, and checked-in manifest helpers together.
- Do not overwrite existing `AGENTS.md`, `CLAUDE.md`, `.claude/commands/**`, `.claude/skills/**`, or other agent instruction files unless the user explicitly asks.
- If manifest/helper paths differ, inspect `devimg.toml` before running `manifest export`.

## Common Commands

```bash
devimg doctor --config devimg.toml
devimg optimize --config devimg.toml --allow-overwrite
devimg manifest export --manifest public/images/devimg-manifest.json --strip-prefix public --url-prefix / --format typescript --output lib/devimg.generated.ts
devimg check --config devimg.toml
devimg doctor --config devimg.toml --export-output lib/devimg.generated.ts --export-format typescript --strip-prefix public --url-prefix /
```

Use `devimg agent init --target codex|claude|both` to create safe project instructions when missing. If files already exist, prefer printing or merging a reviewed snippet instead of replacing them.
