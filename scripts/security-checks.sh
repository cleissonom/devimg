#!/usr/bin/env bash
set -euo pipefail

missing=()
for tool in gitleaks cargo-audit cargo-deny zizmor; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    missing+=("$tool")
  fi
done

if (( ${#missing[@]} > 0 )); then
  echo "Missing security tools: ${missing[*]}" >&2
  echo "Install examples:" >&2
  echo "  cargo install --locked cargo-audit" >&2
  echo "  rustup toolchain install 1.88.0 --profile minimal" >&2
  echo "  cargo +1.88.0 install --locked cargo-deny --version 0.19.6" >&2
  echo "  cargo +1.88.0 install --locked zizmor --version 1.25.2" >&2
  echo "  install gitleaks from https://github.com/gitleaks/gitleaks/releases" >&2
  exit 127
fi

gitleaks detect --redact --config .gitleaks.toml --source .
cargo audit
cargo_for_deny="$(rustup which --toolchain 1.88.0 cargo 2>/dev/null || command -v cargo)"
CARGO="$cargo_for_deny" cargo-deny check
zizmor --offline .
