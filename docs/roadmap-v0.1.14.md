# DevImg v0.1.14 Plan: Public Distribution Prep

`v0.1.14` prepares DevImg for public distribution before the broader v0.2 AI workflow work.

## Goals

- Make `cargo install devimg` the public CLI install path.
- Keep `devimg-core` available as the internal library crate for future integrations.
- Keep GitHub Release binaries as the fast install path for users who do not want to build from source.
- Make the public subpath Action usable as `cleissonom/devimg/action@v0.1.14`.
- Document the manual steps required before opening the repository and publishing crates.

## Scope

- Rename the publishable CLI package from `devimg-cli` to `devimg` while keeping the folder `crates/devimg-cli`.
- Add crates.io package metadata and crate-level READMEs.
- Add a registry `version` to the `devimg-core` workspace path dependency.
- Update CI and release workflows to build package `devimg`.
- Verify release archive checksums inside the Action download path.
- Refresh install, Action, release, and public sharing docs.

## Non-Goals

- Do not add a generic curl installer.
- Do not create a GitHub Marketplace listing yet.
- Do not create a hosted service.
- Do not publish crates automatically from CI.
- Do not make the repository public automatically from local tooling.

## Done Criteria

- `cargo +1.85.1 publish --dry-run -p devimg-core --allow-dirty` passes.
- `cargo +1.85.1 publish --dry-run -p devimg --allow-dirty` passes.
- CI/release workflows build the renamed package.
- Action docs describe public tag usage and checksum verification.
- `docs/public-distribution.md` lists the manual release and marketing sequence.
