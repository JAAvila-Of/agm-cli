//! Fluent builders for constructing AGM [`Node`] values safely.
//!
//! Builders produce validated [`Node`] values that are ready to serialize or
//! write to a file. Calling `.build()` runs the same validation passes as
//! `agm validate` in [`EnforcementLevel::Standard`] mode by default.
//!
//! # Quick start
//!
//! ```rust
//! use agm_core::builder::{TicketBuilder, BuildError};
//! use agm_core::model::fields::Priority;
//!
//! let node = TicketBuilder::new("my.ticket.login")
//!     .summary("add OAuth2 login")
//!     .title("Add OAuth2 login flow")
//!     .description("Add Google OAuth2 login to the dashboard.")
//!     .priority(Priority::High)
//!     .build()
//!     .expect("valid ticket");
//!
//! let agm_text = node.render_canonical();
//! assert!(agm_text.contains("node my.ticket.login"));
//! ```
//!
//! # Error handling
//!
//! [`BuildError::Validation`] wraps the same [`DiagnosticCollection`] produced
//! by `agm validate`, so you can inspect individual diagnostics:
//!
//! ```rust
//! use agm_core::builder::{TicketBuilder, BuildError};
//!
//! match TicketBuilder::new("bad id!").build() {
//!     Ok(_) => panic!("expected error"),
//!     Err(BuildError::Validation(dc)) => {
//!         for d in dc.diagnostics() {
//!             eprintln!("{} {}: {}", d.code, d.severity, d.message);
//!         }
//!     }
//!     Err(BuildError::Precondition(msg)) => eprintln!("precondition: {msg}"),
//! }
//! ```
//!
//! # Setter naming convention
//!
//! All setters follow `fn <field>(self, value) -> Self` — no `with_` prefix.
//! Multi-value setters (e.g. `.labels(iter)`) accept any `IntoIterator`.
//!
//! [`Node`]: crate::model::node::Node
//! [`EnforcementLevel::Standard`]: crate::model::schema::EnforcementLevel
//! [`DiagnosticCollection`]: crate::error::diagnostic::DiagnosticCollection

pub mod code_block;
pub mod common;
pub mod decision;
pub mod error;
pub mod facts;
pub mod memory_entry;
pub mod orchestration;
pub mod rules;
pub mod ticket;
pub mod verify_check;
pub mod workflow;

pub use code_block::CodeBlockBuilder;
pub use decision::DecisionBuilder;
pub use error::BuildError;
pub use facts::FactsBuilder;
pub use memory_entry::MemoryEntryBuilder;
pub use orchestration::OrchestrationBuilder;
pub use rules::RulesBuilder;
pub use ticket::TicketBuilder;
pub use verify_check::VerifyCheckBuilder;
pub use workflow::WorkflowBuilder;
