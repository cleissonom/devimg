# Config Reference

`devimg.toml` is standard TOML parsed through serde. Unknown keys inside known sections are ignored for forward compatibility; unknown top-level sections are rejected.

## Project

- `root`: project root, resolved relative to the config file.
- `manifest`: JSON manifest path relative to `root`.
- `report`: Markdown report path relative to `root`.
- `overwrite`: defaults to `false`; when false, changed existing outputs are refused.
- `strip_metadata`: defaults to `true`; MVP re-encoding strips metadata.
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
- `formats`: `png`, `jpeg`/`jpg`, `webp`
- `quality`: `0..100`; applies to lossy JPEG and WebP output. PNG output remains lossless.
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

## Budgets

`max_total_bytes` and `max_file_bytes` accept byte strings such as `350kb`, `3mb`, or raw byte counts.
