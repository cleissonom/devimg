# Examples

These examples are small, deterministic fixtures used for local development, CI smoke tests, and documentation. Each one has a specific adoption lesson:

- `portfolio`: learn the smallest useful setup: one source folder, stable output paths, common presets, manifest/report generation, and simple CI checks.
- `dogfood`: learn a frontend-app setup: content-hash filenames, cover and contain presets, checked-in helper export, static review artifact, and framework-facing diagnostics.

Generated variants, manifests, reports, helper exports, and review artifacts are reproducible outputs. Regenerate them through `devimg`; do not hand-edit them.
