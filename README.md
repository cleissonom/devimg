# Dev Image Pipeline

`devimg` is a Rust CLI and GitHub Action for deterministic web image variants. It scans configured image folders, generates responsive PNG/JPEG/WebP outputs, writes a JSON manifest and Markdown report, and lets CI fail when generated images are missing, stale, or over budget.

The MVP has no web UI and no remote storage. The CLI is the source of truth; the GitHub Action is a thin wrapper around `devimg check`.

## Quickstart

```bash
cargo run -p devimg-cli -- init --stdout > devimg.toml
cargo run -p devimg-cli -- optimize --config devimg.toml
cargo run -p devimg-cli -- check --config devimg.toml
```

Useful commands:

```bash
cargo run -p devimg-cli -- optimize --config examples/portfolio/devimg.toml --dry-run
cargo run -p devimg-cli -- report --manifest examples/portfolio/public/images/devimg-manifest.json
cargo run -p devimg-cli -- inspect fixtures/images/sample.png
```

## Config

Copy `devimg.example.toml` or run `devimg init`.

```toml
[project]
root = "."
manifest = "public/images/devimg-manifest.json"
report = "devimg-report.md"
overwrite = false
strip_metadata = true

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

[budgets]
max_total_bytes = "3mb"
max_file_bytes = "350kb"
fail_on_regression = true
```

Outputs are named as:

```text
{relative-source-dir}/{stem}.{preset}.{width}.{format}
```

## Safety

- `optimize --dry-run` plans work without writing files.
- Existing unmanaged outputs are not overwritten unless config `overwrite = true` or CLI `--allow-overwrite` is used.
- Re-encoding strips metadata by default. `strip_metadata = false` is parsed, but the MVP encoders do not preserve source metadata.
- `check` fails on missing outputs, stale manifests, modified outputs, outdated config hashes, and byte budget violations.

Exit codes are stable for CI:

- `0`: success or help output.
- `1`: runtime error outside config validation.
- `2`: usage or config error.
- `3`: `devimg check` failed.
- `4`: unsafe overwrite refused.

## GitHub Action

```yaml
jobs:
  images:
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v6
      - uses: cleissonom/devimg/action@v0.1.1
        with:
          config: devimg.toml
          mode: check
```

This repository's CI smoke test builds the CLI, runs the local composite Action with `uses: ./action`, and passes `binary-path: target/debug/devimg`. Consumer workflows should use the published Action path shown above.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

## Release

Create a version tag that matches the workspace version and push it:

```bash
git tag v0.1.1
git push origin v0.1.1
```

The release workflow builds Linux, macOS, and Windows archives, attaches SHA-256 checksums, and publishes a GitHub Release. See `docs/release.md` for install and release details.

## MVP Limits

- Stable format scope is PNG, JPEG, and WebP.
- WebP encoding uses the Rust `image` crate's MVP support; lossy WebP quality control can be improved later with a dedicated encoder.
- The Action does not commit generated files or post PR comments.
