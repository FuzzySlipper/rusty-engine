# Packaged audio admission proof

Build with `-p:RustyEngineFixtureSdkVersion=VERSION` using the matching pair's
SDK feed. Launch with that pair's `rusty dev --project ... --live-debug`.
This explicit Engine fixture consumes the shared synthesized bodies in
`../audio-containers`; ordinary products carry their own content directory.

Start opens WAV/Vorbis/Opus/MP3/FLAC through both `OpenClip` and
`OpenClipFromContent`, checks deduplicated handle identity, and releases the
second owner. Execute `audio.proof.play` with `rusty-live-debug`. It creates
five looping voices and five one-shot emissions. With the default `stream`
output they play in the page watching the frames after its first click or key
press; with `device-optional` or `device-required` on the runtime's output
device. After the tones complete, `audio.proof.inspect` should report
five admitted clips/five active voices and natural completion facts.
`audio.proof.stop` releases loops. `audio.proof.bus <volume> <muted>` sets
the Ambient bus every proof voice plays on. Product logic has no format decoder,
resampler or audio device control.

Spatial proof: `audio.proof.face <yaw>` turns the active camera (the device
listener), `audio.proof.world` loops the WAV tone from (3, 0, 0), and
`audio.proof.entity <x> <z>` publishes entity 900 as a cube there with a looping
tone attached to it. `audio.proof.stop` releases these too.
