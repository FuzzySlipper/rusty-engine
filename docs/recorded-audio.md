# Recorded audio containers

`Audio.OpenClip`, `OpenClipFromContent`, and `PreloadOptional` share one
Engine container admission. Every container uses the same C# requests and
playback handles.

| Bytes / identification | Extension | MIME | Device decoding |
| --- | --- | --- | --- |
| `RIFF` + `WAVE` | `.wav` | `audio/wav` | Decoded once per clip |
| `OggS`, first packet `01 vorbis` | `.ogg` | `audio/ogg` | Streamed per voice (symphonia) |
| `OggS`, first packet `OpusHead` | `.opus` | `audio/ogg` | Streamed per voice (`opus-decoder`) |
| MPEG Layer III frame, optionally after ID3v2 | `.mp3` | `audio/mpeg` | Streamed per voice (symphonia) |
| `fLaC` | `.flac` | `audio/flac` | Streamed per voice (symphonia) |

Bytes select the container; the resource path must use its matching extension.
Ogg video and AAC/M4A are not admitted. This is container identification, not a
second codec validator: a corrupt payload produces a `DecodeFailed` realization
diagnostic from the device. Unknown container bytes fail at admission with
`CSHARP_AUDIO_RESOURCE_CONTAINER`, naming the admitted containers. Admission
(`csharp-engine-services`) and the device (`render-audio`) share
`render_model::AudioContainer`.

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

## Realization

One realizer (`render-audio`, kira) plays the committed audio wherever the
manifest's `audio.output` says. Audio ops are taken out of each call's
publications before anything else sees them. The realizer follows the runtime:
it plays only while the product runs, a binding change (Start, Restart, fault)
replays the committed baseline, and Shutdown stops every voice. Natural
completions and realization diagnostics become realization facts the product
reads through `Audio`. It also plays a playing video clip's Opus soundtrack
(demuxed by `render-video`) from the clip's start, outside the Engine buses.

`Audio.Emit` returns an Engine signal handle for each one-shot, including
repeated signal labels. A product that ends that playback early may call
`Audio.RetireOneShot` with the handle. Retirement stops the active device
playback, releases the pending clip owner, and produces no natural-completion
fact; repeated or already-completed retirement is harmless. Shutdown silences
the output and clears these pending realization owners before product disposal,
so products can release their final clip references in `Dispose`.

- `stream`, the default with streamed frames: the runtime opens no device. It
  mixes in real time, in 10 ms blocks, and the host serves the mix at
  `GET /__rusty/product/runtime/audio` as one long response of interleaved
  stereo 16-bit little-endian PCM at 48 kHz. The shell page starts listening on
  its first pointer press or key press, since a browser lets a page play sound
  only after a gesture, and plays from a 60 ms jitter buffer that it trims
  whenever more than 250 ms pile up. Every watching page that has had a gesture
  plays the same mix; a page that falls behind skips ahead rather than queueing
  sound. Voices advance and complete on the runtime's clock whether or not a
  page listens, so completion facts read the same with no page watching.
- `device-optional`, the default with window output: the runtime plays on its
  default output device (on Linux the host links `libasound.so.2`). The device
  opens when the runtime loads and closes when it drops. A machine with no
  output device (a CI runner, a headless server) runs silent after one warning:
  its audio ops are dropped and report no completions.
- `device-required` (`RustyEngineProductAudioOutput`, or `rusty dev
  --audio-output device-required`) fails the load without a device.

Every voice plays on one of four buses, `Sfx`, `Ambient`, `Ui` and `Music`,
each with its own volume and mute (`Audio.SetBusVolume`, `SetBusMuted`,
`ReadBus`), so a product can give players separate music, ambience, effects
and interface levels.

Opus streams through symphonia's Ogg demuxer and seeking, and the pure-Rust
`opus-decoder` (MIT/Apache-2.0, no FFI) decodes it, applying the OpusHead
output gain. Its output matches libopus for the fixtures, and seeks decode
80 ms of pre-roll. The listener follows the camera of the lowest-ordered
primary view in the committed view composition, and an entity-attached voice
follows the committed graphics node published for its entity
(`source_entity`, as `EntityGraphicsProjection` publishes it) plus its offset.
An entity with no published node reports `HostFailure`.

## Spatial range

A `World3d` or `EntityAttached` source's `AudioSourceDescriptor.MaxDistance`
is its audible range in world units, and `Rolloff` is how its level falls
with its distance from the listener. It plays at its full volume within the
reference distance, `min(1, MaxDistance / 2)`, and is silent at and beyond
`MaxDistance`; a voice that moves across that bound reaches silence within one
128-frame audio block (2.7 ms at 48 kHz). Between the two, `AudioRolloff.Linear`
lowers the amplitude linearly, and `LinearDecibels` lowers the level linearly
in decibels to -60 dB. `MaxDistance` must be positive and finite, or the call
refuses with `CSHARP_AUDIO_PROJECTION`. The range applies whatever the
`SpatialBlend`, which only sets how strongly the source pans; a `Global2d`
source has no range. To change a retained voice's range, `UpdateVoice` with the
new descriptor: the device replays it from its cursor with the new range. The
product chooses a source's range and any rule that scales it.

See [fixture provenance](../fixtures/audio-containers/README.md). Product code
chooses tracks, loops, crossfades and music policy.
