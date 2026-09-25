//! Shared recorded-audio container admission. Decoding belongs to the browser sink.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioContainer {
    Wave,
    Vorbis,
    Opus,
    Mp3,
    Flac,
}

impl AudioContainer {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Wave => ".wav",
            Self::Vorbis => ".ogg",
            Self::Opus => ".opus",
            Self::Mp3 => ".mp3",
            Self::Flac => ".flac",
        }
    }

    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Wave => "audio/wav",
            Self::Vorbis | Self::Opus => "audio/ogg",
            Self::Mp3 => "audio/mpeg",
            Self::Flac => "audio/flac",
        }
    }

    /// Identify a supported container/codec prefix, not decoder conformance.
    /// Ogg's first identification packet distinguishes audio from Theora/etc.
    pub fn identify(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 44 && bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
            return Some(Self::Wave);
        }
        if bytes.starts_with(b"fLaC") && bytes.len() >= 42 {
            return Some(Self::Flac);
        }
        if bytes.starts_with(b"OggS") && bytes.get(4) == Some(&0) {
            let segments = usize::from(*bytes.get(26)?);
            let lacing = bytes.get(27..27 + segments)?;
            let packet_length: usize = lacing
                .iter()
                .take_while(|&&n| n == 255)
                .map(|&n| usize::from(n))
                .sum::<usize>()
                + usize::from(*lacing.iter().find(|&&n| n < 255)?);
            let packet = bytes.get(27 + segments..27 + segments + packet_length)?;
            if packet.starts_with(b"\x01vorbis") && packet.len() >= 30 {
                return Some(Self::Vorbis);
            }
            if packet.starts_with(b"OpusHead") && packet.len() >= 19 {
                return Some(Self::Opus);
            }
            return None;
        }
        // Skip an ID3v2 tag, then require an MPEG Layer III frame header.
        let offset = if bytes.starts_with(b"ID3") {
            let size = bytes.get(6..10)?;
            if size.iter().any(|v| v & 0x80 != 0) {
                return None;
            }
            10 + size.iter().fold(0usize, |n, &v| (n << 7) | usize::from(v))
        } else {
            0
        };
        let header = bytes.get(offset..offset + 4)?;
        if header[0] == 0xff
            && header[1] & 0xe0 == 0xe0
            && header[1] & 0x18 != 0x08
            && header[1] & 0x06 == 0x02
            && header[2] & 0xf0 != 0xf0
            && header[2] & 0xf0 != 0
            && header[2] & 0x0c != 0x0c
        {
            return Some(Self::Mp3);
        }
        None
    }
}

pub const AUDIO_CONTAINER_POLICY: &str =
    "audio container must be RIFF/WAVE, Ogg Vorbis, Ogg Opus, MPEG Layer III (MP3), or FLAC";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_audio_not_just_an_ogg_or_id3_signature() {
        for (prefix, length, expected) in [
            (b"\x01vorbis".as_slice(), 30, AudioContainer::Vorbis),
            (b"OpusHead".as_slice(), 19, AudioContainer::Opus),
        ] {
            let mut body = vec![0; 28 + length];
            body[..4].copy_from_slice(b"OggS");
            body[26] = 1;
            body[27] = length as u8;
            body[28..28 + prefix.len()].copy_from_slice(prefix);
            assert_eq!(AudioContainer::identify(&body), Some(expected));
            body[28] = 0;
            assert_eq!(AudioContainer::identify(&body), None);
        }
        assert_eq!(
            AudioContainer::identify(&[0xff, 0xfb, 0x90, 0]),
            Some(AudioContainer::Mp3)
        );
        assert_eq!(
            AudioContainer::identify(b"ID3\x04\0\0\0\0\0\0\xff\xfb\x90\0"),
            Some(AudioContainer::Mp3)
        );
        assert_eq!(AudioContainer::identify(b"ID3\x04\0\0\0\0\0\0"), None);
        assert_eq!(AudioContainer::identify(b"OggS"), None);
        assert_eq!(AudioContainer::identify(b"\0\0\0\x18ftypM4A "), None);
    }
}
