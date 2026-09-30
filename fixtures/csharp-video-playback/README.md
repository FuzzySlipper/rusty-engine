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

`proof.webm` is FFmpeg's `testsrc2` pattern, 320x180 at 24 fps, eight
seconds, VP9 profile 0, no audio: the format the runtime admits. It plays to
Completed. Regenerate it with:

```bash
ffmpeg -f lavfi -i testsrc2=size=320x180:rate=24 -t 8 -c:v libvpx-vp9 -profile:v 0 \
  -pix_fmt yuv420p -b:v 200k -g 24 -an proof.webm
```

`invalid.webm` contains only a broken EBML/WebM header for the decoder-failure
probe.
