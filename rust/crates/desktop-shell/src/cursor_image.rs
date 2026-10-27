//! Parse the desktop shell's native image-cursor message without requiring a
//! web engine. The overlay converts the parsed pixels to its shared cursor
//! type when it handles the message.

use std::sync::Arc;

#[derive(Clone, Debug)]
pub(crate) struct ParsedCursorImage {
    pub(crate) rgba: Arc<[u8]>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) hotspot_x: u32,
    pub(crate) hotspot_y: u32,
    pub(crate) scale: f32,
}

/// A `rusty-desktop:cursor-image` line's id and image, after its prefix.
pub(crate) fn parse(line: &str) -> Option<(u64, ParsedCursorImage)> {
    let mut fields = line.split(' ');
    let mut number = || fields.next()?.parse::<u32>().ok();
    let id = u64::from(number()?);
    let (width, height, hotspot_x, hotspot_y) = (number()?, number()?, number()?, number()?);
    let scale: f32 = fields.next()?.parse().ok()?;
    let hex = fields.next()?.as_bytes();
    if hex.len() != width as usize * height as usize * 8 || scale.is_nan() || scale <= 0.0 {
        return None;
    }
    let nibble = |byte: u8| (byte as char).to_digit(16).map(|digit| digit as u8);
    let rgba: Option<Vec<u8>> = hex
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[high, low]| Some(nibble(high)? << 4 | nibble(low)?))
        .collect();
    Some((
        id,
        ParsedCursorImage {
            rgba: rgba?.into(),
            width,
            height,
            hotspot_x,
            hotspot_y,
            scale,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn a_shim_cursor_line_is_its_image() {
        // Two pixels, red then half-transparent white, hotspot at the second, two image pixels per CSS pixel.
        let (id, image) = parse("7 2 1 1 0 2 ff0000ffffffff80").unwrap();
        assert_eq!(id, 7);
        assert_eq!(
            (image.width, image.height, image.hotspot_x, image.hotspot_y),
            (2, 1, 1, 0)
        );
        assert_eq!(image.scale, 2.0);
        assert_eq!(&image.rgba[..], &[255, 0, 0, 255, 255, 255, 255, 128]);
        // Pixels that do not fill the size, or are not hex, are no image.
        assert!(parse("7 2 1 1 0 2 ff0000ff").is_none());
        assert!(parse("7 2 1 1 0 2 ff0000ffffffffzz").is_none());
    }
}
