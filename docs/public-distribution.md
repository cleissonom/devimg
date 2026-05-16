# Public Distribution

This checklist is for the first intentionally public DevImg release.

## Before Making The Repo Public

- Confirm the working tree contains no ignored planning files or local artifacts.
- Run the full local verification set in `docs/release.md`.
- Run `scripts/security-checks.sh`.
- Confirm the GitHub Actions `Gitleaks`, `Rust Security`, and `Workflow Security` jobs pass on the public-distribution branch.
- After the repository is public, confirm the public-only `CodeQL` and `Scorecard` workflows pass.
- Review public docs for private-only wording, personal paths, or private tokens.
- Confirm GitHub repository description and topics are set.

Suggested repository description:

```text
Deterministic Rust image pipeline for frontend repositories.
```

Suggested topics:

```text
rust, cli, images, image-optimization, responsive-images, web-performance, github-actions, ci, static-site, nextjs, astro, vite
```

## crates.io

The first public publish should reserve both package names:

```bash
cargo login <crates.io-api-token>
cargo +1.85.1 publish -p devimg-core
cargo +1.85.1 publish --dry-run -p devimg
cargo +1.85.1 publish -p devimg
```

Publish `devimg-core` first because the CLI package depends on it through a registry version plus local path dependency.
Wait until the new `devimg-core` version appears in the crates.io index before publishing `devimg`; otherwise the CLI publish will fail with `no matching package named devimg-core found`.

After publish:

```bash
cargo install devimg
devimg --version
devimg --help
```

Crates are effectively permanent. If a bad version is published, yank it with `cargo yank`; do not treat yanking as secret removal.

## GitHub Release

After crates.io publish succeeds, tag the same commit:

```bash
git tag v0.1.14
git push origin v0.1.14
```

Wait for the Release workflow to publish Linux, macOS, and Windows archives plus `.sha256` files. Download one archive, verify its checksum, and run `devimg --version`.

## Public GitHub Action

After the repository is public and `v0.1.14` release assets exist, users can run:

```yaml
- uses: cleissonom/devimg/action@v0.1.14
  with:
    config: devimg.toml
    mode: check
```

The Action downloads the matching release archive, verifies the `.sha256` checksum, and runs `devimg`.

Do not pursue GitHub Marketplace for this release. The current Action intentionally lives under `action/` in the main repository; Marketplace can be revisited later with a separate root-level Action repository if public usage justifies it.

## cleisson.com

After the public release is live, update the DevImg project page with:

- crates.io install command: `cargo install devimg`;
- public Action usage: `cleissonom/devimg/action@v0.1.14`;
- release binary checksum verification;
- dogfood proof from `cleisson.com` CI and production usage;
- a link to the visual review artifact flow.

## LinkedIn Sequence

Post 1: Launch and problem.

- Frontend image variants drift easily.
- DevImg keeps source images, generated variants, manifest, report, helper exports, and CI checks together.
- Ask for early users who maintain image-heavy frontend repos.

Post 2: Technical workflow.

- Show `devimg.toml`, `devimg optimize`, `devimg manifest export`, `devimg check`, and `devimg review`.
- Include a screenshot of the static review artifact.
- Emphasize deterministic local-first behavior, not SaaS.

Post 3: Dogfood case study.

- Explain how `cleisson.com` uses generated project images.
- Mention content-hash filenames, Vercel/CDN compatibility, and CI enforcement.
- Ask for feedback on framework consumption patterns.
