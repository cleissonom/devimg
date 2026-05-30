# devimg

Rust CLI for deterministic frontend image pipelines.

`devimg` scans configured image sources, generates responsive variants, writes a JSON manifest and Markdown report, exports app-friendly helpers, creates static review artifacts, and fails CI when generated images are missing, stale, modified, or over budget.

Install from crates.io:

```bash
cargo install devimg
```

Requires Rust 1.88 or newer when installing from source.

Then initialize a project:

```bash
devimg init --stdout > devimg.toml
devimg doctor
devimg optimize
devimg check
```

`devimg.toml` is the default config path. Pass `--config <path>` only for custom filenames or locations.

Repository: <https://github.com/cleissonom/devimg>
