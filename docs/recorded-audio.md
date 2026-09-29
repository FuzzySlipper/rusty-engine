# Recorded audio containers

`Audio.OpenClip`, `OpenClipFromContent`, and `PreloadOptional` use the same
Engine container admission. No new C# request or playback handle is needed.

| Bytes / identification | Extension | MIME | Device decoding |
| --- | --- | --- | --- |
| `RIFF` + `WAVE` | `.wav` | `audio/wav` | Decoded once per clip |
| `OggS`, first packet `01 vorbis` | `.ogg` | `audio/ogg` | Streamed per voice (symphonia) |
| `OggS`, first packet `OpusHead` | `.opus` | `audio/ogg` | Streamed per voice (`opus-decoder`) |
| MPEG Layer III frame, optionally after ID3v2 | `.mp3` | `audio/mpeg` | Streamed per voice (symphonia) |
| `fLaC` | `.flac` | `audio/flac` | Streamed per voice (symphonia) |

Bytes select the container; the resource path must use its matching extension.
Ogg video and AAC/M4A are not admitted. This is container identification, not a
second codec validator: a corrupt payload produces a host `decodeFailed`
diagnostic. Unknown container bytes fail at admission with
`CSHARP_AUDIO_RESOURCE_CONTAINER`, naming the admitted policy. Both Rust
compositions share `render_model::AudioContainer`.

## Memory and execution

The Engine sets no clip count or encoded-byte budget; the product decides what
to load. Reopening identical bytes acquires another owner of the same clip.
Optional preload reports a missing resource as a receipt and an invalid
container as an error.

WAV decodes once per clip, which suits short latency-sensitive effects; its
decoded float PCM can exceed its input size. Vorbis, Opus, MP3 and FLAC stream
from their encoded bytes per voice, so decoded residency is bounded by the
active voices' decoder buffers rather than track duration. Decoding runs on the
audio device's threads, not in an admitted C# update or Rust simulation step;
publication does not await a whole-track decode. Compressed looping is not a
sample-accurate, gapless-loop guarantee. Release, reset and disposal stop the
voice and release its decoder; dropping clip ownership releases the clip's
bytes.

## Device realization

The runtime process plays committed audio on its default output device
(`render-audio`, kira over cpal; on Linux the host links `libasound.so.2`),
for streamed frames and the desktop window alike. The device opens when the
runtime loads and closes when it drops. With `RUSTY_AUDIO_OUTPUT` unset, a
machine with no output device (a CI runner, a headless server) runs silent
after one warning: its audio ops are dropped and report no completions.
`RUSTY_AUDIO_OUTPUT=device` requires the device and fails the load without one.
Audio ops are taken out of each call's publications before anything else sees
them. The device follows the runtime: it plays only while the product runs, a
binding change (Start, Restart, fault) replays the committed baseline, and
Shutdown stops every voice. Natural completions and device diagnostics become
realization facts the product reads through `Audio`. The device also plays a
playing video clip's Opus soundtrack (demuxed by `render-video`) from the
clip's start, outside the Engine buses.

Opus streams through symphonia's Ogg demuxer and seeking, and the pure-Rust
`opus-decoder` (MIT/Apache-2.0, no FFI) decodes it, applying the OpusHead
output gain. Its output matches libopus for the fixtures, and seeks decode
80 ms of pre-roll. The listener follows the camera of the lowest-ordered
primary view in the committed view composition, and an entity-attached voice
follows the committed graphics node published for its entity
(`source_entity`, as `EntityGraphicsProjection` publishes it) plus its offset.
An entity with no published node reports `hostFailure`. Spatial voices use
kira's linear distance falloff between 1 and `attenuation`.

See [fixture provenance](../fixtures/audio-containers/README.md). Product code
chooses tracks, loops, crossfades and music policy.
