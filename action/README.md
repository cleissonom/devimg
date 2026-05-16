# Dev Image Pipeline Action

Runs the `devimg` CLI in CI. The default mode is `check`, which fails when generated outputs are missing, stale, modified, or over budget. The Action can also verify checked-in manifest exports and create an optional static HTML review artifact for upload.

```yaml
jobs:
  images:
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v6
      - uses: cleissonom/devimg/action@v0.1.14
        with:
          config: devimg.toml
          mode: check
          export-output: lib/devimg.generated.ts
          export-format: typescript
          strip-prefix: public
          url-prefix: /
          review-output: .devimg/review.html
      - uses: actions/upload-artifact@v4
        with:
          name: devimg-review
          path: .devimg/review.html
          if-no-files-found: error
```

Pin the release tag from public repositories with `cleissonom/devimg/action@vX.Y.Z`. The Action downloads the matching GitHub Release archive and verifies its SHA-256 checksum before extracting the CLI. For local smoke tests inside this repository, build the CLI first and pass `binary-path: target/debug/devimg` with `uses: ./action`.

## Inputs

- `config`: config path. Default: `devimg.toml`.
- `mode`: `check` or `optimize`. Default: `check`.
- `working-directory`: command working directory. Default: `.`.
- `fail-on-warning`: pass `--fail-on-warning` in check mode. Acknowledged warnings remain visible but do not fail strict checks.
- `binary-path`: use a prebuilt local binary, useful for smoke tests.
- `version`: release version to download when no binary is found. Default: `v0.1.14`.
- `report-path`: configured report path appended to the step summary; this does not override `devimg.toml`.
- `manifest-path`: expected manifest path exposed as an output; this does not override `devimg.toml`.
- `export-output`: optional checked-in manifest export/helper file to verify in check mode.
- `export-manifest`: optional manifest path to export; defaults to `manifest-path` when omitted.
- `export-format`: `json` or `typescript`. Default: `json`.
- `export-typescript-helpers`: set to `true` when the checked-in TypeScript export includes `--typescript-helpers`.
- `strip-prefix`: optional project path prefix stripped from exported image URLs.
- `url-prefix`: optional URL prefix added to exported image URLs.
- `review-output`: optional static HTML review artifact path to generate after a successful run.
- `review-manifest`: optional manifest path used for the review artifact. Defaults to `manifest-path`.
- `review-force`: set to `true` to replace an existing `review-output` file. Default: `false`.

When `export-output` is set in check mode, the Action runs `devimg check --no-report` first and then verifies the checked-in export with `devimg manifest export --check`. It does not rewrite the export file. `strip-prefix`, `url-prefix`, and `export-typescript-helpers` must match the command used to generate the checked-in export.

When `review-output` is set, the Action runs `devimg review --manifest <path> --output <path>` after the main command succeeds. Upload that file with `actions/upload-artifact` to inspect variants from the workflow run. The Action does not upload artifacts by itself.

## Outputs

- `status`
- `report-path`
- `manifest-path`
- `review-output`

In check mode, the Action passes `devimg check --no-report`, so it validates without rewriting reports. It also does not commit generated files, rewrite manifest helper files, upload artifacts automatically, or post PR comments.
