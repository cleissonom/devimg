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
      - uses: cleissonom/devimg/action@v0.1.4
        with:
          config: devimg.toml
          mode: check
```

## Inputs

- `config`: config path. Default: `devimg.toml`.
- `mode`: `check` or `optimize`. Default: `check`.
- `working-directory`: command working directory. Default: `.`.
- `fail-on-warning`: pass `--fail-on-warning` in check mode.
- `binary-path`: use a prebuilt local binary, useful for smoke tests.
- `version`: release version to download when no binary is found. Default: `v0.1.4`.
- `report-path`: configured report path appended to the step summary; this does not override `devimg.toml`.
- `manifest-path`: expected manifest path exposed as an output; this does not override `devimg.toml`.

## Outputs

- `status`
- `report-path`
- `manifest-path`

The MVP Action does not commit generated files and does not post PR comments.
