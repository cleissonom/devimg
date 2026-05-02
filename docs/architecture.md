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
- `manifest`: manifest JSON read/write and totals.
- `report`: Markdown report rendering.

`devimg check` rebuilds the current plan, reads the manifest, and fails when outputs are missing, modified, stale, generated with an older config hash, or over budget.

When `[project].content_hash_filenames = true`, `plan` keeps a canonical non-hash output path for operation identity, `transform` inserts the encoded output hash into the actual filename, and `check` matches manifest outputs by operation hash before validating the hashed file path.

Exit codes:

- `0`: success or help output.
- `1`: runtime error outside config validation.
- `2`: usage or config error.
- `3`: `devimg check` failed.
- `4`: unsafe overwrite refused.
