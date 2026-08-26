//! The sole Rust-owned C ABI declaration for trusted NativeAOT products.
//!
//! C# raw declarations and its safe direct facade are generated mechanically
//! from these family modules. The table names Engine service families; it
//! intentionally has no generic invocation, target strings, capability
//! catalogue, or JSON command protocol.

mod appearance;
mod core;
mod look;
mod product;
mod rng;
mod spatial;
mod ui;

pub use appearance::*;
pub use core::*;
pub use look::*;
pub use product::*;
pub use rng::*;
pub use spatial::*;
pub use ui::*;
