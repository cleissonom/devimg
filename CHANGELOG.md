# Changelog

DevImg follows a lightweight changelog during pre-1.0 development. Add unreleased user-facing changes here before cutting a release.

## Unreleased

No unreleased changes.

## 0.1.10

- Added static visual review artifacts through `devimg review`.
- Added advisory quality diagnostics in `optimize`, `check`, `doctor`, Markdown reports, and visual review output.
- Added framework-aware `doctor` diagnostics for Next.js, Astro, and Vite projects.
- Added open-source contributor, security, issue, and PR guidance.
- Added an AI-agent task contract for DevImg-managed repositories.
- Added opt-in TypeScript manifest lookup helpers for generated app exports.
- Added incremental `optimize` behavior that skips manifest-current outputs and reports generated/skipped/stale variant counts.
- Added Action support for optional static review artifacts and read-only check-mode report handling through `devimg check --no-report`.
- Added a broader `examples/dogfood` fixture that exercises content-hash filenames, contain resizing, helper export, visual review, and CI smoke coverage.

## 0.1.9

- Added opt-in AVIF output support.
- Improved image quality controls and per-format quality behavior.
- Updated private-release dogfooding for `cleisson.com`.
