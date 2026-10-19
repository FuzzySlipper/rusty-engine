// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Latest cursor shape reported by CEF.
//!
//! Under windowless rendering the host owns the pointer, so CEF can only ask
//! for a shape and hope someone applies it. Ignore `OnCursorChange` and the
//! page looks inert: no I-beam over text, no hand over links.

use std::sync::Mutex;

use crate::surface::CursorShape;

#[derive(Debug, Default)]
pub(crate) struct LatestCursor(Mutex<Option<CursorShape>>);

impl LatestCursor {
    pub(crate) fn set(&self, shape: CursorShape) {
        *self.0.lock().unwrap() = Some(shape);
    }

    /// Take the pending shape, if it changed since the last poll.
    pub(crate) fn take(&self) -> Option<CursorShape> {
        self.0.lock().unwrap().take()
    }
}

/// Map CEF's cursor change onto the shared vocabulary, copying a custom
/// cursor's pixels: CEF's buffer lives only for the callback.
#[cfg(feature = "cef-runtime")]
pub(crate) fn from_cef_change(
    type_: cef::CursorType,
    custom: Option<&cef::CursorInfo>,
) -> CursorShape {
    match custom {
        Some(info) if type_ == cef::CursorType::CUSTOM => {
            image_from_cef(info).map_or_else(|| from_cef(type_), CursorShape::Image)
        }
        _ => from_cef(type_),
    }
}

/// Chromium hands cursors over as native-order (BGRA) premultiplied pixels.
#[cfg(feature = "cef-runtime")]
fn image_from_cef(info: &cef::CursorInfo) -> Option<crate::surface::CursorImage> {
    let width = u32::try_from(info.size.width).ok()?;
    let height = u32::try_from(info.size.height).ok()?;
    if info.buffer.is_null() || width == 0 || height == 0 {
        return None;
    }
    let len = width as usize * height as usize * 4;
    // SAFETY: CEF documents the buffer as width * height 32-bit pixels, valid
    // for the duration of OnCursorChange.
    let bgra = unsafe { std::slice::from_raw_parts(info.buffer as *const u8, len) };
    Some(crate::surface::CursorImage {
        rgba: bgra_premultiplied_to_rgba(bgra).into(),
        width,
        height,
        hotspot_x: info.hotspot.x.clamp(0, width as i32 - 1) as u32,
        hotspot_y: info.hotspot.y.clamp(0, height as i32 - 1) as u32,
        scale: info.image_scale_factor,
    })
}

fn bgra_premultiplied_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(bgra.len());
    for px in bgra.chunks_exact(4) {
        let a = u16::from(px[3]);
        let straight = |c: u8| {
            if a == 0 {
                0
            } else {
                ((u16::from(c) * 255 + a / 2) / a).min(255) as u8
            }
        };
        rgba.extend_from_slice(&[straight(px[2]), straight(px[1]), straight(px[0]), px[3]]);
    }
    rgba
}

/// Map CEF's cursor type onto the shared vocabulary.
///
/// The two names that matter are crossed, which is exactly the sort of thing
/// that gets miswired once and then looks "nearly right" forever: CEF's
/// `POINTER` is the ordinary arrow, while `CursorShape::Pointer` is the CSS
/// `pointer`, i.e. the link hand, which CEF calls `HAND`.
#[cfg(feature = "cef-runtime")]
pub(crate) fn from_cef(type_: cef::CursorType) -> CursorShape {
    use cef::CursorType as C;

    // Written as comparisons rather than a match: CursorType is a newtype over
    // a #[non_exhaustive] C enum, so its variants are associated consts.
    if type_ == C::POINTER {
        CursorShape::Default
    } else if type_ == C::HAND {
        CursorShape::Pointer
    } else if type_ == C::IBEAM || type_ == C::VERTICALTEXT {
        CursorShape::Text
    } else if type_ == C::CROSS || type_ == C::CELL {
        CursorShape::Crosshair
    } else if type_ == C::WAIT {
        CursorShape::Wait
    } else if type_ == C::PROGRESS {
        CursorShape::Progress
    } else if type_ == C::HELP {
        CursorShape::Help
    } else if type_ == C::MOVE || type_ == C::MIDDLEPANNING {
        CursorShape::Move
    } else if type_ == C::NOTALLOWED || type_ == C::NODROP || type_ == C::DND_NONE {
        CursorShape::NotAllowed
    } else if type_ == C::ZOOMIN {
        CursorShape::ZoomIn
    } else if type_ == C::ZOOMOUT {
        CursorShape::ZoomOut
    } else if type_ == C::GRAB {
        CursorShape::Grab
    } else if type_ == C::GRABBING {
        CursorShape::Grabbing
    } else if type_ == C::NORTHSOUTHRESIZE
        || type_ == C::NORTHRESIZE
        || type_ == C::SOUTHRESIZE
        || type_ == C::ROWRESIZE
    {
        CursorShape::ResizeNs
    } else if type_ == C::EASTWESTRESIZE
        || type_ == C::EASTRESIZE
        || type_ == C::WESTRESIZE
        || type_ == C::COLUMNRESIZE
    {
        CursorShape::ResizeEw
    } else if type_ == C::NORTHEASTSOUTHWESTRESIZE
        || type_ == C::NORTHEASTRESIZE
        || type_ == C::SOUTHWESTRESIZE
    {
        CursorShape::ResizeNeSw
    } else if type_ == C::NORTHWESTSOUTHEASTRESIZE
        || type_ == C::NORTHWESTRESIZE
        || type_ == C::SOUTHEASTRESIZE
    {
        CursorShape::ResizeNwSe
    } else if type_ == C::NONE {
        // The shared vocabulary has no "hidden" shape. Named rather than
        // silently mapped to Default, because a host that wants to honour it
        // can, and one that does not at least sees the difference.
        CursorShape::Custom("none".into())
    } else {
        CursorShape::Custom("cef-other".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_cursor_yields_once_per_change() {
        let c = LatestCursor::default();
        assert_eq!(c.take(), None);
        c.set(CursorShape::Text);
        assert_eq!(c.take(), Some(CursorShape::Text));
        assert_eq!(c.take(), None, "a shape should only be reported once");
    }

    #[test]
    fn premultiplied_bgra_becomes_straight_rgba() {
        // Half-transparent red, opaque blue, transparent.
        let bgra = [0, 0, 128, 128, 255, 0, 0, 255, 9, 9, 9, 0];
        assert_eq!(
            bgra_premultiplied_to_rgba(&bgra),
            [255, 0, 0, 128, 0, 0, 255, 255, 0, 0, 0, 0]
        );
    }

    #[cfg(feature = "cef-runtime")]
    #[test]
    fn the_crossed_pointer_names_map_the_right_way_round() {
        // CEF POINTER is the arrow; CSS/scrying Pointer is the link hand.
        assert_eq!(from_cef(cef::CursorType::POINTER), CursorShape::Default);
        assert_eq!(from_cef(cef::CursorType::HAND), CursorShape::Pointer);
        assert_eq!(from_cef(cef::CursorType::IBEAM), CursorShape::Text);
    }
}
