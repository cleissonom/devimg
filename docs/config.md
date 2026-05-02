# Config Reference

`devimg.toml` is standard TOML parsed through serde. Unknown keys inside known sections are ignored for forward compatibility; unknown top-level sections are rejected.

## Project

- `root`: project root, resolved relative to the config file.
- `manifest`: JSON manifest path relative to `root`.
- `report`: Markdown report path relative to `root`.
- `overwrite`: defaults to `false`; when false, changed existing outputs are refused.
- `strip_metadata`: defaults to `true`; MVP re-encoding strips metadata.

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
- `quality`: `0..100`; currently applies to JPEG.
- `fit`: `cover`, `contain`, or `fill`
- `aspect_ratio`: optional, like `16:9` or `1200:630`
- `allow_upscale`: defaults to `false`

## Budgets

`max_total_bytes` and `max_file_bytes` accept byte strings such as `350kb`, `3mb`, or raw byte counts.
