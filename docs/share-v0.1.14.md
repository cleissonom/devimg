# DevImg v0.1.14 Share Package

Use this file when preparing the first public distribution announcement. Keep the wording grounded in what DevImg does today.

## Positioning

Repo description:

> Deterministic Rust image pipeline for frontend repositories.

Longer description:

> Generate responsive web image variants, manifest exports, Markdown reports, CI budget checks, and static visual review artifacts from `devimg.toml`.

## Launch Draft

I have been building DevImg, a Rust CLI and GitHub Action for frontend image pipelines.

The problem: generated web images often drift from their source files, get committed without clear review context, or depend on hidden build steps. DevImg keeps that workflow explicit:

- `devimg optimize` generates responsive variants;
- `devimg manifest export` creates app-friendly helper files for content-hash filenames;
- `devimg check` fails CI when outputs are missing, stale, modified, or over budget;
- `devimg review` creates a static HTML artifact for visual inspection.

It is local-first and repository-first. No hosted service, remote storage, or automatic PR bot. The goal is to make image changes easy to reproduce, review, and enforce in frontend projects.

I am dogfooding it on `cleisson.com`, including project-card and banner images served through the normal frontend/Vercel/CDN flow.

Install:

```bash
cargo install devimg
```

GitHub Action:

```yaml
- uses: cleissonom/devimg/action@v0.1.14
  with:
    config: devimg.toml
    mode: check
```

I am looking for early users with image-heavy frontend repositories, especially Next.js, Astro, Vite, and static-site projects.

## Screenshot Ideas

- `devimg review` static HTML artifact from `examples/dogfood`.
- A small `devimg.toml` preset section showing widths, formats, quality, and `fit`.
- GitHub Actions summary from `devimg check`.
- `manifest export` helper usage in a frontend component.

## Technical Note Outline

1. Why generated image variants drift.
2. What DevImg owns: config, variants, manifest, report, helper export, CI check, and review artifact.
3. What DevImg does not own: hosted transformations, automatic commits, and frontend rendering components.
4. How it works with framework image components and CDNs.
5. Dogfood notes from `cleisson.com`.
6. Current limits and the v0.2 AI-assisted workflow direction.
