# DevImg v0.1.13 Share Package

Use this file when preparing public posts, README screenshots, or release notes for the Framework Consumption + Public Story Pack. Keep the wording grounded in what DevImg does today.

## Positioning

Repo description:

> Deterministic Rust image pipeline for frontend repositories: generate responsive variants, export app helpers, check freshness and budgets in CI, and review visual output as artifacts.

Problem statement:

Frontend image variants often become stale, oversized, manually copied, or disconnected from app code. That makes PR review and CI enforcement harder than it should be.

Solution statement:

DevImg keeps image generation inside the repository: `devimg optimize` writes variants, a manifest, and a report; `devimg manifest export` creates app-friendly helper files; `devimg check` fails CI when outputs drift; `devimg review` creates a static HTML artifact for visual inspection.

## LinkedIn Draft

I have been building DevImg, a Rust CLI for frontend image pipelines.

The goal is simple: make generated web images deterministic, reviewable, and CI-enforced. Instead of manually copying optimized files or relying on hidden build steps, DevImg keeps the source images, generated variants, manifest, report, and optional app helper export together in the repo.

The current workflow is:

- configure image sources and presets in `devimg.toml`;
- run `devimg optimize` to generate responsive variants;
- run `devimg manifest export` so frontend code can consume content-hash filenames safely;
- run `devimg check` in CI to catch missing, stale, oversized, or out-of-budget outputs;
- upload a static `devimg review` artifact when maintainers need to inspect visual changes.

I am dogfooding it on `cleisson.com` for project card and banner images. The next release focuses on clearer framework consumption diagnostics and a better public story around how DevImg fits next to framework image components, CDNs, and lower-level image engines.

It is not a SaaS, hosted image service, or automatic PR bot. The core is a local-first CLI and GitHub Action wrapper for teams that want generated image files to be explicit and reviewable.

## Technical Note Outline

1. Generated frontend images are easy to drift.
2. Low-level image engines and framework image components solve different layers.
3. DevImg owns the repo workflow: config, variants, manifest, report, helper export, CI check, and review artifact.
4. What changes in a PR: source/config, generated variants, manifest, report, optional helper export, optional review artifact.
5. How framework apps consume outputs:
   - plain `img` or `picture` URLs from exported variants;
   - framework `Image` with `unoptimized` when DevImg owns sizing and quality;
   - intentional framework optimization layered on DevImg-generated source variants.
6. Dogfood example: `cleisson.com` project images.
7. Limits: no hosted service, no automatic commits, no AI automation in `v0.1.13`, and no broad distribution promise before that is intentionally chosen.
8. Future: optional AI-assisted suggestions remain advisory and privacy-explicit in the `v0.2` plan.

## Screenshot And Review Artifact Capture

Regenerate the dogfood example:

```bash
cargo run -p devimg-cli -- optimize --config examples/dogfood/devimg.toml
cargo run -p devimg-cli -- manifest export \
  --manifest examples/dogfood/public/images/devimg-manifest.json \
  --format typescript \
  --strip-prefix public \
  --url-prefix / \
  --typescript-helpers \
  --output examples/dogfood/lib/devimg.generated.ts
cargo run -p devimg-cli -- review \
  --manifest examples/dogfood/public/images/devimg-manifest.json \
  --output examples/dogfood/.devimg/review.html \
  --force
```

Open `examples/dogfood/.devimg/review.html` in a browser and capture screenshots from that artifact. Treat screenshots as derived assets: refresh the review artifact first, then recapture.

## Repo Metadata Suggestions

Description options:

- `Deterministic Rust image pipeline for frontend repositories.`
- `Generate responsive web images, manifests, reports, and CI checks from devimg.toml.`
- `Local-first image variant pipeline with manifest exports and GitHub Action checks.`

Topics:

`rust`, `cli`, `images`, `image-optimization`, `responsive-images`, `web-performance`, `github-actions`, `ci`, `static-site`, `nextjs`, `astro`, `vite`

Release blurb:

`v0.1.13` improves DevImg's public story, example guidance, and framework-consumption diagnostics. `devimg doctor` now discovers common helper files, reports manifest helper paths in JSON/human output, warns when content-hash filenames are not tied to a checked helper export, and explains the supported frontend consumption modes.
