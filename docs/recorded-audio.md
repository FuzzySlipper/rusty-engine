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

The Engine sets no clip count or encoded-byte budget; the product decides what
to load. Reopening identical bytes acquires another owner of the same clip.
Optional preload reports a missing resource as a receipt and an invalid
container as an error.

Compressed clips use `HTMLAudioElement` through `MediaElementAudioSourceNode`
into the ordinary Engine bus/gain/pan/spatial graph. They never call
`decodeAudioData` or retain a whole-track decoded `AudioBuffer`. One encoded
Blob is shared per cached content hash; up to 64 simultaneous compressed
streaming voices are realized per audio host. Each voice has the browser's
incremental decoder/read-ahead buffers. Their implementation-specific byte size
is browser-owned, **not an exact Engine byte cap**. Thus decoded residency is
bounded by active decoder windows/voices rather than track duration; there is
no fixed ratio between encoded and resident bytes. Decoder scratch, resampling,
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
browser audio implementation by default. Headless admission alone does not play
sound. Decoding runs in the browser media pipeline, not in an admitted C#
update or Rust simulation step; publication does not await a whole-track
decode. Browser codec availability still governs browser realization.

## Device realization

`RUSTY_AUDIO_OUTPUT=device` makes the runtime process play committed audio on
its default output device (`render-audio`, kira over cpal; on Linux the host
links `libasound.so.2`). The device opens when the runtime loads and closes when
it drops; an unknown value or a device that will not open fails the load. Audio
ops are then removed from the published presentation, so the browser neither
plays nor reports them. The device follows the runtime: it plays only while the
product runs, a binding change (Start, Restart, fault) replays the committed
baseline, and Shutdown stops every voice. Natural completions and device
diagnostics become the same realization facts the browser reports.

WAV decodes once per clip; Vorbis, MP3 and FLAC stream from their encoded bytes
per voice (symphonia). Opus streams the same way: symphonia demuxes the Ogg
stream and seeks, and the pure-Rust `opus-decoder` (MIT/Apache-2.0, no FFI)
decodes it. Its output matches libopus for the fixture, and seeks decode 80 ms of
pre-roll. The listener follows the camera of the lowest-ordered primary view
in the committed view composition, and an entity-attached voice follows the
committed graphics node published for its entity (`source_entity`, as
`EntityGraphicsProjection` publishes it) plus its offset. An entity with no
published node reports `hostFailure`. The browser realization keeps its listener
at the origin until it is deleted. Spatial voices use
kira's linear distance falloff between 1 and `attenuation` rather than Web
Audio's inverse model. The browser realization stays the development default
until the TypeScript renderer lane is deleted.

See [fixture provenance and browser regression](../fixtures/audio-containers/README.md).
Product code chooses tracks, loops, crossfades and music policy. The standard
[Web Audio media-element source](https://www.w3.org/TR/webaudio/#MediaElementAudioSourceNode)
keeps that streamed decoder connected to the existing audio graph.
