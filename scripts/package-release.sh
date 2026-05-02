#!/usr/bin/env bash
set -euo pipefail

target="${1:?target triple is required}"
asset_os="${2:?asset OS is required}"
asset_arch="${3:?asset architecture is required}"
profile="${PROFILE:-release}"

case "$asset_os" in
  linux | darwin)
    binary_name="devimg"
    ;;
  windows)
    binary_name="devimg.exe"
    ;;
  *)
    echo "unsupported asset OS: $asset_os" >&2
    exit 2
    ;;
esac

root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
binary="$root/target/$target/$profile/$binary_name"
dist="$root/dist"
asset_base="devimg-$asset_os-$asset_arch"
staging="$dist/$asset_base"
archive="$dist/$asset_base.tar.gz"

if [[ ! -f "$binary" ]]; then
  echo "binary not found: $binary" >&2
  exit 1
fi

rm -rf "$staging" "$archive" "$archive.sha256"
mkdir -p "$staging"
cp "$binary" "$staging/$binary_name"

(
  cd "$staging"
  tar -czf "../$asset_base.tar.gz" "$binary_name"
)

(
  cd "$dist"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$asset_base.tar.gz" > "$asset_base.tar.gz.sha256"
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$asset_base.tar.gz" > "$asset_base.tar.gz.sha256"
  else
    echo "sha256sum or shasum is required" >&2
    exit 1
  fi
)

echo "$archive"
