//! Library surface for `agm-cli` — exposes internal modules for integration
//! tests and external tooling.
//!
//! The binary entry-point is `src/main.rs`. This file provides a `lib` target
//! so that integration tests under `tests/` can import runtime types directly.

pub mod corpora;
pub mod runtime;
