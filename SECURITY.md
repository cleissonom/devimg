# Security Policy

## Reporting

Please do not open public issues for suspected vulnerabilities.

Report security concerns privately through GitHub Security Advisories when available, or contact the repository owner through the GitHub profile linked from the repository.

Include:

- Affected version or commit.
- Impact and attack scenario.
- Steps to reproduce.
- Any relevant config, manifest, Action, or CLI output with secrets redacted.

## Scope

In scope:

- Path traversal or unsafe file writes.
- GitHub Action command injection or unsafe input handling.
- Secret or personal data exposure in reports, manifests, logs, or generated artifacts.
- Dependency vulnerabilities.
- Malformed image/config handling that can crash CI or corrupt generated outputs.

Out of scope:

- Hosted-service vulnerabilities; DevImg is local-first and has no hosted service.
- Reports requiring leaked credentials or access to private repositories without authorization.

## Supported Versions

DevImg is pre-1.0. Security fixes target the latest `main` branch and the latest published GitHub Release tag when practical.
