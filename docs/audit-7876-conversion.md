# Audit 7876: conversion source capacity

- Removed the inherited 64 MiB conversion-source policy from request validation,
  volume and object provenance validation, public exports, and both converter
  CLIs. Source-byte provenance remains a nonzero `u64` fact.
- Converter CLIs now use ordinary `fs::read` for source files. Source SHA-256
  identity, actual GLB parsing, checked representation/format validation,
  conversion-work budgets, output-grid bounds, and palette checks remain.
- Removed the now-unused conversion source vertex/index constants after stream
  representation moved to checked `u32` counts; this does not change
  voxelization work or output limits.
- Removed the arbitrary 1 MiB JSON-envelope checks from both conversion
  request decoders and CLIs. Typed JSON decoding and malformed-request
  rejection remain; no parser-envelope quota is approved merely by its name.

## Inspector appendix

- `rusty-inspect import-manifest` no longer applies its command-only 4 MiB
  read cap and now uses the ordinary text read used by `import-source`.
  Inspector format validation and other command/operational limits are
  separate and remain unreviewed here.
