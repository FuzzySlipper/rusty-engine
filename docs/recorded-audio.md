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
publications before anything else sees them. Each bus follows a clock (see
[buses](#buses)), and the device plays its voices and one-shots on it, so
device positions keep pace with the Engine cursors:

- `World` buses play only while world time moves: they hold while
  [gameplay time](csharp-lifecycle.md#gameplay-time) is held, resume where
  they held, and play at the gameplay rate times their own pitch in slow
  motion. That resamples (pitch falls with the rate); there is no time
  stretch. The video soundtrack follows world time too, as its picture does.
- `Host` buses play on at their own pitch through holds and rate changes;
  their Engine cursors advance by the update's `HostElapsedSeconds`.
- Every bus holds while the runtime admits no host time: it is not running,
  or playtest inspection owns time.

World time admitted while the device holds (a playtest inspection advance)
moves every held `World` sound, one-shot and the soundtrack on by that much,
so they resume at the world moment the advance reached, and a one-shot it
carries past its end completes then; `Host` sounds stay where they held, as
an inspection step admits no host time. Each sound holds on its own while its
bus keeps running, so a sound stopped during a hold ends then, along with its
decoder thread. On resume a streaming clip (Ogg, Opus, a soundtrack) is
started again at that moment, once per hold, rather than sought, so it
resumes without first playing what its decoder had buffered. A binding
change (Start, Restart, fault) replays the committed baseline, and Shutdown
stops every voice. Natural completions and realization diagnostics become
realization facts the product reads through `Audio`. It also plays a playing video clip's Opus soundtrack
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

Opus streams through symphonia's Ogg demuxer and seeking, and the pure-Rust
`opus-decoder` (MIT/Apache-2.0, no FFI) decodes it, applying the OpusHead
output gain. Its output matches libopus for the fixtures, and seeks decode
80 ms of pre-roll. The listener follows the camera of the lowest-ordered
primary view in the committed view composition, and an entity-attached voice
follows the committed graphics node published for its entity
(`source_entity`, as `EntityGraphicsProjection` publishes it) plus its offset.
An entity with no published node reports `HostFailure`.

## Buses

Every voice and one-shot plays on one of four buses, `Sfx`, `Ambient`, `Ui`
and `Music`, each with its own volume, mute and clock (`Audio.SetBusVolume`,
`SetBusMuted`, `SetBusClock`, and `ReadBus` for all three), so a product can
give players separate music, ambience, effects and interface levels.

A bus's clock (`AudioClock`) says which time its sounds follow. `Sfx` follows
`World` and `Ambient`, `Ui` and `Music` follow `Host`, so a menu, inventory or
dialogue that holds gameplay time keeps its music, ambience and interface
sounds while effects wait for the world. A product that wants otherwise sets a
bus's clock at runtime, once at setup or whenever its needs change; sounds
already playing on the bus follow the new clock from then on:

```csharp
// Ambience in this game is the world's: it pauses with it.
engine.Audio.SetBusClock(new AudioBusClockRequest(AudioBus.Ambient, AudioClock.World));
```

A sound that should pause with the world goes on a `World` bus; there are no
other tags.

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

## World acoustics

`Audio.SetAcousticSpace(session, reverbLevel)` names the spatial session the
listener and every spatial source are in; its collision (voxels and static
meshes, built work and heightfield tiles included; entity boxes take no part)
then acts on every `World3d` and `EntityAttached` source whose descriptor's
`Acoustics` is `World`, the default. `Dry` plays a source as before. A product
that moves the player into another session (a dungeon) names that one;
`ClearAcousticSpace`, or that session's disposal, plays everything dry again.

A sound reaches the listener the louder of two ways:

- **Straight through.** It falls off with distance, and solid on the line
  costs 6 dB plus 10 dB per metre and low-passes it from 20 kHz, halving the
  cutoff every 0.25 m down to 250 Hz: a metre of rock is about -16 dB at
  1.25 kHz, muffled rather than silent. Through solid, the centre ray is
  averaged with two rays 0.3 m to either side, so the edge of a wall muffles
  part of a sound rather than all of it at once.
- **Round, by open air.** A search over 1 m air cells joined where the
  segment between their centres is clear finds the shortest open path; the
  sound falls off with that path's length, unfiltered, and is heard from the
  way the path leaves the listener.

The listener's space adds reverb on each bus by how enclosed it is: thirteen
24 m rays (up, eight level bearings, four rising) set the send (the share
that meets collision, times `reverbLevel`) and how long it rings (their mean
free length). Open ground does not ring; a cave does. A bus's reverb follows
its volume and mute.

`Audio.QueryArrival(session, source, range, points, rolloff)` answers, by the
same model, how a sound made at `source` arrives at each point: its `Gain`
(the amplitude playback would play it at, before the source's volume and
bus), `LowPassHz`, `Path` (`Silent`, `Open`, `Occluded` or `Around`),
`PathMetres` and `ArrivesFrom` (about three metres back along its path, or
the source). The product owns what loud means: its range, any threshold, and
what a listener does with what arrives.

Limits and cost:

- Playback refreshes acoustics ten times a second for the 32 nearest audible
  sounds, and for a new sound in the call that starts it; changes glide over
  100 ms. One open-air search from the listener serves every occluded sound
  and is rebuilt at most four times a second as the listener moves or the
  collision changes.
- An open-air search settles at most 4096 cells and follows paths no longer
  than the range; a query's readout reports `CellsSearched`, `RaysCast` and
  `Truncated`. A query takes at most 64 points; it searches only when a point
  in range is occluded. There is no per-update query limit.
- The listener's search runs on its own worker thread; the update only casts
  each sound's straight rays and looks its path up. In a 96 × 24 × 96 m
  cave on den-agents (busy, release), a refresh costs about 2–4 µs per
  occluded sound with voxel rock and 8–13 µs with static-mesh rock, and the
  worker's search 2.5–5 ms. A query costs well under a microsecond when its
  points are in the open, and 2–5 ms, on the calling update, when it must
  search round rock to them.

Kira realizes the filtering, sends and reverb for both outputs: the device,
and the stream the viewing page plays, so both hear the same mix.

See [fixture provenance](../fixtures/audio-containers/README.md). Product code
chooses tracks, loops, crossfades and music policy.
