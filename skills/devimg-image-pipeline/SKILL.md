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
5. When crop, composition, or quality needs visual inspection, run `devimg review --manifest <manifest> --output .devimg/review.html` or `--stdout`.
6. Run `devimg check --config <config>`.
7. Run `devimg doctor --config <config>` again before finishing.

## Rules

- Treat `devimg doctor` as the project-state contract.
- Follow `docs/agent-contract.md` when it exists.
- Use `devimg doctor --json` when deterministic machine-readable state helps.
- Treat skipped variants from `devimg optimize` as current outputs that were safely reused from the manifest. Continue to run `devimg check` after optimize.
- Treat `quality_warning` output as a review signal. Do not auto-tune image config silently; suggest explicit changes such as raising `quality`, reducing `widths`, changing `fit`/`crop`, using `fit = "contain"`, or replacing a too-small source image.
- Use `devimg review --stdout` for static HTML context without writing files, and do not overwrite an existing review artifact unless the user approves `--force`.
- In GitHub Actions, use the Action `review-output` input with `actions/upload-artifact` for visual review artifacts. Do not add PR comment bots or automatic commits unless the maintainer asks.
- Do not hand-edit generated image variants, manifests, Markdown reports, or generated helper modules.
- Commit generated image variants, `devimg-manifest.json`, `devimg-report.md`, and checked-in manifest helpers together.
- Do not overwrite existing `AGENTS.md`, `CLAUDE.md`, `.claude/commands/**`, `.claude/skills/**`, or other agent instruction files unless the user explicitly asks.
- If manifest/helper paths differ, inspect `devimg.toml` before running `manifest export`.
- If a checked-in TypeScript helper uses `--typescript-helpers`, include that flag when regenerating it or checking export drift.

## Common Commands

```bash
devimg doctor --config devimg.toml
devimg optimize --config devimg.toml --allow-overwrite
devimg manifest export --manifest public/images/devimg-manifest.json --strip-prefix public --url-prefix / --format typescript --output lib/devimg.generated.ts
devimg review --manifest public/images/devimg-manifest.json --output .devimg/review.html
devimg check --config devimg.toml
devimg doctor --config devimg.toml --export-output lib/devimg.generated.ts --export-format typescript --strip-prefix public --url-prefix /
```

Use `devimg agent init --target codex|claude|both` to create safe project instructions when missing. If files already exist, prefer printing or merging a reviewed snippet instead of replacing them.
