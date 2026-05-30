# Contributing to DevImg

DevImg is a Rust CLI, core library, and GitHub Action for deterministic frontend image pipelines.

## Local Setup

Use Rust `1.88` or newer.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --locked -p devimg
```

## Development Loop

For pipeline changes, verify the example project:

```bash
cargo run -p devimg -- optimize --config examples/portfolio/devimg.toml --dry-run
cargo run -p devimg -- optimize --config examples/portfolio/devimg.toml --allow-overwrite
cargo run -p devimg -- check --config examples/portfolio/devimg.toml
cargo run -p devimg -- review --manifest examples/portfolio/public/images/devimg-manifest.json --output examples/portfolio/.devimg/review.html --force
cargo run -p devimg -- optimize --config examples/dogfood/devimg.toml --dry-run
cargo run -p devimg -- optimize --config examples/dogfood/devimg.toml --allow-overwrite
cargo run -p devimg -- check --config examples/dogfood/devimg.toml
cargo run -p devimg -- manifest export --manifest examples/dogfood/public/images/devimg-manifest.json --format typescript --strip-prefix public --url-prefix / --typescript-helpers --output /tmp/devimg-dogfood.generated.ts
cargo run -p devimg -- review --manifest examples/dogfood/public/images/devimg-manifest.json --output examples/dogfood/.devimg/review.html --force
cargo run -p devimg -- draft --config examples/dogfood/devimg.toml --draft-type project-page-copy --dry-run --review-html examples/dogfood/.devimg/review.html --output /tmp/devimg-dogfood-project-page-copy.md --force
```

Before finishing a change, run:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

Run the local security stack when dependencies, release logic, Action logic, or repository metadata changes:

```bash
scripts/security-checks.sh
```

The script expects `gitleaks`, `cargo-audit`, `cargo-deny`, and `zizmor` on `PATH` and prints install hints when a tool is missing.

## Pull Requests

- Keep changes scoped to one behavior or documentation goal.
- Add or update tests for user-visible behavior.
- Keep generated image variants, manifests, reports, and manifest helper files in sync when examples change.
- Do not hand-edit generated files.
- Do not add hosted services, remote storage, automatic PR commits, or new distribution channels without prior design discussion.

## Release Notes

Update `CHANGELOG.md` for user-facing CLI, config, Action, manifest, release, or docs changes.
