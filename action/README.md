# Dev Image Pipeline Action

Runs the `devimg` CLI in CI. The default mode is `check`, which fails when generated outputs are missing, stale, modified, or over budget.

```yaml
jobs:
  images:
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v6
      - uses: cleissonom/devimg/action@v0.1.8
        with:
          config: devimg.toml
          mode: check
          export-output: lib/devimg.generated.ts
          export-format: typescript
          strip-prefix: public
          url-prefix: /
```

## Inputs

- `config`: config path. Default: `devimg.toml`.
- `mode`: `check` or `optimize`. Default: `check`.
- `working-directory`: command working directory. Default: `.`.
- `fail-on-warning`: pass `--fail-on-warning` in check mode.
- `binary-path`: use a prebuilt local binary, useful for smoke tests.
- `version`: release version to download when no binary is found. Default: `v0.1.8`.
- `report-path`: configured report path appended to the step summary; this does not override `devimg.toml`.
- `manifest-path`: expected manifest path exposed as an output; this does not override `devimg.toml`.
- `export-output`: optional checked-in manifest export/helper file to verify in check mode.
- `export-manifest`: optional manifest path to export; defaults to `manifest-path` when omitted.
- `export-format`: `json` or `typescript`. Default: `json`.
- `strip-prefix`: optional project path prefix stripped from exported image URLs.
- `url-prefix`: optional URL prefix added to exported image URLs.

When `export-output` is set in check mode, the Action runs `devimg check` first and then verifies the checked-in export with `devimg manifest export --check`. It does not rewrite the export file. `strip-prefix` and `url-prefix` must match the command used to generate the checked-in export.

## Outputs

- `status`
- `report-path`
- `manifest-path`

The MVP Action does not commit generated files, rewrite manifest helper files, or post PR comments.
