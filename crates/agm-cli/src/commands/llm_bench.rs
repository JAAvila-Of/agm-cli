//! `agm llm-bench` — run the LLM emission compliance suite.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context as _, bail};

use agm_cli::bench::case::{load_builtin_fixtures, load_from_dir};
use agm_cli::bench::cassette::CassetteMode;
use agm_cli::bench::provider::{ChatCompletionsProvider, MessagesProvider, ProviderConfig};
use agm_cli::bench::report::{ReportFormat, render_report};
use agm_cli::bench::runner::{RunOptions, run_suite};

// ---------------------------------------------------------------------------
// CLI args (passed in from main.rs)
// ---------------------------------------------------------------------------

/// Provider kind selected on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    /// Messages-style HTTP API.
    Messages,
    /// Chat-Completions-style HTTP API.
    Chat,
}

// ---------------------------------------------------------------------------
// run()
// ---------------------------------------------------------------------------

/// Run the bench suite and return an exit code.
///
/// Exit codes (per D9):
/// - 0 = all cases passed
/// - 1 = suite ran, one or more cases failed
/// - 2 = suite errored (config, missing fixtures, `--cost-only` without pricing)
/// - 3 = missing API key OR missing endpoint env var
/// - 4 = `--live` but network unreachable
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
pub fn run(
    model: &str,
    provider_kind: ProviderKind,
    fixtures_dir: Option<&PathBuf>,
    case_glob: Option<&str>,
    format: ReportFormat,
    output: Option<&PathBuf>,
    concurrency: usize,
    max_retries: u8,
    timeout_secs: u64,
    normalize: bool,
    cassettes_dir: Option<&PathBuf>,
    record: bool,
    live: bool,
    api_key_var: Option<&str>,
    max_tokens_out: Option<u32>,
    cost_per_1k_in: Option<f64>,
    cost_per_1k_out: Option<f64>,
    cost_only: bool,
) -> i32 {
    match run_inner(
        model,
        provider_kind,
        fixtures_dir,
        case_glob,
        format,
        output,
        concurrency,
        max_retries,
        timeout_secs,
        normalize,
        cassettes_dir,
        record,
        live,
        api_key_var,
        max_tokens_out,
        cost_per_1k_in,
        cost_per_1k_out,
        cost_only,
    ) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:?}");
            2
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn run_inner(
    model: &str,
    provider_kind: ProviderKind,
    fixtures_dir: Option<&PathBuf>,
    case_glob: Option<&str>,
    format: ReportFormat,
    output: Option<&PathBuf>,
    concurrency: usize,
    max_retries: u8,
    timeout_secs: u64,
    normalize: bool,
    cassettes_dir: Option<&PathBuf>,
    record: bool,
    live: bool,
    api_key_var: Option<&str>,
    max_tokens_out: Option<u32>,
    cost_per_1k_in: Option<f64>,
    cost_per_1k_out: Option<f64>,
    cost_only: bool,
) -> anyhow::Result<i32> {
    // --cost-only requires both pricing flags
    if cost_only {
        let cin = cost_per_1k_in
            .ok_or_else(|| anyhow::anyhow!("--cost-only requires --cost-per-1k-in <f64>"))?;
        let cout = cost_per_1k_out
            .ok_or_else(|| anyhow::anyhow!("--cost-only requires --cost-per-1k-out <f64>"))?;

        // Estimate from fixture prompt sizes only (no API call)
        let cases = load_cases(fixtures_dir, case_glob)?;
        let total_prompt_chars: usize = cases.iter().map(|c| c.prompt.len()).sum();
        // Rough estimate: 1 token ≈ 4 chars
        let estimated_in_tokens = total_prompt_chars / 4;
        let estimated_out_tokens = cases.len() * 512; // conservative default
        let cost = (estimated_in_tokens as f64 / 1000.0) * cin
            + (estimated_out_tokens as f64 / 1000.0) * cout;
        println!(
            "Cost estimate ({} cases): ~{} tokens in, ~{} tokens out = ${:.4}",
            cases.len(),
            estimated_in_tokens,
            estimated_out_tokens,
            cost
        );
        return Ok(0);
    }

    // Resolve API key
    let default_key_var = match provider_kind {
        ProviderKind::Messages => "AGM_MESSAGES_KEY",
        ProviderKind::Chat => "AGM_CHAT_KEY",
    };
    let key_var = api_key_var.unwrap_or(default_key_var);
    let api_key = std::env::var(key_var).map_err(|_| {
        anyhow::anyhow!(
            "API key env var `{key_var}` is not set. \
             Set it or pass a different var name with --api-key <VAR>."
        )
    });

    // Resolve endpoint
    let default_endpoint_var = match provider_kind {
        ProviderKind::Messages => "AGM_MESSAGES_ENDPOINT",
        ProviderKind::Chat => "AGM_CHAT_ENDPOINT",
    };
    let endpoint = std::env::var(default_endpoint_var).map_err(|_| {
        anyhow::anyhow!(
            "Endpoint env var `{default_endpoint_var}` is not set. \
             Set it to the base URL for the provider API."
        )
    });

    // If we're in replay-only mode (cassettes_dir set, not --live, not --record),
    // we don't actually need live credentials — skip the key/endpoint check.
    let cassette_mode = build_cassette_mode(cassettes_dir, record, live);
    let is_replay_only = matches!(cassette_mode, CassetteMode::Replay(_)) && !live;

    let (api_key, endpoint) = if is_replay_only {
        // Use dummy values — they'll never be used in Replay mode
        (
            api_key.unwrap_or_else(|_| "replay-mode-no-key".into()),
            endpoint.unwrap_or_else(|_| "http://localhost:0".into()),
        )
    } else {
        // Propagate as exit code 3 for missing credentials
        let k = api_key.map_err(|e| {
            eprintln!("error: {e}");
            std::process::exit(3);
        });
        let e = endpoint.map_err(|err| {
            eprintln!("error: {err}");
            std::process::exit(3);
        });
        // Both are Ok at this point (or we exited)
        (k.unwrap(), e.unwrap())
    };

    let timeout = Duration::from_secs(timeout_secs);

    // Optional extra version/date header for Messages-style endpoints.
    // Read from AGM_MESSAGES_VERSION_HEADER_NAME and AGM_MESSAGES_VERSION_HEADER_VALUE.
    // If either is absent or empty, no extra header is attached.
    let version_header = {
        let name = std::env::var("AGM_MESSAGES_VERSION_HEADER_NAME")
            .ok()
            .filter(|s| !s.is_empty());
        let value = std::env::var("AGM_MESSAGES_VERSION_HEADER_VALUE")
            .ok()
            .filter(|s| !s.is_empty());
        match (name, value) {
            (Some(n), Some(v)) => Some((n, v)),
            _ => None,
        }
    };

    let config = ProviderConfig {
        model: model.to_owned(),
        api_key,
        endpoint,
        version_header,
    };

    let cases = load_cases(fixtures_dir, case_glob)?;

    let opts = RunOptions {
        normalize,
        max_retries,
        timeout_secs,
        cassette_mode,
        concurrency,
        cost_per_1k_in,
        cost_per_1k_out,
        max_tokens_out,
    };

    let report = match provider_kind {
        ProviderKind::Messages => {
            let provider = MessagesProvider::new(config, timeout)
                .context("failed to create Messages provider")?;
            run_suite(&cases, &provider, &opts)
        }
        ProviderKind::Chat => {
            let provider = ChatCompletionsProvider::new(config, timeout)
                .context("failed to create Chat provider")?;
            run_suite(&cases, &provider, &opts)
        }
    };

    // Render output
    let rendered = render_report(&report, format).context("failed to render report")?;

    match output {
        Some(path) => {
            std::fs::write(path, &rendered)
                .with_context(|| format!("failed to write report to {}", path.display()))?;
        }
        None => print!("{rendered}"),
    }

    // Determine exit code
    use agm_cli::bench::report::ComplianceBucket;

    let failures = report
        .results
        .iter()
        .filter(|r| {
            !matches!(
                r.bucket,
                ComplianceBucket::Pass | ComplianceBucket::NormalizeFixable
            )
        })
        .count();

    if report
        .results
        .iter()
        .any(|r| r.bucket == ComplianceBucket::Error)
        && failures == report.total
    {
        // All cases errored — treat as suite error
        Ok(2)
    } else if failures > 0 {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn load_cases(
    fixtures_dir: Option<&PathBuf>,
    case_glob: Option<&str>,
) -> anyhow::Result<Vec<agm_cli::bench::case::BenchCase>> {
    let mut cases = match fixtures_dir {
        Some(dir) => load_from_dir(dir)
            .with_context(|| format!("failed to load fixtures from {}", dir.display()))?,
        None => load_builtin_fixtures(),
    };

    // Apply --case glob filter
    if let Some(glob) = case_glob {
        cases.retain(|c| glob_matches(glob, &c.id));
        if cases.is_empty() {
            bail!("no cases matched the pattern {glob:?}");
        }
    }

    Ok(cases)
}

/// Simple glob matching: `*` matches any sequence not containing `/`.
/// `*/a` matches `ticket/a`, `workflow/a`, etc.
/// `ticket/*` matches `ticket/a`, `ticket/b`, etc.
fn glob_matches(pattern: &str, id: &str) -> bool {
    // Build regex from glob pattern
    let re_str = pattern
        .split('*')
        .map(regex::escape)
        .collect::<Vec<_>>()
        .join("[^/]*");
    let re_str = format!("^{re_str}$");
    regex::Regex::new(&re_str)
        .map(|re| re.is_match(id))
        .unwrap_or(false)
}

fn build_cassette_mode(cassettes_dir: Option<&PathBuf>, record: bool, live: bool) -> CassetteMode {
    match cassettes_dir {
        None => CassetteMode::Disabled,
        Some(dir) => {
            if live {
                CassetteMode::Disabled
            } else if record {
                CassetteMode::Record(dir.clone())
            } else {
                CassetteMode::Replay(dir.clone())
            }
        }
    }
}
