//! Opt-in continuous gameplay facts over the accepted finite-binary64 value family.
//!
//! `gameplay-standard` owns value, expression, and explicit conversion semantics. This crate
//! owns separately typed durable continuous mechanics catalog/component/service contracts. The
//! frozen exact `gameplay-mechanics` catalog, seven component kinds, codecs, and services remain
//! unchanged; callers compose both families only through one explicit EntityState registry.
#![forbid(unsafe_code)]
mod bits;
mod catalog;
mod component;
mod identity;
mod service;
mod snapshot;
pub use catalog::*;
pub use component::*;
pub use gameplay_standard::ContinuousValue;
pub use identity::*;
pub use service::*;
pub use snapshot::*;
