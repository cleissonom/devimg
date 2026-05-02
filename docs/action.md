# GitHub Action

The Action in `action/` is a composite wrapper around the `devimg` CLI.

```yaml
name: Image Pipeline

on:
  pull_request:
    paths:
      - "assets/images/**"
      - "devimg.toml"

jobs:
  images:
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v4
      - uses: cleissonom/devimg/action@v1
        with:
          config: devimg.toml
          mode: check
```

Use `binary-path` when testing the Action from a local checkout after building `target/debug/devimg`.

`report-path` and `manifest-path` describe files for summary/output metadata. Configure the actual report and manifest paths in `devimg.toml`.

The MVP does not post PR comments and does not commit generated image files.
