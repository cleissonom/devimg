# Architecture

The pipeline is deterministic:

```text
config -> scan -> plan -> execute -> manifest/report
config -> scan -> plan -> read-only check -> doctor diagnostics
```

- `devimg-core` owns config parsing, source scanning, planning, transforms, manifest/report generation, check semantics, and doctor diagnostics.
- `devimg-cli` owns command parsing, starter init profiles, agent instruction generation, user output, and exit codes.
- `action/` owns GitHub-specific invocation and summary output.

Core modules:

- `scan`: source walking, glob matching, format detection, and image inspection.
- `plan`: variant planning, source-specific preset overrides, canonical output naming, dimensions, and operation hashing.
- `quality`: deterministic advisory warnings for lossy quality, upscaling, risky cover crops, and output-size surprises.
- `transform`: image resize, cover-crop positioning, PNG/JPEG/WebP/AVIF encoding, and safe writes.
- `budget`: file and total byte budget evaluation.
- `check`: manifest comparison and CI failure assembly.
- `compare`: manifest-to-manifest diffing for generated variant review.
- `doctor`: read-only project diagnostics, JSON output, next-command hints, and optional manifest export drift checks.
- `pipeline`: public result types and the high-level optimize flow.
- `manifest`: manifest JSON read/write, totals, and source-to-variant export helpers for app consumption.
- `review`: manifest-to-static-HTML rendering for visual inspection, with escaped paths/text and local asset links.
- `report`: Markdown report rendering.

`devimg check` rebuilds the current plan, reads the manifest, and fails when outputs are missing, modified, stale, generated with an older config hash, or over budget.

Quality diagnostics are warnings, not automatic tuning. Planning emits config/source warnings such as low lossy quality, requested upscaling, explicit `allow_upscale = true`, and material `cover` crop loss. Optimize/check add manifest-based warnings such as generated outputs larger than their source files. Default `devimg check` remains backward compatible; `--fail-on-warning` lets CI treat these warnings as failures.

`devimg doctor` is a read-only diagnostic command. It validates source directories, scans source images, builds the current plan, reuses check semantics without writing the Markdown report, verifies manifest/report presence, and optionally verifies checked-in manifest exports. Human output ends with the next command to run; `--json` emits deterministic structured output for CI and AI coding agents.

When `[project].content_hash_filenames = true`, `plan` keeps a canonical non-hash output path for operation identity, `transform` inserts the encoded output hash into the actual filename, and `check` matches manifest outputs by operation hash before validating the hashed file path.

`devimg manifest export` reads the generated manifest and groups variants by source path. The export layer can strip a project path prefix and add a URL prefix so web apps can consume content-hashed generated filenames without hard-coded lookup tables. `devimg manifest export --check --output <file>` compares a checked-in export with the current rendered output and fails without rewriting when it is missing or stale.

`devimg compare --base <manifest> --head <manifest>` compares two manifest snapshots without reading image files or rewriting outputs. It matches variants by source path, preset, output dimensions, and format, then reports added, removed, changed, and unchanged outputs, total byte deltas, variant count deltas, and top byte contributors. `--json` emits the same deterministic compare model for CI and coding agents.

`devimg review --manifest <manifest> --output <file>` renders a static HTML artifact from one manifest. It groups source images and variants, shows local previews, dimensions, formats, byte sizes, hashes, largest sources, largest outputs, and manifest-only warnings. It does not load the config or enforce budgets; budget status is marked as not evaluated and remains owned by `devimg check`. `--stdout` writes the same HTML to standard output, and `--force` is required to replace an existing output file.

`devimg agent init` creates Codex and Claude Code instruction files for the DevImg workflow. It preflights every target path and refuses to overwrite existing files unless `--force` is passed.

Exit codes:

- `0`: success or help output.
- `1`: runtime error outside config validation.
- `2`: usage or config error.
- `3`: `devimg check` failed, `devimg doctor` found required work, or `devimg manifest export --check` found a missing or stale export.
- `4`: unsafe overwrite refused.
