# devimg

Rust CLI for deterministic frontend image pipelines.

`devimg` scans configured image sources, generates responsive variants, writes a JSON manifest and Markdown report, exports app-friendly helpers, creates static review artifacts, and fails CI when generated images are missing, stale, modified, or over budget.

Install from crates.io:

```bash
cargo install devimg
```

Then initialize a project:

```bash
devimg init --stdout > devimg.toml
devimg doctor --config devimg.toml
devimg optimize --config devimg.toml
devimg check --config devimg.toml
```

Repository: <https://github.com/cleissonom/devimg>
