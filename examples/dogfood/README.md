# Dogfood Example

This example is a compact frontend-project fixture for DevImg's own dogfooding. It uses small deterministic images, but names them like common app assets:

- `social/open-graph.png` for SEO/social sharing images.
- `logos/devimg-mark.png` for logo-like assets that should keep the full composition visible.
- `screenshots/cli-output.png` for UI/screenshot-like assets where high lossy quality matters.

The config enables `content_hash_filenames = true` and includes both `cover` and `contain` presets. Generated variants, the manifest, the report, TypeScript helper export, and review artifact are reproducible and should not be edited by hand.

Run the local loop:

```bash
cargo run -p devimg-cli -- doctor --config examples/dogfood/devimg.toml
cargo run -p devimg-cli -- optimize --config examples/dogfood/devimg.toml
cargo run -p devimg-cli -- check --config examples/dogfood/devimg.toml
cargo run -p devimg-cli -- manifest export \
  --manifest examples/dogfood/public/images/devimg-manifest.json \
  --format typescript \
  --strip-prefix public \
  --url-prefix / \
  --typescript-helpers \
  --output examples/dogfood/lib/devimg.generated.ts
cargo run -p devimg-cli -- review \
  --manifest examples/dogfood/public/images/devimg-manifest.json \
  --output examples/dogfood/.devimg/review.html
```

Use this example when changing DevImg behavior that affects hashed output filenames, source-specific paths, contain resizing, crop anchors, helper exports, review artifacts, or Action smoke coverage.
