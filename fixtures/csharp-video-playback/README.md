# Packaged video and failure-diagnostic proof

Build this ordinary product with an explicit `RustyEngineFixtureSdkVersion`
and `RestoreAdditionalProjectSources` pointing at the matching SDK feed. Run
through that pair's `rusty dev`. The runtime renderer plays the clips
(`render-video`, `render-wgpu`); the product has no decoder or renderer.

- `video.proof.play` plays `proof.webm`.
- `video.proof.inspect` reads retained terminal facts. Each playback yields
  exactly one Completed, Skipped or Failed fact.
- `video.proof.fail` requests an intentionally corrupt WebM body; inspect for
  exactly one Failed/DecodeFailed fact.
- `video.proof.skip` skips the current playback and reports Skipped.
- `diagnostics.proof` catches the refusals for an unknown Audio clip handle and
  a missing Graphics resource, verifies the copied
  `EngineCallException.Diagnostics` and message text, and publishes two
  `FIXTURE_NATIVE_REASON` observations to the Engine diagnostic sink. The
  product keeps running afterward.

`proof.webm` is an authored test pattern generated with FFmpeg `testsrc2`,
320x180 at 24 fps, eight seconds, VP8, no audio. The runtime admits WebM with
one VP9 profile 0 track only, so `video.proof.play` on this clip reports
Failed/DecodeFailed. `invalid.webm` contains only a broken EBML/WebM header for
the decoder-failure probe.
