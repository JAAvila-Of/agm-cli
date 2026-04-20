//! LLM emission benchmark suite — `agm llm-bench`.
//!
//! Runs prompt fixtures against HTTP inference providers (Messages-API or
//! Chat-Completions-API shaped), parses the AGM text from each response,
//! validates it, and produces a compliance report.
//!
//! All logic lives in this crate; no benchmark concepts are added to `agm-core`.

pub mod case;
pub mod cassette;
pub mod provider;
pub mod report;
pub mod runner;

pub use case::{BenchCase, BenchExpectation};
pub use cassette::CassetteMode;
pub use provider::{Provider, ProviderConfig, ProviderResponse};
pub use report::{BenchReport, CaseResult, ComplianceBucket};
pub use runner::{RunOptions, run_suite};
