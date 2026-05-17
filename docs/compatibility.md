# Legacy Tool Compatibility

DevImg preserves the useful behavior from the sibling `imgconvert` and `imgcrop` ideas without depending on those relative paths.

- Source extension normalization accepts `jpg`, `jpeg`, `png`, and `webp`.
- Output format normalization accepts `jpg`, `jpeg`, `png`, `webp`, and opt-in `avif`.
- Actual image bytes are checked against the file extension before processing.
- Output naming is deterministic and avoids input/output collisions.
- Cover cropping keeps the old center-framed `resize -> crop` behavior by default with Lanczos filtering.
- Existing changed outputs are refused unless overwrite is explicitly enabled.

Intentional changes:

- Manual argument parsing from the old tools is replaced by `clap`-backed `devimg` subcommands.
- Single-image commands are replaced by config-driven source scanning and operation planning.
- PNG, JPEG, and WebP remain the stable source formats. AVIF is supported as an output-only format.
- The manifest stores BLAKE3 config, content, and operation hashes so CI can detect stale outputs without relying on timestamps.
- Cover crop positioning is now configurable with anchors or normalized focal points.
- Source-specific preset overrides are opt-in; configs keep global preset behavior until `[[overrides]]` entries are added.
