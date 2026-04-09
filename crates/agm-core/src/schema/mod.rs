//! Type schema registry and enforcement (spec §14).

pub mod enforcement;
pub mod registry;

pub use enforcement::validate_schema;
pub use registry::get_schema;
