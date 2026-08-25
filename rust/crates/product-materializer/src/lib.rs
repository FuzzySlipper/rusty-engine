//! Build-time-only product source materialization.
//!
//! This crate turns the manifest-declared rules and UI source lanes into
//! immutable Assembly inputs. It never participates in the generated product
//! runtime: Node, TypeScript, Vite, source maps, and authored source paths are
//! deliberately absent from [`MaterializedProduct`].

#![forbid(unsafe_code)]

mod materialize;

pub use materialize::{
    materialize_product, EngineAsset, EngineAssets, MaterializationDiagnostic,
    MaterializationError, MaterializationLimits, MaterializationToolchain, MaterializedProduct,
};
