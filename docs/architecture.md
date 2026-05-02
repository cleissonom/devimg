# Architecture

The pipeline is deterministic:

```text
config -> scan -> plan -> execute -> manifest/report
```

- `devimg-core` owns config parsing, source scanning, planning, transforms, manifest/report generation, and check semantics.
- `devimg-cli` owns command parsing, user output, and exit codes.
- `action/` owns GitHub-specific invocation and summary output.

Core modules:

- `scan`: source walking, glob matching, format detection, and image inspection.
- `plan`: variant planning, source-specific preset overrides, canonical output naming, dimensions, and operation hashing.
- `transform`: image resize, cover-crop positioning, encode, and safe writes.
- `budget`: file and total byte budget evaluation.
- `check`: manifest comparison and CI failure assembly.
- `pipeline`: public result types and the high-level optimize flow.
- `manifest`: manifest JSON read/write, totals, and source-to-variant export helpers for app consumption.
- `report`: Markdown report rendering.

`devimg check` rebuilds the current plan, reads the manifest, and fails when outputs are missing, modified, stale, generated with an older config hash, or over budget.

When `[project].content_hash_filenames = true`, `plan` keeps a canonical non-hash output path for operation identity, `transform` inserts the encoded output hash into the actual filename, and `check` matches manifest outputs by operation hash before validating the hashed file path.

`devimg manifest export` reads the generated manifest and groups variants by source path. The export layer can strip a project path prefix and add a URL prefix so web apps can consume content-hashed generated filenames without hard-coded lookup tables.

Exit codes:

- `0`: success or help output.
- `1`: runtime error outside config validation.
- `2`: usage or config error.
- `3`: `devimg check` failed.
- `4`: unsafe overwrite refused.
