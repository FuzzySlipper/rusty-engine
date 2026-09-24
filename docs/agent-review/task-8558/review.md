# Task 8558 — live content admission

The ordinary generated Content service admits copied bytes and an explicit,
private companion-file set into an independently disposable ContentReference.
The existing Animation importer and renderer consume that reference, preserving
GLB materials, textures, skins and clips. Transient imports bypass the immutable
startup cache so their source and resource owners can release their payloads.
Animation also exposes admitted bounds/counts and copied clip metadata for editor
framing and playback controls. Failed content imports return operation diagnostics
without poisoning an otherwise valid product update.

## Proof

- `cargo test -p csharp-engine-services --lib`: 153 passed, 2 existing ignored.
  Live-source tests cover copied bytes, source release, private external image
  closure, resource release, and keeping a previous appearance usable after a
  malformed replacement.
- `scripts/test-csharp-sdk-package.sh --coreclr-smoke`: passed. The generated
  package consumer admits managed bytes after Create, mutates the source array,
  reads mesh/clip facts, catches malformed admission, then publishes and releases
  the valid animated model. This proves the C# marshalling and real CoreCLR update
  callback as well as the Rust service tests.
- SDK/runtime pair publication and workbench browser consumption are recorded in
  the Den task closeout; source proof alone is not browser fidelity acceptance.

## Review

- Engine capability reuse: passed; existing content, import, animation and
  resource owners extended, no competing renderer or resolution authority.
- Existing owner reuse: passed; private source context and transient cache policy
  remain in RuntimeContentBridge/RuntimeAppearanceBridge.
- Runtime trust/error paths: pending.

No product file-selection policy, filesystem access route, renderer, startup
bundle rebuild, or independent asset catalog was added upstream. The workbench
receives the pair in asset-pipeline #8559. Other gallery-only behavior is not
claimed by this Engine task.
