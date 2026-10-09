//! Verifying rendered output: comparing two renders of one scene, and the
//! GPU verification lane built on it (`rusty-gpu-lane`, docs/verification.md).

pub mod capture;
pub mod font;
pub mod gallery;
pub mod image;
pub mod lane;

pub use image::{compare, heat_map, Image, ImageDiff};
