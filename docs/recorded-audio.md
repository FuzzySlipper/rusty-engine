# Recorded audio containers

`Audio.OpenClip`, `OpenClipFromContent`, and `PreloadOptional` use the same
Engine container admission. No new C# request or playback handle is needed.

| Bytes / identification | Extension | MIME | Browser realization |
| --- | --- | --- | --- |
| `RIFF` + `WAVE` | `.wav` | `audio/wav` | Existing Web Audio buffer |
| `OggS`, first packet `01 vorbis` | `.ogg` | `audio/ogg` | Media-element stream |
| `OggS`, first packet `OpusHead` | `.opus` | `audio/ogg` | Media-element stream |
| MPEG Layer III frame, optionally after ID3v2 | `.mp3` | `audio/mpeg` | Media-element stream |
| `fLaC` | `.flac` | `audio/flac` | Media-element stream |

Bytes select the container; the resource path must use its matching extension.
Ogg video and AAC/M4A are not admitted. This is container identification, not a
second codec validator: corrupt payloads or a browser missing a codec produce
host `decodeFailed` diagnostics. Unknown container bytes fail at admission with
`CSHARP_AUDIO_RESOURCE_CONTAINER` (dev-host: `DEV_HOST_RENDERER_AUDIO_CONTAINER`),
naming the admitted policy. Both Rust compositions share `render_model::AudioContainer`.
Bundle descriptors and dynamic resource HTTP delivery preserve the MIME.

## Memory and execution

The C# owner retains its existing limits: **8 MiB per encoded clip, 64 distinct
clips, 32 MiB total encoded bytes**. Reopening identical bytes acquires another
owner of the same clip; mixed formats share those budgets. Optional preload
continues to distinguish missing/capacity receipts from invalid containers.
These numbers do not purport to measure decoded PCM or total browser RSS.

Compressed clips use `HTMLAudioElement` through `MediaElementAudioSourceNode`
into the ordinary Engine bus/gain/pan/spatial graph. They never call
`decodeAudioData` or retain a whole-track decoded `AudioBuffer`. One encoded
Blob is shared per cached content hash; up to 64 simultaneous compressed
streaming voices are realized per audio host. Each voice has the browser's
incremental decoder/read-ahead buffers. Their implementation-specific byte size
is browser-owned, **not an exact Engine byte cap**. Thus decoded residency is
bounded by active decoder windows/voices rather than track duration; there is
no promise that 32 MiB encoded means 32 MiB resident. Decoder scratch, resampling,
encoded delivery copies and browser caching remain additional overhead.

WAV retains its existing buffer path and compatibility. It is suitable for
short latency-sensitive effects; its decoded float PCM can exceed its input
size. Compressed media looping is browser-managed and is not a sample-accurate,
gapless-loop guarantee. Pitch disables media pitch preservation; pause/resume
reads the actual media cursor, so buffering time is not mistaken for playback.
Autoplay/codec failures are surfaced through existing realized diagnostics.
Release/reset/disposal clears the media source, releases its decoder and revokes
its object URL. Dropping clip ownership also drops an inactive encoded cache.

CoreCLR and NativeAOT hosts deliver the same admitted resources to the same
browser audio implementation. The dev-host bundle and embedded webview use that
implementation too; neither Rust composition decodes audio. Headless admission
alone does not play sound. Decoding runs in the browser media pipeline, not in
an admitted C# update or Rust simulation step; publication does not await a
whole-track decode. Browser codec availability still governs realization.

See [fixture provenance and browser regression](../fixtures/audio-containers/README.md).
Product code chooses tracks, loops, crossfades and music policy. The standard
[Web Audio media-element source](https://www.w3.org/TR/webaudio/#MediaElementAudioSourceNode)
keeps that streamed decoder connected to the existing audio graph.
