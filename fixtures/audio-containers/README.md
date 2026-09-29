# Recorded audio container fixtures

These five one-second, mono 48 kHz 440 Hz sine tones were synthesized for Engine
regressions. No third-party recording or music is included. They were encoded
once with libsndfile (PCM16 WAV, Vorbis Ogg, Opus Ogg, MP3, FLAC). The Engine
has no dependency on that encoder, Python, ffmpeg, or a Rust decoder.

Rust admission tests consume these exact bodies. The browser test
`render/browser/audio-containers.browser.spec.ts` exercises actual decoding,
one-shot completion, retained looping, nonzero analyser samples and cleanup for
all five containers through `RendererAudioHost`. This proves sink behavior,
not physical speakers or product music policy.

`tone-gain6.opus` is `tone.opus` with its OpusHead output gain set to +6 dB
(Q7.8 value 1536) and the first Ogg page CRC recomputed; nothing else changes.
The Rust device decoder test checks that the gain is applied as libopus applies
it (review of #8812).
