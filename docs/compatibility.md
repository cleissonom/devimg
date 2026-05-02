# Legacy Tool Compatibility

This MVP preserves the useful behavior from the sibling `imgconvert` and `imgcrop` tools without depending on those relative paths.

- Extension normalization accepts `jpg`, `jpeg`, `png`, and `webp`.
- Actual image bytes are checked against the file extension before processing.
- Output naming is deterministic and avoids input/output collisions.
- Cover cropping keeps the old center-framed `resize -> crop` behavior with Lanczos filtering.
- Existing changed outputs are refused unless overwrite is explicitly enabled.

Intentional changes:

- Manual argument parsing from the old tools is replaced by `clap`-backed `devimg` subcommands.
- Single-image commands are replaced by config-driven source scanning and operation planning.
- PNG, JPEG, and WebP are the only MVP formats.
- The manifest stores BLAKE3 config, content, and operation hashes so CI can detect stale outputs without relying on timestamps.
