# Dev Image Pipeline

`devimg` is a Rust CLI and GitHub Action for deterministic web image variants. It scans configured image folders, generates responsive PNG/JPEG/WebP outputs, writes a JSON manifest and Markdown report, and lets CI fail when generated images are missing, stale, or over budget.

The MVP has no web UI and no remote storage. The CLI is the source of truth; the GitHub Action wraps `devimg check` or `devimg optimize` and can optionally verify checked-in manifest exports.

## Quickstart

```bash
cargo run -p devimg-cli -- init --stdout > devimg.toml
# Or choose framework-friendly starter paths:
cargo run -p devimg-cli -- init --profile next --stdout > devimg.toml
cargo run -p devimg-cli -- doctor --config devimg.toml
cargo run -p devimg-cli -- optimize --config devimg.toml
cargo run -p devimg-cli -- check --config devimg.toml
cargo run -p devimg-cli -- doctor --config devimg.toml
```

Useful commands:

```bash
cargo run -p devimg-cli -- doctor --config examples/portfolio/devimg.toml
cargo run -p devimg-cli -- doctor --config examples/portfolio/devimg.toml --json
cargo run -p devimg-cli -- agent init --target both --stdout
cargo run -p devimg-cli -- optimize --config examples/portfolio/devimg.toml --dry-run
cargo run -p devimg-cli -- report --manifest examples/portfolio/public/images/devimg-manifest.json
cargo run -p devimg-cli -- manifest export --manifest examples/portfolio/public/images/devimg-manifest.json
cargo run -p devimg-cli -- compare --base old-devimg-manifest.json --head public/images/devimg-manifest.json
cargo run -p devimg-cli -- inspect fixtures/images/sample.png
```

Recommended local loop:

```bash
devimg doctor --config devimg.toml
devimg optimize --config devimg.toml --allow-overwrite
devimg manifest export --manifest public/images/devimg-manifest.json --strip-prefix public --url-prefix / --format typescript --output lib/devimg.generated.ts
devimg check --config devimg.toml
devimg doctor --config devimg.toml --export-output lib/devimg.generated.ts --export-format typescript --strip-prefix public --url-prefix /
```

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
formats = ["webp", "jpeg"]
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

The exported variants include `src`, `output_path`, `preset`, `fit`, `width`, `height`, `format`, `bytes`, and `hash`. `--strip-prefix public --url-prefix /` converts an output path such as `public/images/generated/card.project-card.640.jpeg` into `/images/generated/card.project-card.640.jpeg`.

`devimg doctor` can also verify a checked-in export without rewriting it:

```bash
devimg doctor \
  --config devimg.toml \
  --export-output lib/devimg.generated.ts \
  --export-format typescript \
  --strip-prefix public \
  --url-prefix /
```

## Manifest Compare

Use `compare` when reviewing image PRs or checking why a branch changed generated assets. Keep a copy of the old manifest, regenerate images on the branch, then compare the two manifests:

```bash
cp public/images/devimg-manifest.json /tmp/devimg-base-manifest.json
devimg optimize --config devimg.toml --allow-overwrite
devimg compare --base /tmp/devimg-base-manifest.json --head public/images/devimg-manifest.json
```

The human report shows variant count delta, output byte delta, added outputs, removed outputs, changed outputs, unchanged outputs, and the largest head-manifest byte contributors. Use JSON output for CI or AI agents:

```bash
devimg compare \
  --base /tmp/devimg-base-manifest.json \
  --head public/images/devimg-manifest.json \
  --json
```

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
- Re-encoding strips metadata by default. `strip_metadata = false` is parsed, but the MVP encoders do not preserve source metadata.
- `check` fails on missing outputs, stale manifests, modified outputs, outdated config hashes, and byte budget violations.

Exit codes are stable for CI:

- `0`: success or help output.
- `1`: runtime error outside config validation.
- `2`: usage or config error.
- `3`: `devimg check` failed, `devimg doctor` found required work, or `devimg manifest export --check` found a missing or stale export.
- `4`: unsafe overwrite refused.

## AI Agent Workflow

Codex, Claude Code, and similar tools should run `devimg doctor --config devimg.toml` before editing image sources, config, manifests, generated variants, or app helper files. After changes, run the local loop above and commit the generated variants, manifest, report, and checked-in helper files together.

Do not edit generated files by hand. If agent instruction files such as `AGENTS.md`, `CLAUDE.md`, or `.claude/skills/**` already exist, do not overwrite them; add project-specific guidance only through an explicit reviewed change.

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
      - uses: cleissonom/devimg/action@v0.1.7
        with:
          config: devimg.toml
          mode: check
          export-output: lib/devimg.generated.ts
          export-format: typescript
          strip-prefix: public
          url-prefix: /
```

This repository's CI smoke test builds the CLI, runs the local composite Action with `uses: ./action`, and passes `binary-path: target/debug/devimg`. Consumer workflows should use the published Action path shown above.

When `export-output` is set, the Action runs `devimg manifest export --check` after `devimg check` and fails if the checked-in helper file is missing or stale. It does not rewrite the helper.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

## Release

Create a version tag that matches the workspace version and push it:

```bash
git tag v0.1.7
git push origin v0.1.7
```

The release workflow builds Linux, macOS, and Windows archives, attaches SHA-256 checksums, and publishes a GitHub Release. See `docs/release.md` for install and release details.

## MVP Limits

- Stable format scope is PNG, JPEG, and WebP.
- `quality` controls lossy JPEG and WebP output. PNG remains lossless.
- The Action does not commit generated files or post PR comments.
