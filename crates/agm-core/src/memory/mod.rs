//! Memory model utilities (spec S28).
//!
//! This module provides validation functions for memory entries, keys, and
//! topics. It does NOT implement storage -- that is Phase 2.
//!
//! The validator module (`validator::memory`) handles node-level validation
//! during file validation. This module provides the underlying validation
//! functions that can be reused by the validator, CLI tools, and the
//! Phase 2 runtime.

pub mod schema;
pub mod store;
