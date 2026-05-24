# Dev Image Pipeline

`devimg` is a Rust CLI and GitHub Action for deterministic web image variants. It scans configured PNG/JPEG/WebP image folders, generates responsive PNG/JPEG/WebP/AVIF outputs, writes a JSON manifest and Markdown report, and lets CI fail when generated images are missing, stale, or over budget.

DevImg has no web UI and no remote storage. The CLI is the source of truth; the GitHub Action wraps `devimg check` or `devimg optimize`, can verify checked-in manifest exports, and can create an optional static review artifact for CI upload.

## Where DevImg Fits

DevImg sits above image engines, framework image components, and hosted transformation services. Use it when a repository needs generated images to be reproducible, reviewable, and enforced in CI.

- Image engines such as `sharp` are excellent low-level processors; DevImg owns project config, manifests, reports, and stale/budget checks.
- Framework image components such as Next.js `Image` and Astro assets are useful runtime/build integrations; DevImg keeps generated variants and app helper exports explicit in the repo.
- Hosted image services are powerful for on-the-fly transformations; DevImg stays local-first and CI-first with no required remote storage.
- Compression tools are useful for one-off optimization; DevImg manages the repeatable source-to-variant workflow.

Detailed docs live in `docs/`: configuration, GitHub Action usage, release/distribution, architecture, compatibility, AI-agent workflow, and the public v0.2 AI roadmap.

## Quickstart

Install the CLI from crates.io:

```bash
cargo install devimg
```

Source installs require Rust 1.85 or newer. If your default toolchain is older, run `rustup update stable` or install with an explicit toolchain such as `cargo +1.85.1 install devimg`.

Then use the installed binary:

```bash
devimg init --stdout > devimg.toml
devimg doctor
devimg optimize
devimg check
```

From a local source checkout, use `cargo run -p devimg --` before each command:

```bash
cargo run -p devimg -- init --stdout > devimg.toml
# Or choose framework-friendly starter paths:
cargo run -p devimg -- init --profile next --stdout > devimg.toml
cargo run -p devimg -- doctor
cargo run -p devimg -- optimize
cargo run -p devimg -- check
cargo run -p devimg -- doctor
```

`devimg.toml` is the default config path. Use `--config <path>` only when a project keeps its config somewhere else, such as the example fixtures below.

Useful commands:

```bash
cargo run -p devimg -- doctor --config examples/portfolio/devimg.toml
cargo run -p devimg -- doctor --config examples/portfolio/devimg.toml --json
cargo run -p devimg -- agent init --target both --stdout
cargo run -p devimg -- optimize --config examples/portfolio/devimg.toml --dry-run
cargo run -p devimg -- report --manifest examples/portfolio/public/images/devimg-manifest.json
cargo run -p devimg -- review --manifest examples/portfolio/public/images/devimg-manifest.json --output examples/portfolio/.devimg/review.html
cargo run -p devimg -- optimize --config examples/dogfood/devimg.toml
cargo run -p devimg -- manifest export --manifest examples/portfolio/public/images/devimg-manifest.json
cargo run -p devimg -- compare --base old-devimg-manifest.json --head public/images/devimg-manifest.json
cargo run -p devimg -- inspect fixtures/images/sample.png
```

Recommended local loop:

```bash
devimg doctor
devimg optimize --allow-overwrite
devimg manifest export --manifest public/images/devimg-manifest.json --strip-prefix public --url-prefix / --format typescript --output lib/devimg.generated.ts
devimg check
devimg doctor --export-output lib/devimg.generated.ts --export-format typescript --strip-prefix public --url-prefix /
devimg review --manifest public/images/devimg-manifest.json --output .devimg/review.html --force
```

Repeated `optimize` runs are incremental. When the current manifest and output file prove a variant is already fresh, DevImg skips the expensive decode/resize/encode work and reports the skipped count. Operation hashes track transform inputs rather than every config byte, so config-only metadata changes such as warning acknowledgements can refresh the manifest/report without re-encoding unchanged images. If transform settings, source bytes, output bytes, or dimensions are stale, DevImg falls back to normal generation and keeps the existing safe-overwrite rules.

## Examples

See `examples/README.md` for maintained fixtures:

- `examples/portfolio`: small portfolio/blog/docs workflow with stable filenames.
- `examples/dogfood`: broader frontend workflow with content-hash filenames, social/card/logo/screenshot-like paths, `cover` and `contain` presets, helper export, and visual review coverage.

Generated example outputs are ignored and reproducible. Regenerate them with `devimg`; do not hand-edit generated variants, manifests, reports, helper exports, or review artifacts. The dogfood example is the best local fixture for public-story screenshots and review-artifact demos because it exercises content-hash filenames, helper exports, and the static HTML review flow.

## What Changes In A PR?

A normal image update should make the source change easy to review and keep generated outputs consistent:

- Source image or `devimg.toml` changes describe the intent.
- Generated variants update under the configured output directory.
- `devimg-manifest.json` records the exact source-to-output mapping, dimensions, bytes, hashes, preset, fit, crop, and format.
- Optional app helper exports such as `lib/devimg.generated.ts` keep framework code away from hard-coded content-hash filenames.
- `devimg-report.md` summarizes counts, byte totals, warnings, and budget status.
- Optional `.devimg/review.html` artifacts let maintainers inspect generated variants locally or from CI artifact upload.

Treat generated variants, manifests, reports, helper exports, and review artifacts as derived files. Refresh them with DevImg commands and review them together with the source/config change.

## Config

Copy `devimg.example.toml` or run `devimg init`. Use `devimg init --profile next`, `--profile astro`, or `--profile vite` to start with common framework paths while keeping the same config format.

```toml
[project]
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
formats = ["webp", "jpeg"] # add "avif" explicitly when you want AVIF variants
quality = 82
fit = "cover"
aspect_ratio = "16:9"
crop = "center"

[budgets]
max_total_bytes = "3mb"
max_file_bytes = "350kb"
fail_on_regression = true
```

Outputs are named as:

```text
{relative-source-dir}/{stem}.{preset}.{width}.{format}
```

Set `[project].content_hash_filenames = true` to include a generated-byte hash in output names:

```text
{relative-source-dir}/{stem}.{preset}.{width}.{hash}.{format}
```

Use hashed filenames before applying broad immutable CDN cache headers to generated assets.

## Manifest Consumption

Use `manifest export` to turn the generated manifest into an app-friendly source-to-variant mapping. This is useful when generated filenames include content hashes and the consuming app should not hand-edit those paths.

```bash
devimg manifest export \
  --manifest public/images/devimg-manifest.json \
  --strip-prefix public \
  --url-prefix / \
  --format typescript \
  --output lib/devimg.generated.ts
```

Use `--check` in CI to fail when a checked-in export is missing or stale without rewriting it:

```bash
devimg manifest export \
  --manifest public/images/devimg-manifest.json \
  --strip-prefix public \
  --url-prefix / \
  --format typescript \
  --output lib/devimg.generated.ts \
  --check
```

The stable export contract is:

- Top level: `version`, `generated_at`, `config_hash`, and `sources`.
- Each source: `source_path`, `source_hash`, `source_width`, `source_height`, `source_bytes`, and `variants`.
- Each variant: `src`, `output_path`, `preset`, `fit`, `width`, `height`, `format`, `bytes`, and `hash`.

Application code should select variants from the export instead of hard-coding generated filenames. `--strip-prefix public --url-prefix /` converts an output path such as `public/images/generated/card.project-card.640.jpeg` into `/images/generated/card.project-card.640.jpeg`.

The default JSON and TypeScript export shape is stable and data-only. For TypeScript apps that want generated lookup helpers instead of hand-written helper code, add `--typescript-helpers`:

```bash
devimg manifest export \
  --manifest public/images/devimg-manifest.json \
  --strip-prefix public \
  --url-prefix / \
  --format typescript \
  --typescript-helpers \
  --output lib/devimg.generated.ts
```

Helper-mode TypeScript still exports `DEVIMG_MANIFEST`; it also exports `findDevimgSource`, `listDevimgVariants`, and `findDevimgVariant`. The selector accepts `source`, plus optional `preset`, `format`, exact `width`, or `minWidth`. When `minWidth` is used and no large-enough variant exists, `findDevimgVariant` returns the largest matching variant as a practical fallback.

`devimg doctor` can also verify a checked-in export without rewriting it:

```bash
devimg doctor \
  --export-output lib/devimg.generated.ts \
  --export-format typescript \
  --strip-prefix public \
  --url-prefix /
```

Add `--typescript-helpers` to `doctor` and `manifest export --check` when the checked-in helper file was generated with helper mode.

## Manifest Compare

Use `compare` when reviewing image PRs or checking why a branch changed generated assets. Keep a copy of the old manifest, regenerate images on the branch, then compare the two manifests:

```bash
cp public/images/devimg-manifest.json /tmp/devimg-base-manifest.json
devimg optimize --allow-overwrite
devimg compare --base /tmp/devimg-base-manifest.json --head public/images/devimg-manifest.json
```

The human report shows variant count delta, output byte delta, added outputs, removed outputs, changed outputs, metadata-only output changes, unchanged outputs, and the largest head-manifest byte contributors. Metadata-only changes mean the output path, bytes, and content hash stayed the same while operation metadata changed, which is useful when reviewing config-only or DevImg-version metadata updates separately from real image changes. Use JSON output for CI or AI agents:

```bash
devimg compare \
  --base /tmp/devimg-base-manifest.json \
  --head public/images/devimg-manifest.json \
  --json
```

## Visual Review

Use `review` to turn a generated manifest into a local static HTML artifact for human review and AI-agent context:

```bash
devimg review \
  --manifest public/images/devimg-manifest.json \
  --output .devimg/review.html
```

Use `--stdout` when another tool should capture the artifact directly:

```bash
devimg review --manifest public/images/devimg-manifest.json --stdout
```

The review groups variants by source image, shows source and generated previews, dimensions, formats, byte sizes, hashes, largest sources, largest outputs, and manifest-only warnings such as upscaled variants or outputs larger than their source files. It has no external scripts, no CDN assets, and no tracking. Because it only reads the manifest, budget status is shown as not evaluated; run `devimg check` for budget enforcement.

`review --output` refuses to overwrite an existing file unless `--force` is passed. When written inside the project, for example `.devimg/review.html`, image links are made relative to the artifact so the file can be opened directly in a browser.

## Quality Diagnostics

DevImg emits advisory `quality:` warnings when config or generated outputs look suspicious. These warnings do not change generated images automatically and do not fail default `devimg check`; tune `devimg.toml` explicitly when a warning is valid for your project.

Warnings currently cover:

- Low JPEG/WebP/AVIF quality for assets that look screenshot-, banner-, card-, hero-, logo-, diagram-, UI-, or text-heavy.
- Requested widths that would require upscaling, including explicit `allow_upscale = true`.
- `fit = "cover"` plus an aspect ratio that crops a material part of the resized image.
- Generated outputs that are larger than their source file.

Use stricter CI when warnings should block a branch:

```bash
devimg check --fail-on-warning
```

Use `devimg doctor --json` when an AI agent or CI tool needs machine-readable warning entries such as `quality:cover-crop` or `quality:low-lossy-quality`. Use `devimg review` for manifest-only visual checks of upscaled outputs and output-size surprises.

When a warning is intentional, acknowledge it narrowly instead of weakening the preset for every image:

```toml
[[warnings.acknowledge]]
code = "quality:cover-crop"
source = "assets/images/accesstrace.png"
preset = "project-card"
reason = "Intentional card framing after visual review."
```

Acknowledged warnings move to an `Acknowledged Warnings` section in reports and `acknowledged_warnings` in `doctor --json`. They remain visible, but `devimg check --fail-on-warning` only fails on unacknowledged warnings. Prefer changing `quality`, `widths`, `fit`, `crop`, `allow_upscale`, or source assets when the warning represents a real regression.

## Framework Diagnostics

`devimg doctor` detects common frontend projects from config files and `package.json` dependencies:

- Next.js
- Astro
- Vite

Framework diagnostics are advisory warnings. They do not change image generation and do not fail `doctor` by themselves. Current hints cover mixed framework detection, public generated outputs without content-hash filenames, common helper files such as `lib/devimg.generated.ts`, and content-hash filenames without a checked manifest export passed through `--export-output`. For Next.js projects, DevImg explains that generated files under `public/` are static assets that Vercel/CDNs can cache directly, while `next/image` may optimize them again unless the app intentionally uses `img`/`picture`, uses `Image` with `unoptimized`, or deliberately layers framework optimization on top of DevImg-generated source variants.

For `fit = "cover"`, `crop` controls which part of the resized image is preserved when the aspect ratio requires cropping. It defaults to `center`. Use anchors such as `top`, `bottom`, `left`, `right`, `top-left`, or a normalized focal point:

```toml
crop = "top"
crop = { x = 0.5, y = 0.0 }
```

Use `[[overrides]]` to keep shared presets while changing transform behavior for specific source paths. Override paths are relative to the matching source input:

```toml
[[overrides]]
include = ["cli_tools.png"]
fit = "contain"
```

## Safety

- `doctor` is read-only. It does not generate images, rewrite reports, update manifests, or touch manifest export files.
- `optimize --dry-run` plans work without writing files.
- Existing unmanaged outputs are not overwritten unless config `overwrite = true` or CLI `--allow-overwrite` is used.
- Re-encoding strips metadata by default. `strip_metadata = false` is parsed, but the current encoders do not preserve source metadata.
- `check` fails on missing outputs, stale manifests, modified outputs, outdated config hashes, and byte budget violations. Add `--fail-on-warning` when advisory warnings should fail CI, or `--no-report` when a wrapper needs read-only validation without rewriting the Markdown report.

Exit codes are stable for CI:

- `0`: success or help output.
- `1`: runtime error outside config validation.
- `2`: usage or config error.
- `3`: `devimg check` failed, `devimg doctor` found required work, or `devimg manifest export --check` found a missing or stale export.
- `4`: unsafe overwrite refused.

## AI Agent Workflow

Codex, Claude Code, and similar tools should run `devimg doctor` before editing image sources, config, manifests, generated variants, or app helper files. After changes, run the local loop above and commit the generated variants, manifest, report, and checked-in helper files together.

Do not edit generated files by hand. If agent instruction files such as `AGENTS.md`, `CLAUDE.md`, or `.claude/skills/**` already exist, do not overwrite them; add project-specific guidance only through an explicit reviewed change.

See `docs/agent-contract.md` for the full AI-agent contract, including editable files, generated files, warning policy, and final response expectations.

Use `devimg agent init` to create safe project instructions when a project does not already have them:

```bash
devimg agent init --target codex
devimg agent init --target claude
devimg agent init --target both
devimg agent init --target both --stdout
```

The generator refuses to overwrite existing files unless `--force` is passed. The repo also ships a reusable Codex skill at `skills/devimg-image-pipeline/`; copy that folder into `${CODEX_HOME:-~/.codex}/skills/` to make the workflow available as `$devimg-image-pipeline`.

## GitHub Action

```yaml
jobs:
  images:
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v6
      - uses: cleissonom/devimg/action@v0.1.15
        with:
          mode: check
          export-output: lib/devimg.generated.ts
          export-format: typescript
          strip-prefix: public
          url-prefix: /
          review-output: .devimg/review.html
      - uses: actions/upload-artifact@v4
        with:
          name: devimg-review
          path: .devimg/review.html
          if-no-files-found: error
```

This repository's CI smoke test builds the CLI, runs the local composite Action with `uses: ./action`, and passes `binary-path: target/debug/devimg`. Public repositories can pin the release tag shown above; the Action downloads the matching GitHub Release archive and verifies its SHA-256 checksum before running.

When `export-output` is set, the Action runs `devimg manifest export --check` after `devimg check --no-report` and fails if the checked-in helper file is missing or stale. It does not rewrite the helper. Set `export-typescript-helpers: "true"` when the checked-in TypeScript file was generated with `--typescript-helpers`.

When `review-output` is set, the Action generates a static HTML visual review artifact after the main command succeeds. Upload that file with `actions/upload-artifact` to inspect generated variants from the workflow run. The Action does not upload artifacts, commit generated files, or post PR comments by itself. In `mode: check`, it uses `devimg check --no-report` so report validation stays read-only.

## Development

See `CONTRIBUTING.md` for the contributor workflow, `SECURITY.md` for private vulnerability reporting, and `CHANGELOG.md` for unreleased user-facing changes.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

Run local security checks before release or security-sensitive changes:

```bash
scripts/security-checks.sh
```

## Release

Public releases are published to crates.io and GitHub Releases. The current public install path is:

```bash
cargo install devimg
```

Create a version tag that matches the workspace version and push it after publishing crates:

```bash
git tag v0.1.15
git push origin v0.1.15
```

The release workflow builds Linux, macOS, and Windows archives, attaches SHA-256 checksums, and publishes a GitHub Release. See `docs/release.md` for install and release details.

## Current Limits

- Stable source image scope is PNG, JPEG, and WebP. AVIF is supported as an opt-in output format only.
- `quality` controls lossy JPEG, WebP, and AVIF output. PNG remains lossless and ignores `quality`.
- The Action does not commit generated files, upload artifacts automatically, or post PR comments.

## License

DevImg is licensed under the Apache License, Version 2.0. See `LICENSE`.
