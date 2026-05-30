# Changelog

DevImg follows a lightweight changelog during pre-1.0 development. Add unreleased user-facing changes here before cutting a release.

## Unreleased

## 0.2.4

- Added `devimg review --ai` for opt-in OpenAI AI review of manifest metadata and, with explicit `--include-images`, generated image bytes.
- Added AI review JSON and Markdown artifacts, dry-run previews, overwrite protection, image-detail and max-image controls, and advisory observation categories.
- Added OpenAI Responses API integration with structured output parsing, `.env` credential loading limited to explicit AI commands, and no key/value persistence.
- Kept Anthropic real AI review deferred while preserving existing provider consent previews.
- Updated CI smoke coverage, docs, release references, and dogfood guidance for `v0.2.4`.

## 0.2.3

- Added `devimg ai consent` for deterministic OpenAI/Anthropic consent previews.
- Added provider identifiers, provider-specific API key validation, metadata-only default previews, `--include-images` preview metadata, and dry-run output that does not require keys.
- Added mocked provider-client boundaries for future AI commands without adding SDK, HTTP, or network behavior.
- Documented no-key-leak handling, no image bytes by default, and deferred real provider calls for later `0.2.x` AI commands.
- Exposed the GitHub Action `binary-path` output and added AI consent dry-run dogfood coverage.

## 0.2.2

- Added `devimg suggest --check` for read-only local and CI suggestion gates.
- Added `--fail-on-severity advisory|warning|error`, stable terminal summaries, and explicit blocking counts for suggestion checks.
- Added `affected_path` and `next_command` fields to deterministic suggestion items.
- Added suggestion commands to human doctor output and check/optimize Markdown reports when active or acknowledged warnings exist.
- Updated dogfood docs, CI smoke coverage, and public release references for `v0.2.2`.

## 0.2.1

- Added `devimg suggest --metadata-only` for deterministic local suggestion files generated from existing DevImg diagnostics.
- Added stable suggestion JSON output, optional Markdown summaries, overwrite protection through `--force`, and structured `suggested_config` guidance for humans and coding agents.
- Documented deterministic suggestions, local-only behavior, and the deferred status of provider-backed AI suggestions.
- Updated public release, Action, and distribution references for `v0.2.1`.

## 0.2.0

- Bumped the workspace to `0.2.0` and added `devimg agent task` for local-only Markdown task contracts for Codex, Claude Code, and generic agents.
- Added `agent task --agent codex|claude-code|generic`, stdout output by default, `--output <path>`, overwrite protection through `--force`, and protection for existing agent instruction paths.
- Documented the `agent task` workflow, generated task context, safe output handling, and the deferred status of provider-backed AI features.

## 0.1.16

- Migrated the `image` dependency to `0.25.10`, raised the source-build MSRV to Rust 1.88, and replaced `ImageOutputFormat` usage with current JPEG/PNG encoder APIs.
- Updated `serde_json` to `1.0.150`.
- Updated the pinned `github/codeql-action` workflow SHAs to `4.36.0`.

## 0.1.15

- Treated `devimg.toml` as the documented default config path in CLI hints, doctor next-step output, generated agent instructions, and public docs, while preserving `--config <path>` for custom config files.

## 0.1.14

- Prepared public distribution metadata for crates.io.
- Renamed the CLI package from `devimg-cli` to `devimg` before first crates.io publish, while keeping the binary name `devimg`.
- Added crates.io package README files for `devimg` and `devimg-core`.
- Added release archive checksum verification to the GitHub Action download path.
- Updated public installation, Action, and release documentation for `cargo install devimg` and `v0.1.14`.

## 0.1.13

- Improved `devimg doctor` framework-consumption diagnostics with common helper discovery, `manifest_helpers` JSON output, and clearer frontend consumption-mode guidance.
- Added the `framework_manifest_helper_unchecked` advisory warning when content-hash filenames are used with discovered helper files that are not verified by `--export-output`.
- Polished README/example guidance for the first public distribution story.

## 0.1.12

- Changed operation hashing to be transform-focused, so warning acknowledgements and other non-transform config metadata do not make every planned output look changed.
- Improved incremental optimize reuse when a config hash changes but existing outputs still match the current transform plan.
- Added manifest compare reporting for metadata-only output changes, separate from byte/path/content changes.
- Clarified Next.js/Vercel framework diagnostics around static generated assets, `next/image`, and CDN caching.

## 0.1.11

- Added scoped `[[warnings.acknowledge]]` support for reviewed advisory warnings.
- Added stable warning codes such as `quality:cover-crop` and `quality:low-lossy-quality`.
- Updated `check --fail-on-warning` so acknowledged warnings remain visible but do not fail strict checks.
- Added `doctor --json` `acknowledged_warnings` output.

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
- Added dogfood validation for frontend project image quality.
