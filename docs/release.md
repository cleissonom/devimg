# Release

`devimg` releases are GitHub Releases built from version tags. The tag must match the workspace package version.

## Publish

For version `0.1.3`:

```bash
git tag v0.1.3
git push origin v0.1.3
```

The `Release` workflow builds and publishes:

- `devimg-linux-x86_64.tar.gz`
- `devimg-darwin-x86_64.tar.gz`
- `devimg-darwin-aarch64.tar.gz`
- `devimg-windows-x86_64.tar.gz`

Each archive has a matching `.sha256` checksum file.

You can also rerun the workflow manually for an existing tag from GitHub Actions by passing the tag input.

## Install From GitHub

Download the matching archive from the release page, verify its checksum, extract it, and place `devimg` on `PATH`.

Example for Linux x86_64:

```bash
curl -fsSLO https://github.com/cleissonom/devimg/releases/download/v0.1.3/devimg-linux-x86_64.tar.gz
curl -fsSLO https://github.com/cleissonom/devimg/releases/download/v0.1.3/devimg-linux-x86_64.tar.gz.sha256
sha256sum -c devimg-linux-x86_64.tar.gz.sha256
tar -xzf devimg-linux-x86_64.tar.gz
./devimg --help
```

## Cargo Install

Install directly from the Git tag:

```bash
cargo install --git https://github.com/cleissonom/devimg --tag v0.1.3 devimg-cli
```

For local development:

```bash
cargo install --path crates/devimg-cli
```

## GitHub Action

Consumer workflows should pin the Action to the release tag:

```yaml
- uses: cleissonom/devimg/action@v0.1.3
  with:
    config: devimg.toml
    mode: check
```

The Action downloads the release archive matching the runner OS and architecture unless `binary-path` is supplied.

## Private Repository Notes

GitHub Releases work in private repositories, but release assets are only visible to users and workflows with read access to that repository.

For GitHub Actions consumers, keep the repository public for broad public use. For private use, configure the repository's Actions access settings so allowed private repositories can use the private Action.
