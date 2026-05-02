# Portfolio Example

This example project is intentionally small but exercises the same workflow intended for portfolio, blog, and docs images.

```bash
cargo run -p devimg-cli -- optimize --config examples/portfolio/devimg.toml
cargo run -p devimg-cli -- check --config examples/portfolio/devimg.toml
```

Generated images are written under `public/images/generated`, with a manifest at `public/images/devimg-manifest.json` and a report at `devimg-report.md`.

