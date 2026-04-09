//! Runtime execution engine for AGM orchestration (Phase 2).

pub mod agent;
pub mod context;
pub mod memory;
pub mod scheduler;
pub mod state;
pub mod verifier;

#[allow(unused_imports)]
pub use agent::{AgentBackend, AgentRequest, AgentResponse, MockAgent, ShellAgent};
#[allow(unused_imports)]
pub use context::{BuiltContext, ContextSource};
#[allow(unused_imports)]
pub use memory::MemoryRuntime;
#[allow(unused_imports)]
pub use scheduler::{NodeResult, RunConfig, RunReport};
#[allow(unused_imports)]
pub use state::{ExecutionProgress, ExecutionTracker};
#[allow(unused_imports)]
pub use verifier::{CheckResult, VerifyResult};
