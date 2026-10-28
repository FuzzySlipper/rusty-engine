//! The window's mode and its last windowed size and position, kept between
//! runs in one small file under the product's persistence root.
//!
//! The file is one line: `width height x y mode`, mode `0` windowed, `1`
//! maximized and `2` fullscreen, with `-` for a position the platform does
//! not report (Wayland) or the window never had. The size and position are
//! the window's last while windowed, so leaving maximized or fullscreen,
//! now or on a later run, restores them. `x y` is the client area's
//! top-left: the platforms place a new window's frame there instead, and
//! winit's frame position is unreliable where the window manager does not
//! reparent (KWin's Xwayland reports none), so [`Settle`] moves the frame
//! once the client area has landed. A missing or unreadable file opens the
//! window as the product asks.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use render_model::WindowMode;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event_loop::ActiveEventLoop;
use winit::window::{Fullscreen, Window, WindowAttributes};

/// Smallest and largest restored side, in physical pixels.
const SIDE: std::ops::RangeInclusive<u32> = 200..=16384;
/// The window manager places a new window within this; a later move is the
/// user's.
const PLACING: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placement {
    /// The windowed size and position.
    pub(crate) size: PhysicalSize<u32>,
    pub(crate) position: Option<PhysicalPosition<i32>>,
    pub(crate) mode: WindowMode,
}

impl Placement {
    pub(crate) fn load(path: &Path) -> Option<Self> {
        Self::parse(&fs::read_to_string(path).ok()?)
    }

    fn parse(text: &str) -> Option<Self> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let [width, height, x, y, mode] = words.as_slice() else {
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
            mode: match *mode {
                "0" => WindowMode::Windowed,
                "1" => WindowMode::Maximized,
                "2" => WindowMode::Fullscreen,
                _ => return None,
            },
        })
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
            match self.mode {
                WindowMode::Windowed => 0,
                WindowMode::Maximized => 1,
                WindowMode::Fullscreen => 2,
            }
        );
        let partial = path.with_extension("partial");
        fs::write(&partial, text).map_err(|error| error.to_string())?;
        fs::rename(&partial, path).map_err(|error| error.to_string())
    }

    /// The window opens in this mode, restoring to this size, and windowed at
    /// this position when it still falls on a connected monitor. The
    /// returned [`Settle`] finishes the position once the window exists.
    pub(crate) fn apply(
        &self,
        attributes: WindowAttributes,
        event_loop: &ActiveEventLoop,
    ) -> (WindowAttributes, Settle) {
        let attributes = attributes
            .with_inner_size(self.size)
            .with_maximized(self.mode == WindowMode::Maximized)
            .with_fullscreen(
                (self.mode == WindowMode::Fullscreen).then_some(Fullscreen::Borderless(None)),
            );
        match self.position {
            Some(position)
                if self.mode == WindowMode::Windowed && on_a_monitor(position, event_loop) =>
            {
                (
                    attributes.with_position(position),
                    Settle(Some((position, Instant::now()))),
                )
            }
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

/// How `window` shows now.
pub(crate) fn mode_of(window: &Window) -> WindowMode {
    if window.fullscreen().is_some() {
        WindowMode::Fullscreen
    } else if window.is_maximized() {
        WindowMode::Maximized
    } else {
        WindowMode::Windowed
    }
}

/// Whether `window`, at `size`, is minimized: the platform says so, or it
/// has no area (Windows reports a minimized window as 0x0).
pub(crate) fn minimized(window: &Window, size: PhysicalSize<u32>) -> bool {
    window.is_minimized() == Some(true) || size.width == 0 || size.height == 0
}

/// Shows `window` in `mode`; leaving maximized or fullscreen restores
/// `windowed`.
pub(crate) fn show(window: &Window, mode: WindowMode, windowed: PhysicalSize<u32>) {
    match mode {
        WindowMode::Fullscreen => window.set_fullscreen(Some(Fullscreen::Borderless(None))),
        WindowMode::Maximized => {
            window.set_fullscreen(None);
            window.set_maximized(true);
        }
        WindowMode::Windowed => {
            window.set_fullscreen(None);
            window.set_maximized(false);
            let _ = window.request_inner_size(windowed);
        }
    }
}

/// The size a window opens windowed at with none remembered: `wanted`, kept
/// within the monitor, else three quarters of the monitor each way. `None`
/// where no monitor is known, so the platform chooses.
pub(crate) fn first_windowed_size(
    wanted: Option<(u32, u32)>,
    event_loop: &ActiveEventLoop,
) -> Option<PhysicalSize<u32>> {
    let monitor = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())
        .map(|monitor| monitor.size());
    match (wanted, monitor) {
        (Some((width, height)), Some(monitor)) => Some(PhysicalSize::new(
            width.min(monitor.width),
            height.min(monitor.height),
        )),
        (Some((width, height)), None) => Some(PhysicalSize::new(width, height)),
        (None, Some(monitor)) => Some(PhysicalSize::new(
            monitor.width * 3 / 4,
            monitor.height * 3 / 4,
        )),
        (None, None) => None,
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
            mode: WindowMode::Fullscreen,
        };
        placement.save(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "1600 900 -40 25 2\n");
        assert_eq!(Placement::load(&path), Some(placement));
        let wayland = Placement {
            position: None,
            mode: WindowMode::Windowed,
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
            "1280 720 - - 3",
        ] {
            assert_eq!(Placement::parse(text), None, "{text:?}");
        }
    }
}
