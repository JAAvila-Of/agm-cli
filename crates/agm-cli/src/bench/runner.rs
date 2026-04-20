//! Suite runner — executes bench cases using a provider and aggregates results.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use agm_core::model::fields::NodeType;
use agm_core::model::schema::EnforcementLevel;
use agm_core::normalize::{NormalizeConfig, normalize_text};
use agm_core::schemas::{SchemaOptions, schema_for};
use agm_core::validator;

use super::case::BenchCase;
use super::cassette::{CassetteMode, cassette_key, cassette_path, read_cassette};
use super::provider::{Provider, ProviderRequestOpts};
use super::report::{BenchReport, CaseResult, ComplianceBucket};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options for `run_suite`.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Apply `agm_core::normalize` before evaluating.
    pub normalize: bool,
    /// Maximum retries on transient provider errors.
    pub max_retries: u8,
    /// Per-request timeout in seconds.
    pub timeout_secs: u64,
    /// Cassette mode.
    pub cassette_mode: CassetteMode,
    /// Maximum concurrent threads.
    pub concurrency: usize,
    /// Cost per 1k input tokens (USD). `None` = omit cost.
    pub cost_per_1k_in: Option<f64>,
    /// Cost per 1k output tokens (USD). `None` = omit cost.
    pub cost_per_1k_out: Option<f64>,
    /// Max output tokens hint to pass to the provider.
    pub max_tokens_out: Option<u32>,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            normalize: false,
            max_retries: 2,
            timeout_secs: 60,
            cassette_mode: CassetteMode::Disabled,
            concurrency: 2,
            cost_per_1k_in: None,
            cost_per_1k_out: None,
            max_tokens_out: None,
        }
    }
}

// ---------------------------------------------------------------------------
// run_suite
// ---------------------------------------------------------------------------

/// Run all bench cases and return an aggregated `BenchReport`.
///
/// Uses scoped threads for concurrency; never exceeds `opts.concurrency`
/// concurrent in-flight requests.
pub fn run_suite(cases: &[BenchCase], provider: &dyn Provider, opts: &RunOptions) -> BenchReport {
    let results: Arc<Mutex<Vec<CaseResult>>> = Arc::new(Mutex::new(Vec::new()));
    let schema_pass: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let validate_pass: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    let semaphore = Arc::new(Semaphore::new(opts.concurrency));
    let timeout = Duration::from_secs(opts.timeout_secs);

    std::thread::scope(|scope| {
        for case in cases {
            let results = Arc::clone(&results);
            let schema_pass = Arc::clone(&schema_pass);
            let validate_pass = Arc::clone(&validate_pass);
            let semaphore: Arc<Semaphore> = Arc::clone(&semaphore);

            scope.spawn(move || {
                let _permit = semaphore.acquire();
                let result = run_case(case, provider, opts, timeout);

                // Tally schema / validate passes
                match result.bucket {
                    ComplianceBucket::Pass
                    | ComplianceBucket::NormalizeFixable
                    | ComplianceBucket::SchemaOnly => {
                        *schema_pass.lock().unwrap() += 1;
                    }
                    _ => {}
                }
                match result.bucket {
                    ComplianceBucket::Pass
                    | ComplianceBucket::NormalizeFixable
                    | ComplianceBucket::ValidateOnly => {
                        *validate_pass.lock().unwrap() += 1;
                    }
                    _ => {}
                }

                results.lock().unwrap().push(result);
            });
        }
    });

    let mut results = Arc::try_unwrap(results)
        .expect("all threads finished")
        .into_inner()
        .unwrap();

    // Sort by case_id for deterministic output
    results.sort_by(|a, b| a.case_id.cmp(&b.case_id));

    let sp = *schema_pass.lock().unwrap();
    let vp = *validate_pass.lock().unwrap();

    BenchReport::from_results(
        provider.name().to_owned(),
        provider.model().to_owned(),
        results,
        sp,
        vp,
        opts.cost_per_1k_in,
        opts.cost_per_1k_out,
    )
}

// ---------------------------------------------------------------------------
// Per-case evaluation
// ---------------------------------------------------------------------------

fn run_case(
    case: &BenchCase,
    provider: &dyn Provider,
    opts: &RunOptions,
    timeout: Duration,
) -> CaseResult {
    let req_opts = ProviderRequestOpts {
        max_tokens_out: opts.max_tokens_out.or(Some(4096)),
        timeout,
        tool_schema: build_tool_schema(&case.expectation.expected_type),
        ..Default::default()
    };

    // Fetch response — cassette or live
    let resp = fetch_with_cassette(case, provider, &req_opts, &opts.cassette_mode);

    let resp = match resp {
        Ok(r) => r,
        Err(e) => {
            return CaseResult {
                case_id: case.id.clone(),
                bucket: ComplianceBucket::Error,
                tokens_in: 0,
                tokens_out: 0,
                latency_ms: 0,
                notes: vec![format!("provider error: {e}")],
            };
        }
    };

    let raw_text = resp.raw_text.clone();
    let tokens_in = resp.tokens_in;
    let tokens_out = resp.tokens_out;
    let latency_ms = resp.latency_ms;

    // Parse + evaluate in a single pass (D10: one pass, normalize if needed)
    evaluate_response(
        case,
        &raw_text,
        tokens_in,
        tokens_out,
        latency_ms,
        opts.normalize,
    )
}

fn fetch_with_cassette(
    case: &BenchCase,
    provider: &dyn Provider,
    opts: &ProviderRequestOpts,
    mode: &CassetteMode,
) -> Result<super::provider::ProviderResponse, super::provider::ProviderError> {
    match mode {
        CassetteMode::Disabled => provider.send(&case.prompt, opts),

        CassetteMode::Replay(dir) => {
            let path = cassette_path(dir, provider.name(), provider.model(), &case.id);
            let hash = cassette_key(provider.name(), provider.model(), &case.prompt);
            match read_cassette(&path, &hash) {
                Ok(Some(env)) => Ok(super::provider::ProviderResponse {
                    raw_text: env.response_text,
                    tokens_in: env.tokens_in,
                    tokens_out: env.tokens_out,
                    latency_ms: env.latency_ms,
                }),
                Ok(None) => Err(super::provider::ProviderError::CassetteMiss),
                Err(e) => Err(super::provider::ProviderError::ParseResponse(e.to_string())),
            }
        }

        CassetteMode::Record(dir) => {
            let resp = provider.send(&case.prompt, opts)?;
            let path = cassette_path(dir, provider.name(), provider.model(), &case.id);
            let hash = cassette_key(provider.name(), provider.model(), &case.prompt);
            let env = super::cassette::CassetteEnvelope {
                request_hash: hash,
                prompt_preview: case.prompt.chars().take(120).collect(),
                response_text: resp.raw_text.clone(),
                tokens_in: resp.tokens_in,
                tokens_out: resp.tokens_out,
                latency_ms: resp.latency_ms,
                recorded_at: "2026-04-20T00:00:00Z".into(),
            };
            if let Err(e) = super::cassette::write_cassette(&path, &env) {
                eprintln!("warn: failed to write cassette: {e}");
            }
            Ok(resp)
        }

        CassetteMode::RecordOrReplay(dir) => {
            let path = cassette_path(dir, provider.name(), provider.model(), &case.id);
            let hash = cassette_key(provider.name(), provider.model(), &case.prompt);
            if let Ok(Some(env)) = read_cassette(&path, &hash) {
                return Ok(super::provider::ProviderResponse {
                    raw_text: env.response_text,
                    tokens_in: env.tokens_in,
                    tokens_out: env.tokens_out,
                    latency_ms: env.latency_ms,
                });
            }
            let resp = provider.send(&case.prompt, opts)?;
            let env = super::cassette::CassetteEnvelope {
                request_hash: hash,
                prompt_preview: case.prompt.chars().take(120).collect(),
                response_text: resp.raw_text.clone(),
                tokens_in: resp.tokens_in,
                tokens_out: resp.tokens_out,
                latency_ms: resp.latency_ms,
                recorded_at: "2026-04-20T00:00:00Z".into(),
            };
            if let Err(e) = super::cassette::write_cassette(&path, &env) {
                eprintln!("warn: failed to write cassette: {e}");
            }
            Ok(resp)
        }
    }
}

fn evaluate_response(
    case: &BenchCase,
    raw_text: &str,
    tokens_in: u32,
    tokens_out: u32,
    latency_ms: u64,
    try_normalize: bool,
) -> CaseResult {
    let mut notes: Vec<String> = Vec::new();

    // --- Parse ---
    let parse_result = agm_core::parser::parse(raw_text);
    let agm_file = match parse_result {
        Ok(f) => f,
        Err(errs) => {
            // Try normalize path if requested
            if try_normalize {
                let cfg = NormalizeConfig::default();
                if let Ok((normalized, norm_report)) = normalize_text(raw_text, &cfg) {
                    if !norm_report.rewrites.is_empty() {
                        if let Ok(f2) = agm_core::parser::parse(&normalized) {
                            let rewrite_count = norm_report.rewrites.len();
                            notes.push(format!("normalized {rewrite_count} field(s)"));
                            return evaluate_parsed(
                                case,
                                &f2,
                                &normalized,
                                ParsedEvalContext {
                                    tokens_in,
                                    tokens_out,
                                    latency_ms,
                                    notes,
                                    was_normalized: true,
                                },
                            );
                        }
                    }
                }
            }
            let err_msg: String = errs
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            return CaseResult {
                case_id: case.id.clone(),
                bucket: ComplianceBucket::Error,
                tokens_in,
                tokens_out,
                latency_ms,
                notes: vec![format!("parse error: {err_msg}")],
            };
        }
    };

    // --- Evaluate raw ---
    evaluate_parsed(
        case,
        &agm_file,
        raw_text,
        ParsedEvalContext {
            tokens_in,
            tokens_out,
            latency_ms,
            notes,
            was_normalized: false,
        },
    )
}

struct ParsedEvalContext {
    tokens_in: u32,
    tokens_out: u32,
    latency_ms: u64,
    notes: Vec<String>,
    was_normalized: bool,
}

fn evaluate_parsed(
    case: &BenchCase,
    agm_file: &agm_core::model::AgmFile,
    source_text: &str,
    ctx: ParsedEvalContext,
) -> CaseResult {
    let ParsedEvalContext {
        tokens_in,
        tokens_out,
        latency_ms,
        mut notes,
        was_normalized,
    } = ctx;
    let validate_opts = agm_core::validator::ValidateOptions {
        enforcement_level: EnforcementLevel::Standard,
        ..Default::default()
    };
    let diagnostics = validator::validate(agm_file, source_text, "<bench>", &validate_opts);
    let validate_ok = !diagnostics.has_errors();
    if !validate_ok {
        for err in diagnostics.diagnostics().iter().filter(|e| e.is_error()) {
            notes.push(format!("validate: {}", err.code));
        }
    }

    // Schema check
    let mut schema_ok = false;
    if let Some(node) = agm_file.nodes.first() {
        let schema_opts = SchemaOptions::default();
        if let Ok(schema_val) = schema_for(&node.node_type, &schema_opts) {
            schema_ok = check_schema(&node.node_type, &schema_val, node);
        }
    }

    // Required fields check
    let fields_ok = check_required_fields(case, agm_file);
    if !fields_ok {
        notes.push("missing required fields".into());
    }

    // Type check
    let type_ok = check_expected_type(case, agm_file);
    if !type_ok {
        notes.push("wrong node type".into());
    }

    // Now try normalize path if raw parse succeeded but validate failed and normalize not yet tried
    // This implements D10: single pass, both buckets computed together.
    let bucket = if validate_ok && schema_ok && fields_ok && type_ok {
        if was_normalized {
            ComplianceBucket::NormalizeFixable
        } else {
            ComplianceBucket::Pass
        }
    } else if !validate_ok && schema_ok {
        ComplianceBucket::SchemaOnly
    } else if validate_ok && !schema_ok {
        ComplianceBucket::ValidateOnly
    } else {
        ComplianceBucket::Fail
    };

    CaseResult {
        case_id: case.id.clone(),
        bucket,
        tokens_in,
        tokens_out,
        latency_ms,
        notes,
    }
}

fn check_schema(
    _node_type: &NodeType,
    schema_val: &serde_json::Value,
    node: &agm_core::model::Node,
) -> bool {
    // Convert node to JSON for schema validation
    let node_json = match serde_json::to_value(node) {
        Ok(v) => v,
        Err(_) => return false,
    };
    // Use jsonschema crate via agm-core's dependency
    jsonschema::validate(schema_val, &node_json).is_ok()
}

fn check_required_fields(case: &BenchCase, agm_file: &agm_core::model::AgmFile) -> bool {
    if case.expectation.required_fields.is_empty() {
        return true;
    }
    let node = match agm_file.nodes.first() {
        Some(n) => n,
        None => return false,
    };
    // Build a JSON representation for field checking
    let json = match serde_json::to_value(node) {
        Ok(v) => v,
        Err(_) => return false,
    };
    for (field, expected_val) in &case.expectation.required_fields {
        match json.get(field) {
            None => return false,
            Some(v) => {
                if let Some(expected) = expected_val {
                    if v.as_str() != Some(expected.as_str()) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

fn check_expected_type(case: &BenchCase, agm_file: &agm_core::model::AgmFile) -> bool {
    let expected = match &case.expectation.expected_type {
        Some(t) => t,
        None => return true,
    };
    let node = match agm_file.nodes.first() {
        Some(n) => n,
        None => return false,
    };
    node.node_type.to_string() == *expected
}

fn build_tool_schema(expected_type: &Option<String>) -> Option<serde_json::Value> {
    let type_str = expected_type.as_deref()?;
    let node_type: NodeType = type_str.parse().ok()?;
    let opts = SchemaOptions::default();
    schema_for(&node_type, &opts).ok()
}

// ---------------------------------------------------------------------------
// Simple semaphore for concurrency gating
// ---------------------------------------------------------------------------

struct Semaphore {
    inner: Mutex<usize>,
    cvar: std::sync::Condvar,
}

impl Semaphore {
    fn new(n: usize) -> Self {
        Self {
            inner: Mutex::new(n),
            cvar: std::sync::Condvar::new(),
        }
    }

    fn acquire(&self) -> SemaphorePermit<'_> {
        let mut count = self.inner.lock().unwrap();
        while *count == 0 {
            count = self.cvar.wait(count).unwrap();
        }
        *count -= 1;
        SemaphorePermit { sem: self }
    }
}

struct SemaphorePermit<'a> {
    sem: &'a Semaphore,
}

impl Drop for SemaphorePermit<'_> {
    fn drop(&mut self) {
        *self.sem.inner.lock().unwrap() += 1;
        self.sem.cvar.notify_one();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::case::BenchExpectation;
    use crate::bench::provider::{ProviderError, ProviderResponse};

    struct StubProvider {
        responses: Vec<Result<ProviderResponse, ProviderError>>,
        idx: Mutex<usize>,
    }

    impl StubProvider {
        fn new(responses: Vec<Result<ProviderResponse, ProviderError>>) -> Self {
            Self {
                responses,
                idx: Mutex::new(0),
            }
        }
    }

    impl Provider for StubProvider {
        fn name(&self) -> &'static str {
            "messages"
        }
        fn model(&self) -> &str {
            "demo-m1"
        }
        fn send(
            &self,
            _prompt: &str,
            _opts: &ProviderRequestOpts,
        ) -> Result<ProviderResponse, ProviderError> {
            let mut idx = self.idx.lock().unwrap();
            let resp = self.responses[*idx % self.responses.len()].clone();
            *idx += 1;
            resp
        }
    }

    fn minimal_valid_agm(node_type: &str, node_id: &str) -> String {
        format!(
            "agm\npackage: test\nversion: 0.1.0\n\nnode {node_id}\ntype: {node_type}\nsummary: Test node\n"
        )
    }

    #[test]
    fn test_run_suite_aggregates_correctly() {
        let ticket_agm = minimal_valid_agm("ticket", "node_a");
        let cases = vec![
            BenchCase {
                id: "ticket/a".into(),
                node_type: NodeType::Ticket,
                prompt: "create a ticket".into(),
                expectation: BenchExpectation {
                    expected_type: Some("ticket".into()),
                    must_validate: true,
                    must_match_schema: false,
                    required_fields: vec![],
                },
            },
            BenchCase {
                id: "ticket/b".into(),
                node_type: NodeType::Ticket,
                prompt: "create another ticket".into(),
                expectation: BenchExpectation {
                    expected_type: Some("ticket".into()),
                    must_validate: false,
                    must_match_schema: false,
                    required_fields: vec![],
                },
            },
        ];

        let provider = StubProvider::new(vec![Ok(ProviderResponse {
            raw_text: ticket_agm,
            tokens_in: 100,
            tokens_out: 80,
            latency_ms: 500,
        })]);

        let opts = RunOptions {
            concurrency: 1,
            cassette_mode: CassetteMode::Disabled,
            ..Default::default()
        };

        let report = run_suite(&cases, &provider, &opts);
        assert_eq!(report.total, 2);
        assert!(report.compliance_rate >= 0.0);
        assert!(report.avg_tokens_in > 0.0);
    }
}
