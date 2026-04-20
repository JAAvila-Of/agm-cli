//! Benchmark report types and formatters.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// How a single bench case was judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplianceBucket {
    /// Response passed all checks without any normalization.
    Pass,
    /// Response failed raw checks but passed after `normalize`.
    NormalizeFixable,
    /// Response matches schema but fails `agm validate`.
    SchemaOnly,
    /// Response passes `agm validate` but does not match schema.
    ValidateOnly,
    /// Response failed all checks.
    Fail,
    /// Provider or parse error; response could not be evaluated.
    Error,
}

impl ComplianceBucket {
    /// Returns `true` for outcomes that count toward "compliance" (Pass + NormalizeFixable).
    #[must_use]
    pub fn is_compliant(&self) -> bool {
        matches!(self, Self::Pass | Self::NormalizeFixable)
    }
}

impl std::fmt::Display for ComplianceBucket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pass => write!(f, "PASS"),
            Self::NormalizeFixable => write!(f, "NORM_FIXABLE"),
            Self::SchemaOnly => write!(f, "SCHEMA_ONLY"),
            Self::ValidateOnly => write!(f, "VALIDATE_ONLY"),
            Self::Fail => write!(f, "FAIL"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

/// Result for a single bench case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    /// Case identifier, e.g. `"ticket/a"`.
    pub case_id: String,
    /// How this case was judged.
    pub bucket: ComplianceBucket,
    /// Input tokens consumed.
    pub tokens_in: u32,
    /// Output tokens produced.
    pub tokens_out: u32,
    /// Round-trip latency in milliseconds.
    pub latency_ms: u64,
    /// Human-readable notes (normalization details, validation errors, etc.).
    pub notes: Vec<String>,
}

/// Aggregated benchmark report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchReport {
    /// Provider name (`"messages"` or `"chat"`).
    pub provider: String,
    /// Model identifier string.
    pub model: String,
    /// Total number of cases run.
    pub total: usize,
    /// Fraction of cases with `Pass` bucket.
    pub compliance_rate: f64,
    /// Fraction of cases that are `Pass` or `NormalizeFixable`.
    pub normalize_fixable_rate: f64,
    /// Fraction of cases that passed JSON Schema validation.
    pub schema_rate: f64,
    /// Fraction of cases that passed `agm validate`.
    pub validate_rate: f64,
    /// Average input tokens across all cases.
    pub avg_tokens_in: f64,
    /// Average output tokens across all cases.
    pub avg_tokens_out: f64,
    /// p50 latency in milliseconds.
    pub p50_latency_ms: u64,
    /// p95 latency in milliseconds.
    pub p95_latency_ms: u64,
    /// Estimated cost in USD, or `None` if pricing flags were not supplied.
    pub estimated_cost_usd: Option<f64>,
    /// Per-case results.
    pub results: Vec<CaseResult>,
}

impl BenchReport {
    /// Compute a `BenchReport` from a list of case results and aggregate metrics.
    #[must_use]
    pub fn from_results(
        provider: String,
        model: String,
        results: Vec<CaseResult>,
        schema_pass_count: usize,
        validate_pass_count: usize,
        cost_per_1k_in: Option<f64>,
        cost_per_1k_out: Option<f64>,
    ) -> Self {
        let total = results.len();

        let compliant = results
            .iter()
            .filter(|r| r.bucket == ComplianceBucket::Pass)
            .count();
        let norm_fixable = results.iter().filter(|r| r.bucket.is_compliant()).count();

        let compliance_rate = if total == 0 {
            0.0
        } else {
            compliant as f64 / total as f64
        };
        let normalize_fixable_rate = if total == 0 {
            0.0
        } else {
            norm_fixable as f64 / total as f64
        };
        let schema_rate = if total == 0 {
            0.0
        } else {
            schema_pass_count as f64 / total as f64
        };
        let validate_rate = if total == 0 {
            0.0
        } else {
            validate_pass_count as f64 / total as f64
        };

        let avg_tokens_in = if total == 0 {
            0.0
        } else {
            results.iter().map(|r| r.tokens_in as f64).sum::<f64>() / total as f64
        };
        let avg_tokens_out = if total == 0 {
            0.0
        } else {
            results.iter().map(|r| r.tokens_out as f64).sum::<f64>() / total as f64
        };

        let mut latencies: Vec<u64> = results.iter().map(|r| r.latency_ms).collect();
        latencies.sort_unstable();
        let p50_latency_ms = percentile(&latencies, 50);
        let p95_latency_ms = percentile(&latencies, 95);

        let estimated_cost_usd = match (cost_per_1k_in, cost_per_1k_out) {
            (Some(cin), Some(cout)) => {
                let total_in: f64 = results.iter().map(|r| r.tokens_in as f64).sum();
                let total_out: f64 = results.iter().map(|r| r.tokens_out as f64).sum();
                Some(total_in / 1000.0 * cin + total_out / 1000.0 * cout)
            }
            _ => None,
        };

        Self {
            provider,
            model,
            total,
            compliance_rate,
            normalize_fixable_rate,
            schema_rate,
            validate_rate,
            avg_tokens_in,
            avg_tokens_out,
            p50_latency_ms,
            p95_latency_ms,
            estimated_cost_usd,
            results,
        }
    }
}

fn percentile(sorted: &[u64], p: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p as f64 / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

// ---------------------------------------------------------------------------
// Formatters
// ---------------------------------------------------------------------------

/// Output format for the bench report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Text,
    Markdown,
    Json,
}

/// Render a `BenchReport` to a string in the requested format.
///
/// # Errors
///
/// JSON serialization errors are propagated.
pub fn render_report(report: &BenchReport, format: ReportFormat) -> anyhow::Result<String> {
    match format {
        ReportFormat::Text => Ok(render_text(report)),
        ReportFormat::Markdown => Ok(render_markdown(report)),
        ReportFormat::Json => serde_json::to_string_pretty(report)
            .map_err(|e| anyhow::anyhow!("JSON serialization failed: {e}")),
    }
}

fn render_text(report: &BenchReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Model:  {}", report.model);
    let _ = writeln!(out, "Cases:  {}", report.total);
    let _ = writeln!(out);

    for r in &report.results {
        let _ = writeln!(
            out,
            "  {:<24}  {:<14}  ({} in, {} out, {} ms)",
            r.case_id,
            r.bucket.to_string(),
            r.tokens_in,
            r.tokens_out,
            r.latency_ms
        );
        for note in &r.notes {
            let _ = writeln!(out, "    note: {note}");
        }
    }

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Compliance:        {}/{} ({:.1}%)",
        (report.compliance_rate * report.total as f64).round() as usize,
        report.total,
        report.compliance_rate * 100.0
    );
    let _ = writeln!(
        out,
        "Normalize-fixable: {}/{} ({:.1}%)",
        (report.normalize_fixable_rate * report.total as f64).round() as usize,
        report.total,
        report.normalize_fixable_rate * 100.0
    );
    let _ = writeln!(
        out,
        "Schema match:      {}/{} ({:.1}%)",
        (report.schema_rate * report.total as f64).round() as usize,
        report.total,
        report.schema_rate * 100.0
    );
    let _ = writeln!(
        out,
        "Validate:          {}/{} ({:.1}%)",
        (report.validate_rate * report.total as f64).round() as usize,
        report.total,
        report.validate_rate * 100.0
    );
    let _ = writeln!(out, "Avg tokens in:     {:.0}", report.avg_tokens_in);
    let _ = writeln!(out, "Avg tokens out:    {:.0}", report.avg_tokens_out);
    let _ = writeln!(
        out,
        "p50 / p95 latency: {} ms / {} ms",
        report.p50_latency_ms, report.p95_latency_ms
    );
    if let Some(cost) = report.estimated_cost_usd {
        let _ = writeln!(out, "Estimated cost:    ${cost:.4}");
    }
    out
}

fn render_markdown(report: &BenchReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# AGM LLM Bench Report");
    let _ = writeln!(out);
    let _ = writeln!(out, "| Field | Value |");
    let _ = writeln!(out, "|-------|-------|");
    let _ = writeln!(out, "| Model | `{}` |", report.model);
    let _ = writeln!(out, "| Cases | {} |", report.total);
    let _ = writeln!(
        out,
        "| Compliance | {:.1}% |",
        report.compliance_rate * 100.0
    );
    let _ = writeln!(
        out,
        "| Normalize-fixable | {:.1}% |",
        report.normalize_fixable_rate * 100.0
    );
    let _ = writeln!(out, "| Schema match | {:.1}% |", report.schema_rate * 100.0);
    let _ = writeln!(out, "| Validate | {:.1}% |", report.validate_rate * 100.0);
    let _ = writeln!(out, "| Avg tokens in | {:.0} |", report.avg_tokens_in);
    let _ = writeln!(out, "| Avg tokens out | {:.0} |", report.avg_tokens_out);
    let _ = writeln!(out, "| p50 latency | {} ms |", report.p50_latency_ms);
    let _ = writeln!(out, "| p95 latency | {} ms |", report.p95_latency_ms);
    if let Some(cost) = report.estimated_cost_usd {
        let _ = writeln!(out, "| Estimated cost | ${cost:.4} |");
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "## Results by Case");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "| Case | Bucket | Tokens In | Tokens Out | Latency (ms) | Notes |"
    );
    let _ = writeln!(
        out,
        "|------|--------|-----------|------------|--------------|-------|"
    );
    for r in &report.results {
        let notes = r.notes.join("; ");
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} |",
            r.case_id, r.bucket, r.tokens_in, r.tokens_out, r.latency_ms, notes
        );
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report() -> BenchReport {
        let results = vec![
            CaseResult {
                case_id: "ticket/a".into(),
                bucket: ComplianceBucket::Pass,
                tokens_in: 200,
                tokens_out: 150,
                latency_ms: 1100,
                notes: vec![],
            },
            CaseResult {
                case_id: "ticket/b".into(),
                bucket: ComplianceBucket::NormalizeFixable,
                tokens_in: 210,
                tokens_out: 160,
                latency_ms: 1300,
                notes: vec!["normalized 1 field".into()],
            },
            CaseResult {
                case_id: "ticket/c".into(),
                bucket: ComplianceBucket::Fail,
                tokens_in: 195,
                tokens_out: 120,
                latency_ms: 900,
                notes: vec!["validate: V024 missing description".into()],
            },
        ];
        BenchReport::from_results(
            "messages".into(),
            "demo-m1".into(),
            results,
            2,
            2,
            None,
            None,
        )
    }

    #[test]
    fn test_text_formatter_snapshot() {
        let report = make_report();
        let text = render_text(&report);
        insta::assert_snapshot!("text_formatter", text);
    }

    #[test]
    fn test_markdown_formatter_snapshot() {
        let report = make_report();
        let md = render_markdown(&report);
        insta::assert_snapshot!("markdown_formatter", md);
    }

    #[test]
    fn test_json_formatter_roundtrip() {
        let report = make_report();
        let json = render_report(&report, ReportFormat::Json).unwrap();
        let parsed: BenchReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total, report.total);
        assert_eq!(parsed.results.len(), report.results.len());
        assert!((parsed.compliance_rate - report.compliance_rate).abs() < 1e-9);
    }

    #[test]
    fn test_compliance_bucket_display() {
        assert_eq!(ComplianceBucket::Pass.to_string(), "PASS");
        assert_eq!(
            ComplianceBucket::NormalizeFixable.to_string(),
            "NORM_FIXABLE"
        );
        assert_eq!(ComplianceBucket::Fail.to_string(), "FAIL");
        assert_eq!(ComplianceBucket::Error.to_string(), "ERROR");
    }

    #[test]
    fn test_cost_estimation_present() {
        let results = vec![CaseResult {
            case_id: "ticket/a".into(),
            bucket: ComplianceBucket::Pass,
            tokens_in: 1000,
            tokens_out: 1000,
            latency_ms: 500,
            notes: vec![],
        }];
        let report = BenchReport::from_results(
            "messages".into(),
            "demo-m1".into(),
            results,
            1,
            1,
            Some(3.0),
            Some(15.0),
        );
        let cost = report.estimated_cost_usd.unwrap();
        // 1000/1000 * 3.0 + 1000/1000 * 15.0 = 18.0
        assert!((cost - 18.0).abs() < 1e-9);
    }

    #[test]
    fn test_cost_estimation_absent_when_flags_missing() {
        let results = vec![CaseResult {
            case_id: "ticket/a".into(),
            bucket: ComplianceBucket::Pass,
            tokens_in: 1000,
            tokens_out: 1000,
            latency_ms: 500,
            notes: vec![],
        }];
        let report = BenchReport::from_results(
            "messages".into(),
            "demo-m1".into(),
            results,
            1,
            1,
            None,
            None,
        );
        assert!(report.estimated_cost_usd.is_none());
    }
}
