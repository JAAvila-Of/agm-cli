//! agm-core: parsing, validation, loading, rendering, and graph operations for AGM files.

pub mod builder;
pub mod diff;
pub mod error;
pub mod graph;
pub mod import;
pub mod ingest;
pub mod loader;
pub mod memory;
pub mod model;
pub mod normalize;
pub mod parser;
pub mod renderer;
pub mod schema;
pub mod schemas;
pub mod validator;

#[cfg(feature = "compiler")]
pub mod compiler;
