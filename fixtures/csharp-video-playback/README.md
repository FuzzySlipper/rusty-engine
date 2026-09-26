# Packaged video and failure-diagnostic proof

Build this ordinary product with an explicit `RustyEngineFixtureSdkVersion`
and `RestoreAdditionalProjectSources` pointing at the matching SDK feed. Run
through that pair's `rusty dev`; no copied browser or product renderer is used.

- `video.proof.play` plays the eight-second generated WebM test pattern.
- `video.proof.inspect` reads retained terminal facts. Each playback must yield
  exactly one Completed fact (or an explicit browser playback failure).
- `video.proof.fail` requests an intentionally corrupt WebM body; inspect for
  exactly one Failed/DecodeFailed fact.
- `video.proof.skip` skips the current playback and reports Skipped.
- Run `diagnostics.proof` last. It deliberately refuses an unknown Audio clip handle and a missing
  Graphics resource, verifies copied `EngineCallException.Diagnostics` and
  message text, and publishes two `FIXTURE_NATIVE_REASON` observations to the
  Engine diagnostic sink. Existing callback-failure semantics still apply:
  this negative probe can fault its host and requires a fresh launch afterward.

`proof.webm` is an authored test pattern generated with FFmpeg `testsrc2`,
320x180 at 24 fps, eight seconds, VP8, no audio. `invalid.webm` intentionally
contains only a broken EBML/WebM header for a decoder-failure probe.
