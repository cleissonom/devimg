# Config Reference

`devimg.toml` is standard TOML parsed through serde. Unknown keys inside known sections are ignored for forward compatibility; unknown top-level sections are rejected.

## Init Profiles

`devimg init` writes the generic starter config. `devimg init --profile next`, `devimg init --profile astro`, and `devimg init --profile vite` write the same config shape with framework-friendly source paths.

Profiles only choose starter paths and source names. They do not add new config syntax or framework-specific runtime behavior.

- `next`: `public/images/source` -> `public/images/generated`
- `astro`: `src/assets/images` -> `public/images/generated`
- `vite`: `src/assets/images` -> `public/images/generated`

## Project

- `root`: project root, resolved relative to the config file.
- `manifest`: JSON manifest path relative to `root`.
- `report`: Markdown report path relative to `root`.
- `overwrite`: defaults to `false`; when false, changed existing outputs are refused.
- `strip_metadata`: defaults to `true`; current re-encoding strips metadata.
- `content_hash_filenames`: defaults to `false`; when true, generated output filenames include a short hash fragment derived from the encoded output bytes. Aliases: `hash_filenames`, `hashed_filenames`.

Default filenames are deterministic:

```text
card.project-card.640.webp
```

Content-hash filenames are opt-in:

```text
card.project-card.640.abcdef123456.webp
```

Use content-hash filenames before applying immutable CDN cache headers to generated assets.

## Sources

Each `[[sources]]` entry has:

- `name`
- `input`
- `output`
- `include`
- `exclude`

Glob matching uses `globset`, is case-insensitive, and covers patterns such as `**/*.png`, `*.jpg`, and `**/generated/**`. A leading `**/` also matches files at the source root, so `**/*.png` matches both `card.png` and `nested/card.png`.

## Presets

Each `[[preset]]` entry has:

- `name`
- `widths`
- `formats`: `png`, `jpeg`/`jpg`, `webp`, `avif`
- `quality`: `0..100`; applies to lossy JPEG, WebP, and AVIF output. PNG output remains lossless and ignores `quality`.
- `fit`: `cover`, `contain`, or `fill`
- `aspect_ratio`: optional, like `16:9` or `1200:630`
- `crop`: optional cover-crop position; defaults to `center`
- `allow_upscale`: defaults to `false`

`crop` only affects `fit = "cover"`. String anchors are:

- `center`
- `top`, `bottom`, `left`, `right`
- `top-left`, `top-right`, `bottom-left`, `bottom-right`

For more precise composition, use a normalized focal point where `0.0` is the left/top edge and `1.0` is the right/bottom edge:

```toml
crop = "top"
crop = { x = 0.5, y = 0.0 }
```

## Overrides

Use `[[overrides]]` when most sources should use the shared presets but a few source files need different transform behavior.

Each override matches source-relative paths with the same glob behavior as source include/exclude patterns:

- `include`: optional list of matching paths or globs; empty means all sources
- `exclude`: optional list of paths or globs to skip
- `presets`: optional list of preset names; empty means all presets
- `quality`: optional replacement quality
- `fit`: optional replacement fit
- `crop`: optional replacement cover-crop position
- `allow_upscale`: optional replacement upscale setting

Overrides are applied in config order, and later matching overrides win for the fields they set.

```toml
[[overrides]]
include = ["cli_tools.png"]
fit = "contain"

[[overrides]]
include = ["avatars/**"]
presets = ["avatar"]
crop = "top"
```

Overrides do not change preset `widths`, `formats`, or `aspect_ratio`; keep those in named presets so output shapes remain explicit.

## Format Guidance

- PNG: lossless output for graphics and transparency-sensitive assets; `quality` is intentionally ignored.
- JPEG: lossy output for broad compatibility and opaque photos; `quality` directly affects encoded size.
- WebP: lossy output through libwebp; `quality` directly affects encoded size and is usually a good default for modern frontend projects.
- AVIF: opt-in lossy output through `ravif`; useful for aggressive byte savings on supported browsers, but generation is slower than WebP. DevImg does not scan AVIF source files yet.

## Quality Diagnostics

DevImg keeps quality decisions explicit in config, but it warns when common settings are likely to surprise developers:

- `quality:` warnings are advisory by default.
- `devimg check --fail-on-warning` turns warnings into exit code `3`.
- `devimg doctor --json` exposes stable warning codes such as `quality:cover-crop`, `quality:low-lossy-quality`, and `quality:output-larger-than-source`.
- Markdown reports include warnings from `optimize` and `check`.
- `devimg review` includes manifest-only quality warnings such as upscaled outputs and outputs larger than their source files.

Current diagnostics:

- JPEG/WebP quality below `70`, or below `82` for screenshot-, banner-, card-, hero-, logo-, diagram-, UI-, or text-like assets.
- AVIF quality below `45`, or below `60` for those detail-sensitive assets.
- Requested variants that would require upscaling; DevImg skips them unless `allow_upscale = true`.
- `allow_upscale = true` variants that enlarge source dimensions and may look soft.
- `fit = "cover"` variants that crop about 15% or more of the resized image.
- Generated output files that are larger than their source file.

Larger generated files are not always wrong. They can happen when the source is already aggressively optimized, when quality is intentionally high, when converting formats, or when graphics/transparency compress better in the source format. Compare visually before lowering quality.

## Warning Acknowledgements

Use warning acknowledgements only after reviewing the image and deciding the warning is intentional. Acknowledgements do not hide hard failures such as missing outputs, stale manifests, modified files, outdated config hashes, or budget violations.

```toml
[[warnings.acknowledge]]
code = "quality:cover-crop"
source = "assets/images/accesstrace.png"
preset = "project-card"
reason = "Intentional card crop after visual review."
```

Acknowledgements are matched by warning `code` plus optional `source`, `output`, `preset`, and `width`. Source-based quality warnings such as `quality:cover-crop`, `quality:low-lossy-quality`, and upscaling diagnostics require both `source` and `preset` so they stay scoped to the reviewed image and transform. Manifest-output warnings such as `quality:output-larger-than-source` require `output`.

Acknowledged warnings are still reported under `Acknowledged Warnings` and in `doctor --json` as `acknowledged_warnings`. `devimg check --fail-on-warning` passes when only acknowledged warnings remain and fails when a new unacknowledged warning appears.

After adding or changing acknowledgements, run `devimg optimize` once so the manifest and report carry the current config hash. DevImg can reuse existing outputs when only acknowledgement metadata changed, so this refresh should update manifest/report metadata without re-encoding unchanged images.

## Framework Diagnostics

`devimg doctor` detects Next.js, Astro, and Vite from common config files and `package.json` dependency sections.

Framework warnings are advisory:

- `framework_multiple_detected`: more than one supported framework was detected.
- `framework_next_image_double_optimization`: Next.js was detected with generated files under `public/`; verify `next/image` or hosting image optimization is not reprocessing generated variants unexpectedly.
- `framework_cache_without_hash`: a framework project outputs generated assets under `public/` while `content_hash_filenames = false`.
- `framework_manifest_export_missing`: content-hash filenames are enabled in a framework project, but no checked manifest helper was configured or discovered.
- `framework_manifest_helper_unchecked`: a common helper file such as `lib/devimg.generated.ts`, `lib/devimg-*.generated.ts`, or `lib/devimg.ts` was discovered, but `doctor` was not given a matching `--export-output` drift check.

`doctor --json` also includes `manifest_helpers` when common helper files are discovered. These warnings do not add framework runtime coupling. They are hints to review config, app image consumption, cache headers, and manifest export checks. For framework projects, `doctor` explains the intended consumption modes: plain `img`/`picture` URLs from exports, framework `Image` with `unoptimized` when DevImg owns sizing and quality, or intentional framework optimization layered on generated source variants.

## Budgets

`max_total_bytes` and `max_file_bytes` accept byte strings such as `350kb`, `3mb`, or raw byte counts.
