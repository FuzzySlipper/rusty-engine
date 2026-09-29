# Recorded audio container fixtures

These five one-second, mono 48 kHz 440 Hz sine tones were synthesized for Engine
regressions. No third-party recording or music is included. They were encoded
once with libsndfile (PCM16 WAV, Vorbis Ogg, Opus Ogg, MP3, FLAC). The Engine
has no dependency on that encoder, Python, ffmpeg, or a Rust decoder.

Rust admission tests consume these exact bodies, and `render-audio`'s tests
play them through its realizer on kira's mock backend: a one-shot completes for
every container, and looping and retained voices resume and complete. This
proves sink behavior, not physical speakers or product music policy.

`tone-gain6.opus` is `tone.opus` with its OpusHead output gain set to +6 dB
(Q7.8 value 1536) and the first Ogg page CRC recomputed; nothing else changes.
The Rust device decoder test checks that the gain is applied as libopus applies
it (review of #8812).
