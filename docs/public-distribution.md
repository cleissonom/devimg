# Distribution

DevImg is distributed as public crates.io packages, GitHub Release binaries, and a subdirectory GitHub Action.

## Public Channels

- Repository: <https://github.com/cleissonom/devimg>
- CLI crate: <https://crates.io/crates/devimg>
- Core crate: <https://crates.io/crates/devimg-core>
- Current release: `v0.1.15`
- Minimum Rust version for source installs: `1.88`
- GitHub Action: `cleissonom/devimg/action@v0.1.15`

## CLI Install

The primary install path is crates.io:

```bash
cargo install devimg
devimg --version
```

Source installs require Rust 1.88 or newer. If the active default toolchain is older, run `rustup update stable` or install with an explicit toolchain:

```bash
cargo +1.88.0 install devimg
```

Users who do not want to build from source can download a GitHub Release archive for their platform and verify the matching `.sha256` checksum before running the binary.

## GitHub Action

Consumer workflows should pin a release tag:

```yaml
- uses: cleissonom/devimg/action@v0.1.15
  with:
    mode: check
```

The Action downloads the matching GitHub Release archive, verifies its `.sha256` checksum, and runs `devimg`. Projects can pass `binary-path` when testing the Action from a local checkout.

The Action is not published through GitHub Marketplace. It intentionally lives under `action/` in this repository until a separate Marketplace release becomes useful.

## Maintainer Checklist

Before a public release:

- Confirm the version is consistent across the workspace, docs, Action defaults, and changelog.
- Run the verification commands in `docs/release.md`.
- Run `scripts/security-checks.sh`.
- Confirm GitHub security jobs pass after the release commit is pushed.
- Publish `devimg-core` before `devimg`.
- Tag the same commit after crates.io publish succeeds.
- Verify the release workflow published archives and checksum files for Linux, macOS, and Windows.

Suggested repository description:

```text
Deterministic Rust image pipeline for frontend repositories.
```

Suggested topics:

```text
rust, cli, images, image-optimization, responsive-images, web-performance, github-actions, ci, static-site, nextjs, astro, vite
```

## Distribution Boundaries

Current public distribution does not include:

- automatic crates.io publishing from CI;
- a generic shell installer;
- a GitHub Marketplace listing;
- hosted image storage, accounts, dashboards, or SaaS features;
- automatic PR commits from the Action.
