# Dogfood Example

This example is a compact frontend-project fixture for DevImg's own dogfooding. It uses small deterministic images, but names them like common app assets:

- `social/open-graph.png` for SEO/social sharing images.
- `logos/devimg-mark.png` for logo-like assets that should keep the full composition visible.
- `screenshots/cli-output.png` for UI/screenshot-like assets where high lossy quality matters.

The config enables `content_hash_filenames = true` and includes both `cover` and `contain` presets. Generated variants, the manifest, the report, TypeScript helper export, and review artifact are reproducible and should not be edited by hand.

This is the main example for public demos:

- Content-hash filenames show how generated URLs can be used with immutable caching.
- `cover` presets show card/social cropping.
- `contain` presets show logo or diagram resizing without cropping.
- The TypeScript helper export shows how apps can avoid hard-coded generated filenames.
- The static review artifact shows how generated variants can be reviewed locally or uploaded from CI with `actions/upload-artifact`.

Run the local loop:

```bash
cargo run -p devimg -- doctor --config examples/dogfood/devimg.toml
cargo run -p devimg -- optimize --config examples/dogfood/devimg.toml
cargo run -p devimg -- check --config examples/dogfood/devimg.toml
cargo run -p devimg -- suggest --metadata-only --check --fail-on-severity warning --config examples/dogfood/devimg.toml
cargo run -p devimg -- suggest --metadata-only \
  --config examples/dogfood/devimg.toml \
  --output /tmp/devimg-dogfood-suggestions.json \
  --markdown /tmp/devimg-dogfood-suggestions.md \
  --force
cargo run -p devimg -- ai consent \
  --config examples/dogfood/devimg.toml \
  --ai-provider openai \
  --model openai-dry-run-model \
  --dry-run \
  --output /tmp/devimg-dogfood-openai-consent.json \
  --force
cargo run -p devimg -- ai consent \
  --config examples/dogfood/devimg.toml \
  --ai-provider anthropic \
  --model anthropic-dry-run-model \
  --dry-run \
  --output /tmp/devimg-dogfood-anthropic-consent.json \
  --force
cargo run -p devimg -- manifest export \
  --manifest examples/dogfood/public/images/devimg-manifest.json \
  --format typescript \
  --strip-prefix public \
  --url-prefix / \
  --typescript-helpers \
  --output examples/dogfood/lib/devimg.generated.ts
cargo run -p devimg -- review \
  --manifest examples/dogfood/public/images/devimg-manifest.json \
  --output examples/dogfood/.devimg/review.html
```

The review artifact is derived output. Use the command above before taking screenshots for docs, release notes, or demos. If a screenshot is committed later, refresh it from the regenerated review artifact rather than editing it by hand.

The suggestion gate is read-only and should stay in CI. The explicit suggestion and AI consent preview artifact commands write to `/tmp` for local review; do not commit those files unless a later task intentionally promotes them. Consent dry-runs require no API keys, include no image bytes by default, and perform no OpenAI or Anthropic calls in `0.2.3`.

Use this example when changing DevImg behavior that affects hashed output filenames, source-specific paths, contain resizing, crop anchors, helper exports, review artifacts, or Action smoke coverage.
