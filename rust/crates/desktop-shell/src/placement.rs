//! The window's size, position and maximized state, kept between runs in one
//! small file under the product's persistence root.
//!
//! The file is one line: `width height x y maximized`, with `-` for a position
//! the platform does not report (Wayland). `x y` is the client area's
//! top-left: the platforms place a new window's frame there instead, and
//! winit's frame position is unreliable where the window manager does not
//! reparent (KWin's Xwayland reports none), so [`Settle`] moves the frame
//! once the client area has landed. A missing or unreadable file opens the
//! window at its default size.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes};

/// Smallest and largest restored side, in physical pixels.
const SIDE: std::ops::RangeInclusive<u32> = 200..=16384;
/// The window manager places a new window within this; a later move is the
/// user's.
const PLACING: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placement {
    size: PhysicalSize<u32>,
    position: Option<PhysicalPosition<i32>>,
    maximized: bool,
}

impl Placement {
    pub(crate) fn load(path: &Path) -> Option<Self> {
        Self::parse(&fs::read_to_string(path).ok()?)
    }

    fn parse(text: &str) -> Option<Self> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let [width, height, x, y, maximized] = words.as_slice() else {
            return None;
        };
        let side = |value: &str| value.parse().ok().filter(|side| SIDE.contains(side));
        let position = match (*x, *y) {
            ("-", "-") => None,
            (x, y) => Some(PhysicalPosition::new(x.parse().ok()?, y.parse().ok()?)),
        };
        Some(Self {
            size: PhysicalSize::new(side(width)?, side(height)?),
            position,
            maximized: match *maximized {
                "1" => true,
                "0" => false,
                _ => return None,
            },
        })
    }

    pub(crate) fn of(window: &Window) -> Self {
        Self {
            size: window.inner_size(),
            position: window.inner_position().ok(),
            maximized: window.is_maximized(),
        }
    }

    /// Writes the placement, replacing the previous file whole.
    pub(crate) fn save(&self, path: &Path) -> Result<(), String> {
        let (x, y) = self.position.map_or_else(
            || ("-".to_owned(), "-".to_owned()),
            |position| (position.x.to_string(), position.y.to_string()),
        );
        let text = format!(
            "{} {} {x} {y} {}\n",
            self.size.width,
            self.size.height,
            u8::from(self.maximized)
        );
        let partial = path.with_extension("partial");
        fs::write(&partial, text).map_err(|error| error.to_string())?;
        fs::rename(&partial, path).map_err(|error| error.to_string())
    }

    /// The window opens at this size, and at this position when it still
    /// falls on a connected monitor. The returned [`Settle`] finishes the
    /// position once the window exists.
    pub(crate) fn apply(
        &self,
        attributes: WindowAttributes,
        event_loop: &ActiveEventLoop,
    ) -> (WindowAttributes, Settle) {
        let attributes = attributes
            .with_inner_size(self.size)
            .with_maximized(self.maximized);
        match self.position {
            Some(position) if !self.maximized && on_a_monitor(position, event_loop) => (
                attributes.with_position(position),
                Settle(Some((position, Instant::now()))),
            ),
            _ => (attributes, Settle(None)),
        }
    }
}

/// A restored client-area position the window has not reached yet.
#[derive(Debug, Default)]
pub(crate) struct Settle(Option<(PhysicalPosition<i32>, Instant)>);

impl Settle {
    /// Call after creating the window and on each move. When the client area
    /// sits off the saved position by the frame the platform added, move the
    /// window back by as much, once. The first move, or [`PLACING`], ends it.
    pub(crate) fn check(&mut self, window: &Window, moved: bool) {
        let Some((wanted, since)) = self.0 else {
            return;
        };
        let Ok(actual) = window.inner_position() else {
            self.0 = None;
            return;
        };
        if actual != wanted {
            // The window was asked for `wanted` as its frame position and its
            // client area landed at `actual`; asking for the frame that far
            // back puts the client area at `wanted`.
            window.set_outer_position(PhysicalPosition::new(
                2 * wanted.x - actual.x,
                2 * wanted.y - actual.y,
            ));
            self.0 = None;
        } else if moved || since.elapsed() > PLACING {
            self.0 = None;
        }
    }
}

fn on_a_monitor(position: PhysicalPosition<i32>, event_loop: &ActiveEventLoop) -> bool {
    event_loop.available_monitors().any(|monitor| {
        let origin = monitor.position();
        let size = monitor.size();
        (origin.x..origin.x + size.width as i32).contains(&position.x)
            && (origin.y..origin.y + size.height as i32).contains(&position.y)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_placement_round_trips_through_its_file() {
        let path = std::env::temp_dir().join(format!("desktop-window-{}", std::process::id()));
        let placement = Placement {
            size: PhysicalSize::new(1600, 900),
            position: Some(PhysicalPosition::new(-40, 25)),
            maximized: true,
        };
        placement.save(&path).unwrap();
        assert_eq!(Placement::load(&path), Some(placement));
        let wayland = Placement {
            position: None,
            maximized: false,
            ..placement
        };
        wayland.save(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "1600 900 - - 0\n");
        assert_eq!(Placement::load(&path), Some(wayland));
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn an_unusable_file_is_ignored() {
        for text in [
            "",
            "1280 720",
            "10 10 - - 0",
            "1280 720 a b 0",
            "1280 720 - - 2",
        ] {
            assert_eq!(Placement::parse(text), None, "{text:?}");
        }
    }
}
