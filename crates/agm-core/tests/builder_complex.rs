//! Complex / extensive integration tests for the `agm_core::builder` API.
//!
//! Covers:
//! - Fully-populated ticket (every optional field)
//! - Workflow with every CodeAction variant and ≥5 verify checks
//! - Large workflow (20+ steps)
//! - Multi-group orchestration with cross-group `requires` DAG
//! - Multi-node AGmFile assembled from 6+ builders
//! - Edge-case strings: Unicode, multi-line description, long titles
//! - CodeBlock helper variants
//! - Double-render idempotence (round-trip semantic equality)
//! - Error paths on complex content

use std::collections::HashSet;

use agm_core::builder::{
    BuildError, CodeBlockBuilder, DecisionBuilder, FactsBuilder, OrchestrationBuilder,
    RulesBuilder, TicketBuilder, VerifyCheckBuilder, WorkflowBuilder,
};
use agm_core::error::codes::ErrorCode;
use agm_core::model::code::{CodeAction, CodeBlock};
use agm_core::model::context::{AgentContext, FileRange, LoadFile};
use agm_core::model::fields::{NodeType, Priority, SddPhase, Stability, TicketAction};
use agm_core::model::file::{AgmFile, Header};
use agm_core::model::orchestration::{ParallelGroup, Strategy};
use agm_core::model::schema::EnforcementLevel;
use agm_core::parser;
use agm_core::renderer::canonical::render_canonical;
use agm_core::validator;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parses an AGM text string and validates it at Standard level with SingleNode
/// scope, panicking on any error. Returns the parsed `AgmFile`.
///
/// SingleNode scope is used here because builder round-trips render a single
/// node; cross-node reference checks (V004, V009 node_status) require the full
/// assembled file and are therefore out of scope for render-fidelity tests.
fn reparse_and_validate(agm_text: &str) -> AgmFile {
    use agm_core::validator::{ValidateOptions, ValidationScope};
    let file = parser::parse(agm_text).expect("re-parse failed");
    let opts = ValidateOptions {
        scope: ValidationScope::SingleNode,
        ..Default::default()
    };
    let diags = validator::validate(&file, agm_text, "<test>", &opts);
    assert!(
        !diags.has_errors(),
        "re-parsed file has validation errors: {}",
        diags
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );
    file
}

/// Builds a real `AgmFile` header (not the scratch builder header).
fn test_header(package: &str) -> Header {
    Header {
        agm: "1.0".to_owned(),
        package: package.to_owned(),
        version: "1.0.0".to_owned(),
        title: None,
        owner: None,
        imports: None,
        default_load: None,
        description: None,
        tags: None,
        status: None,
        load_profiles: None,
        target_runtime: None,
    }
}

// ---------------------------------------------------------------------------
// Test 1: Fully-populated ticket — every optional field set
//
// Cross-node references (depends, related_to, agent_context.load_nodes) are
// validated against the full file's id set. In single-node mode only the node's
// own id exists, so any external references would produce V004. To exercise all
// fields we:
//   a) Build the ticket with `build_unchecked` to capture all fields.
//   b) Assemble a multi-node file containing the referenced nodes.
//   c) Validate the file — expects zero errors in Standard and Strict modes.
//   d) Snapshot the canonical node-only text.
// ---------------------------------------------------------------------------

#[test]
fn test_complex_ticket_fully_populated_roundtrip_standard_and_strict() {
    // Referenced nodes that must exist in the same file.
    let constraints_id = "myapp.ref.constraints";
    let rules_id = "myapp.ref.rules";
    let session_id = "myapp.ref.session";

    let agent_ctx = AgentContext {
        load_nodes: Some(vec![constraints_id.to_owned(), rules_id.to_owned()]),
        load_files: Some(vec![LoadFile {
            path: "src/auth.rs".to_owned(),
            range: FileRange::Full,
        }]),
        system_hint: Some("Focus on OAuth2 best practices.".to_owned()),
        max_tokens: Some(8000),
        load_memory: None,
    };

    let cb = CodeBlockBuilder::create()
        .lang("rust")
        .target("src/auth/oauth.rs")
        .body(
            "use axum::Router;\n\
             use tower_http::trace::TraceLayer;\n\
             \n\
             pub fn oauth_router() -> Router {\n\
             Router::new()\n\
             .layer(TraceLayer::new_for_http())\n\
             }",
        )
        .build()
        .unwrap();

    // Build with all fields set; use build_unchecked because cross-references
    // to other nodes can only be validated in a multi-node file context.
    let ticket = TicketBuilder::new("myapp.ticket.oauth2.login")
        .summary("add OAuth2 login flow to the dashboard")
        .title("Add OAuth2 Login Flow")
        .description(
            "Implement full Google OAuth2 login to the dashboard.\n\
             Covers consent screen redirect, token exchange, session creation,\n\
             and logout endpoint. Backend in Rust with axum.",
        )
        .priority(Priority::High)
        .action(TicketAction::Create)
        .sdd_phase(SddPhase::Apply)
        .labels(["auth", "oauth2", "security", "backend"])
        .prompt("Implement the OAuth2 login flow following RFC 6749.")
        .assignee("agent-rust-01")
        .ticket_id("GH-2024")
        .detail("Use PKCE extension for public clients. Store tokens in httpOnly cookies.")
        .stability(Stability::Medium)
        .depends([constraints_id, rules_id])
        .related_to([session_id])
        .tags(["oauth2", "rust", "axum"])
        .notes("See RFC 6749 and PKCE extension RFC 7636.")
        .agent_context(agent_ctx)
        .code_blocks([cb])
        .build_unchecked()
        .expect("build_unchecked must succeed");

    // Verify all fields are present on the built node before the file-level validation.
    assert_eq!(ticket.priority, Some(Priority::High));
    assert_eq!(ticket.action, Some(TicketAction::Create));
    assert_eq!(ticket.sdd_phase, Some(SddPhase::Apply));
    assert_eq!(ticket.assignee.as_deref(), Some("agent-rust-01"));
    assert_eq!(ticket.ticket_id.as_deref(), Some("GH-2024"));
    assert_eq!(ticket.stability, Some(Stability::Medium));
    assert_eq!(ticket.labels.as_deref().unwrap().len(), 4);
    assert_eq!(ticket.tags.as_deref().unwrap().len(), 3);
    assert!(ticket.prompt.is_some());
    assert!(ticket.detail.is_some());
    assert!(ticket.notes.is_some());
    assert!(ticket.agent_context.is_some());
    assert_eq!(ticket.code_blocks.as_ref().unwrap().len(), 1);
    assert_eq!(ticket.depends.as_deref().unwrap().len(), 2);
    assert_eq!(ticket.related_to.as_deref().unwrap().len(), 1);

    // Build the three referenced stub nodes.
    let constraints_node = FactsBuilder::new(constraints_id)
        .summary("authentication constraints (stub)")
        .items(["sessions expire after 24h"])
        .build_unchecked()
        .unwrap();
    let rules_node = RulesBuilder::new(rules_id)
        .summary("auth rules (stub)")
        .items(["require HTTPS"])
        .build_unchecked()
        .unwrap();
    let session_node = FactsBuilder::new(session_id)
        .summary("session facts (stub)")
        .build_unchecked()
        .unwrap();

    // Assemble multi-node file and validate.
    let file = AgmFile {
        header: test_header("myapp.ticket.pkg"),
        nodes: vec![constraints_node, rules_node, session_node, ticket.clone()],
    };

    let rendered = render_canonical(&file);
    let parsed = parser::parse(&rendered).expect("multi-node file must parse");
    let diags_std = validator::validate(&parsed, &rendered, "<test>", &Default::default());
    assert!(
        !diags_std.has_errors(),
        "fully-populated ticket in multi-node Standard mode has errors: {}",
        diags_std
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );

    // Validate at Strict level via the full file text.
    let strict_opts = agm_core::validator::ValidateOptions {
        enforcement_level: EnforcementLevel::Strict,
        import_resolver: None,
        scope: agm_core::validator::ValidationScope::File,
    };
    let diags_strict = validator::validate(&parsed, &rendered, "<test-strict>", &strict_opts);
    assert!(
        !diags_strict.has_errors(),
        "fully-populated ticket in Strict mode has errors: {}",
        diags_strict
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );

    // Structural assertions on the re-parsed ticket node.
    let reparsed_ticket = parsed
        .nodes
        .iter()
        .find(|n| n.id == "myapp.ticket.oauth2.login")
        .expect("ticket node must be present after parse");
    assert_eq!(reparsed_ticket.node_type, NodeType::Ticket);
    assert_eq!(reparsed_ticket.priority, Some(Priority::High));
    assert_eq!(reparsed_ticket.action, Some(TicketAction::Create));
    assert_eq!(reparsed_ticket.sdd_phase, Some(SddPhase::Apply));
    assert_eq!(reparsed_ticket.assignee.as_deref(), Some("agent-rust-01"));
    assert_eq!(reparsed_ticket.ticket_id.as_deref(), Some("GH-2024"));
    assert_eq!(reparsed_ticket.stability, Some(Stability::Medium));
    assert_eq!(reparsed_ticket.labels.as_deref().unwrap().len(), 4);
    assert_eq!(reparsed_ticket.tags.as_deref().unwrap().len(), 3);
    assert_eq!(reparsed_ticket.code_blocks.as_ref().unwrap().len(), 1);

    // Snapshot the canonical node-only text (from the original built node).
    let snapshot_text = ticket.render_node_only();
    insta::assert_snapshot!("complex_ticket_fully_populated", snapshot_text);
}

// ---------------------------------------------------------------------------
// Test 2: Workflow with every CodeAction variant + ≥5 verify checks
// ---------------------------------------------------------------------------

#[test]
fn test_complex_workflow_all_code_actions_and_verify_checks_roundtrip() {
    // Build one block for every CodeAction variant.
    let cb_create = CodeBlockBuilder::create()
        .lang("rust")
        .target("src/handlers/auth.rs")
        .body(
            "use axum::extract::State;\n\
             use axum::response::Redirect;\n\
             \n\
             pub async fn login_handler(\n\
                 State(state): State<AppState>,\n\
             ) -> Redirect {\n\
                 Redirect::to(&state.oauth_url)\n\
             }",
        )
        .build()
        .unwrap();

    let cb_append = CodeBlockBuilder::append()
        .lang("rust")
        .target("src/router.rs")
        .body(
            "// OAuth2 routes\n\
             .route(\"/auth/login\", get(login_handler))\n\
             .route(\"/auth/callback\", get(callback_handler))\n\
             .route(\"/auth/logout\", post(logout_handler))",
        )
        .build()
        .unwrap();

    let cb_prepend = CodeBlockBuilder::prepend()
        .lang("rust")
        .target("src/main.rs")
        .body(
            "// OAuth2 feature flag\n\
             #[cfg(feature = \"oauth2\")]\n\
             mod oauth2;\n\
             \n\
             use oauth2::OAuthConfig;",
        )
        .build()
        .unwrap();

    let cb_replace_old = CodeBlockBuilder::replace()
        .lang("rust")
        .target("src/auth.rs")
        .old(
            "pub fn authenticate(user: &str) -> bool {\n\
                 todo!()\n\
             }",
        )
        .body(
            "pub fn authenticate(user: &str) -> bool {\n\
                 // Delegated to OAuth2 provider\n\
                 verify_jwt_token(user)\n\
             }",
        )
        .build()
        .unwrap();

    // Second Replace-by-old block: targets session.rs to exercise a second file path.
    // (Replace+anchor was removed — spec §23.4 requires `old` for Replace action;
    // the builder now enforces this with BuildError::Precondition.)
    let cb_replace_anchor = CodeBlockBuilder::replace()
        .lang("rust")
        .target("src/session.rs")
        .old("fn create_session() { todo!() }")
        .body(
            "fn create_session(\n\
                 user_id: &str,\n\
                 token: OAuthToken,\n\
                 jar: &CookieJar,\n\
             ) -> CookieJar {\n\
                 let sid = Uuid::new_v4().to_string();\n\
                 jar.add(Cookie::new(\"sid\", sid))\n\
             }",
        )
        .build()
        .unwrap();

    let cb_insert_before = CodeBlockBuilder::insert_before("pub struct AppState {")
        .lang("rust")
        .target("src/state.rs")
        .body(
            "/// OAuth2 configuration loaded at startup.\n\
             pub struct OAuthConfig {\n\
                 pub client_id: String,\n\
                 pub client_secret: String,\n\
                 pub redirect_uri: String,\n\
             }",
        )
        .build()
        .unwrap();

    let cb_insert_after = CodeBlockBuilder::insert_after("pub client_secret: String,")
        .lang("rust")
        .target("src/state.rs")
        .body(
            "pub scopes: Vec<String>,\n\
             pub pkce_enabled: bool,",
        )
        .build()
        .unwrap();

    let cb_full = CodeBlockBuilder::full()
        .lang("toml")
        .body(
            "[package]\n\
             name = \"myapp\"\n\
             version = \"0.1.0\"\n\
             edition = \"2024\"\n\
             \n\
             [dependencies]\n\
             axum = \"0.8\"\n\
             tower-http = \"0.6\"\n\
             uuid = { version = \"1\", features = [\"v4\"] }",
        )
        .build()
        .unwrap();

    // ≥5 verify checks of varied types
    let checks = vec![
        VerifyCheckBuilder::command("cargo test --lib -- auth")
            .expect("exit_code_0")
            .build()
            .unwrap(),
        VerifyCheckBuilder::command("cargo clippy -- -D warnings")
            .build()
            .unwrap(),
        VerifyCheckBuilder::file_exists("src/handlers/auth.rs")
            .build()
            .unwrap(),
        VerifyCheckBuilder::file_contains("src/router.rs", "/auth/login")
            .build()
            .unwrap(),
        VerifyCheckBuilder::file_not_contains("src/auth.rs", "todo!()")
            .build()
            .unwrap(),
        // node_status cross-ref is skipped in SingleNode validation scope (Bug 3 fix).
        VerifyCheckBuilder::node_status("auth.session.validate", "completed")
            .build()
            .unwrap(),
    ];

    // load_nodes cross-ref is skipped in SingleNode validation scope (Bug 3 fix).
    // These IDs would only be resolved when the full file is assembled and validated.
    let agent_ctx = AgentContext {
        load_nodes: Some(vec!["auth.token.model".to_owned(), "auth.session.validate".to_owned()]),
        load_files: Some(vec![
            LoadFile {
                path: "src/auth.rs".to_owned(),
                range: FileRange::Full,
            },
            LoadFile {
                path: "src/session.rs".to_owned(),
                range: FileRange::Full,
            },
        ]),
        system_hint: Some("Apply PKCE extension for all public clients.".to_owned()),
        max_tokens: Some(12000),
        load_memory: None,
    };

    let all_blocks = vec![
        cb_create,
        cb_append,
        cb_prepend,
        cb_replace_old,
        cb_replace_anchor,
        cb_insert_before,
        cb_insert_after,
        cb_full,
    ];

    assert_eq!(
        all_blocks.len(),
        8,
        "expected 8 code blocks, one per action"
    );

    let node = WorkflowBuilder::new("auth.oauth2.implement")
        .summary("implement full OAuth2 login flow")
        .steps([
            "add OAuthConfig struct to state",
            "implement login handler",
            "register OAuth2 routes",
            "implement callback handler",
            "create session on token exchange",
            "implement logout endpoint",
        ])
        .input(["host", "return_url", "client_id", "client_secret"])
        .output(["sid_cookie", "redirect_url", "oauth_token"])
        .code_blocks(all_blocks)
        .verify(checks)
        .agent_context(agent_ctx)
        .build()
        .expect("workflow with all code actions should be valid");

    // Parse and validate round-trip
    let agm_text = node.render_canonical();
    let file = reparse_and_validate(&agm_text);

    assert_eq!(file.nodes.len(), 1);
    let reparsed = &file.nodes[0];
    assert_eq!(reparsed.id, "auth.oauth2.implement");
    assert_eq!(reparsed.node_type, NodeType::Workflow);

    // Assert all 8 blocks round-tripped
    let blocks = reparsed.code_blocks.as_ref().unwrap();
    assert_eq!(blocks.len(), 8, "all 8 code blocks must round-trip");

    // Assert each CodeAction variant is present
    let actions: HashSet<String> = blocks.iter().map(|b| format!("{:?}", b.action)).collect();
    assert!(actions.contains("Create"), "Create action missing");
    assert!(actions.contains("Append"), "Append action missing");
    assert!(actions.contains("Prepend"), "Prepend action missing");
    assert!(actions.contains("Replace"), "Replace action missing (×2)");
    assert!(
        actions.contains("InsertBefore"),
        "InsertBefore action missing"
    );
    assert!(
        actions.contains("InsertAfter"),
        "InsertAfter action missing"
    );
    assert!(actions.contains("Full"), "Full action missing");

    // Assert ≥5 verify checks round-tripped
    let verify = reparsed.verify.as_ref().unwrap();
    assert!(
        verify.len() >= 5,
        "expected ≥5 verify checks, got {}",
        verify.len()
    );

    // Assert zero validation errors (SingleNode scope: skip cross-node refs)
    use agm_core::validator::{ValidateOptions, ValidationScope};
    let diags = validator::validate(
        &file,
        &agm_text,
        "<test>",
        &ValidateOptions {
            scope: ValidationScope::SingleNode,
            ..Default::default()
        },
    );
    assert!(
        !diags.has_errors(),
        "workflow with all code actions has validation errors: {}",
        diags
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );
}

// ---------------------------------------------------------------------------
// Test 3: Large workflow — 20+ steps
// ---------------------------------------------------------------------------

#[test]
fn test_complex_workflow_twenty_steps_validates_standard() {
    let steps: Vec<&str> = vec![
        "validate input parameters",
        "resolve tenant from host header",
        "load OAuth2 configuration from secrets manager",
        "generate PKCE code verifier and challenge",
        "build authorization URL with state parameter",
        "store state and code verifier in session",
        "redirect user to authorization server",
        "receive authorization code at callback",
        "validate state parameter from session",
        "exchange authorization code for tokens",
        "validate id_token signature and claims",
        "extract user profile from id_token",
        "look up or create user record in database",
        "create session record with encrypted tokens",
        "set httpOnly session cookie on response",
        "log authentication event to audit trail",
        "emit user.authenticated domain event",
        "clean up PKCE verifier from session",
        "redirect user to original return_url",
        "handle token refresh on expiry",
        "revoke tokens on logout",
    ];

    assert!(steps.len() >= 20, "need ≥20 steps for this test");

    let code_body = "use std::collections::HashMap;\n\
                     \n\
                     /// Orchestrates the full OAuth2 PKCE flow.\n\
                     /// Called by the auth module for each login attempt.\n\
                     pub async fn run_oauth_flow(\n\
                         config: &OAuthConfig,\n\
                         params: HashMap<String, String>,\n\
                     ) -> Result<Session, AuthError> {\n\
                         // Step 1: validate\n\
                         validate_params(&params)?;\n\
                         // Step 2: build authorize URL\n\
                         let url = build_authorize_url(config, &params).await?;\n\
                         Ok(Session { redirect_url: url.to_string() })\n\
                     }";

    let node = WorkflowBuilder::new("auth.oauth2.flow")
        .summary("orchestrate full OAuth2 PKCE authentication flow")
        .steps(steps.clone())
        .input([
            "host",
            "return_url",
            "client_id",
            "oauth_provider",
            "scopes",
        ])
        .output([
            "sid_cookie",
            "user_profile",
            "access_token",
            "refresh_token",
        ])
        .code(CodeBlock {
            action: CodeAction::Full,
            body: code_body.to_owned(),
            lang: Some("rust".to_owned()),
            target: None,
            anchor: None,
            old: None,
        })
        .build()
        .expect("large workflow should be valid");

    assert_eq!(node.steps.as_deref().unwrap().len(), 21);

    // Round-trip
    let agm_text = node.render_canonical();
    let file = reparse_and_validate(&agm_text);

    let reparsed = &file.nodes[0];
    assert_eq!(
        reparsed.steps.as_deref().unwrap().len(),
        21,
        "all 21 steps must survive round-trip"
    );
    assert_eq!(reparsed.input.as_deref().unwrap().len(), 5);
    assert_eq!(reparsed.output.as_deref().unwrap().len(), 4);
}

// ---------------------------------------------------------------------------
// Test 4: Multi-group orchestration with cross-group requires
// ---------------------------------------------------------------------------

#[test]
fn test_complex_orchestration_five_groups_with_requires_dag() {
    // DAG topology (requires = must complete before this group starts):
    //  1-schema  (no requires)
    //  2-migrate (requires 1-schema)
    //  3-seed    (requires 1-schema)
    //  4-backend (requires 2-migrate, 3-seed)
    //  5-smoke   (requires 4-backend)
    //
    // All groups reference the orchestration node itself to satisfy
    // single-node validation (the validator checks that group node IDs
    // exist in the file's id set).
    let node_id = "deploy.full.pipeline";

    let groups = vec![
        ParallelGroup {
            group: "1-schema".to_owned(),
            nodes: vec![node_id.to_owned()],
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        },
        ParallelGroup {
            group: "2-migrate".to_owned(),
            nodes: vec![node_id.to_owned()],
            strategy: Strategy::Sequential,
            requires: Some(vec!["1-schema".to_owned()]),
            max_concurrency: None,
        },
        ParallelGroup {
            group: "3-seed".to_owned(),
            nodes: vec![node_id.to_owned()],
            strategy: Strategy::Sequential,
            requires: Some(vec!["1-schema".to_owned()]),
            max_concurrency: None,
        },
        ParallelGroup {
            group: "4-backend".to_owned(),
            nodes: vec![node_id.to_owned()],
            strategy: Strategy::Parallel,
            requires: Some(vec!["2-migrate".to_owned(), "3-seed".to_owned()]),
            max_concurrency: Some(2),
        },
        ParallelGroup {
            group: "5-smoke".to_owned(),
            nodes: vec![node_id.to_owned()],
            strategy: Strategy::Sequential,
            requires: Some(vec!["4-backend".to_owned()]),
            max_concurrency: None,
        },
    ];

    let node = OrchestrationBuilder::new(node_id)
        .summary("orchestrate full deployment pipeline: schema, migrate, seed, backend, smoke")
        .parallel_groups(groups)
        .detail("Run in CI only. Requires DATABASE_URL and API_KEY env vars.")
        .notes("Smoke tests run against staging environment.")
        .tags(["deploy", "ci", "production"])
        .build()
        .expect("five-group orchestration with requires DAG should be valid");

    assert_eq!(node.parallel_groups.as_ref().unwrap().len(), 5);

    // Check requires wiring is preserved
    let groups = node.parallel_groups.as_ref().unwrap();
    assert!(groups[0].requires.is_none());
    assert_eq!(
        groups[1].requires.as_deref().unwrap(),
        ["1-schema"],
        "2-migrate must require 1-schema"
    );
    assert_eq!(
        groups[2].requires.as_deref().unwrap(),
        ["1-schema"],
        "3-seed must require 1-schema"
    );
    assert_eq!(
        groups[3].requires.as_deref().unwrap().len(),
        2,
        "4-backend must require 2 groups"
    );
    assert_eq!(
        groups[4].requires.as_deref().unwrap(),
        ["4-backend"],
        "5-smoke must require 4-backend"
    );

    // Round-trip
    let agm_text = node.render_canonical();
    let file = reparse_and_validate(&agm_text);
    let reparsed = &file.nodes[0];
    assert_eq!(reparsed.parallel_groups.as_ref().unwrap().len(), 5);
}

// ---------------------------------------------------------------------------
// Test 5: Multi-node AgmFile — 6+ nodes, cross-references, zero errors
// ---------------------------------------------------------------------------

#[test]
fn test_complex_multinode_file_six_nodes_zero_errors() {
    // Node IDs (all in the same package so cross-references resolve)
    let facts_id = "myapp.auth.constraints";
    let rules_id = "myapp.auth.rules";
    let workflow_id = "myapp.auth.login";
    let decision_id = "myapp.arch.db.choice";
    let ticket_id = "myapp.ticket.oauth2";
    let orch_id = "myapp.deploy.pipeline";

    // 1. Facts
    let facts = FactsBuilder::new(facts_id)
        .summary("authentication policy constraints")
        .items([
            "sessions expire after 24 hours",
            "MFA required for admin accounts",
            "max 5 failed login attempts before lockout",
            "tokens stored in httpOnly cookies only",
        ])
        .stability(Stability::High)
        .tags(["auth", "security", "policy"])
        .build_unchecked()
        .unwrap();

    // 2. Rules
    let rules = RulesBuilder::new(rules_id)
        .summary("authentication enforcement rules")
        .items([
            "require HTTPS on all auth endpoints",
            "rate-limit login to 5 attempts per minute per IP",
            "validate PKCE code verifier on token exchange",
            "reject tokens older than 1 hour without refresh",
        ])
        .stability(Stability::High)
        .depends([facts_id])
        .tags(["auth", "security"])
        .build_unchecked()
        .unwrap();

    // 3. Workflow
    let check = VerifyCheckBuilder::command("cargo test -- auth")
        .expect("exit_code_0")
        .build()
        .unwrap();
    let cb = CodeBlockBuilder::create()
        .lang("rust")
        .target("src/auth/login.rs")
        .body(
            "pub async fn login(\n\
                 params: LoginParams,\n\
                 state: AppState,\n\
             ) -> Result<Redirect, AuthError> {\n\
                 state.oauth.start_flow(&params).await\n\
             }",
        )
        .build()
        .unwrap();
    let workflow = WorkflowBuilder::new(workflow_id)
        .summary("authenticate user via OAuth2 PKCE and create session")
        .steps([
            "validate input",
            "generate PKCE challenge",
            "redirect to provider",
            "exchange code for tokens",
            "create session",
        ])
        .input(["host", "return_url"])
        .output(["sid_cookie", "user_id"])
        .code_blocks([cb])
        .verify([check])
        .depends([facts_id, rules_id])
        .tags(["auth", "oauth2"])
        .build_unchecked()
        .unwrap();

    // 4. Decision
    let decision = DecisionBuilder::new(decision_id)
        .summary("chose PostgreSQL over MongoDB for session and user storage")
        .rationale([
            "ACID guarantees required for session consistency",
            "existing team expertise with PostgreSQL",
            "pgvector extension for future ML features",
        ])
        .tradeoffs([
            "more complex horizontal scaling than MongoDB",
            "requires schema migrations for schema changes",
        ])
        .resolution([
            "use PostgreSQL 16 with connection pooling via PgBouncer",
            "enable pgvector extension from day one",
        ])
        .stability(Stability::High)
        .tags(["architecture", "database", "decision"])
        .build_unchecked()
        .unwrap();

    // 5. Ticket — depends on workflow and facts
    let ticket = TicketBuilder::new(ticket_id)
        .summary("implement OAuth2 PKCE login for dashboard users")
        .title("Implement OAuth2 PKCE Login")
        .description(
            "Implement the full OAuth2 PKCE login flow.\n\
             Integrates with the auth.login workflow and enforces auth.constraints.",
        )
        .priority(Priority::High)
        .action(TicketAction::Create)
        .sdd_phase(SddPhase::Apply)
        .labels(["auth", "oauth2", "backend"])
        .depends([workflow_id, facts_id])
        .related_to([rules_id])
        .tags(["oauth2", "rust"])
        .build_unchecked()
        .unwrap();

    // 6. Orchestration — groups reference all nodes in the file
    let groups = vec![
        ParallelGroup {
            group: "1-foundation".to_owned(),
            nodes: vec![facts_id.to_owned(), rules_id.to_owned()],
            strategy: Strategy::Sequential,
            requires: None,
            max_concurrency: None,
        },
        ParallelGroup {
            group: "2-implementation".to_owned(),
            nodes: vec![workflow_id.to_owned(), decision_id.to_owned()],
            strategy: Strategy::Parallel,
            requires: Some(vec!["1-foundation".to_owned()]),
            max_concurrency: Some(2),
        },
        ParallelGroup {
            group: "3-deploy".to_owned(),
            nodes: vec![ticket_id.to_owned(), orch_id.to_owned()],
            strategy: Strategy::Sequential,
            requires: Some(vec!["2-implementation".to_owned()]),
            max_concurrency: None,
        },
    ];

    let orch = OrchestrationBuilder::new(orch_id)
        .summary("orchestrate full auth feature delivery: foundation, implementation, deploy")
        .parallel_groups(groups)
        .tags(["deploy", "ci"])
        .build_unchecked()
        .unwrap();

    // Assemble multi-node AgmFile
    let file = AgmFile {
        header: test_header("myapp.auth"),
        nodes: vec![facts, rules, workflow, decision, ticket, orch],
    };

    // Render the whole file
    let rendered = render_canonical(&file);

    // Re-parse and validate
    let parsed = parser::parse(&rendered).expect("multi-node file must parse");
    let diags = validator::validate(&parsed, &rendered, "<test>", &Default::default());

    assert!(
        !diags.has_errors(),
        "multi-node file has errors: {}",
        diags
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );

    // Assert node count
    assert_eq!(parsed.nodes.len(), 6, "expected 6 nodes in multi-node file");

    // Assert node types
    let types: Vec<_> = parsed.nodes.iter().map(|n| &n.node_type).collect();
    assert!(types.contains(&&NodeType::Facts));
    assert!(types.contains(&&NodeType::Rules));
    assert!(types.contains(&&NodeType::Workflow));
    assert!(types.contains(&&NodeType::Decision));
    assert!(types.contains(&&NodeType::Ticket));
    assert!(types.contains(&&NodeType::Orchestration));
}

// ---------------------------------------------------------------------------
// Test 6a: Edge-case strings — Unicode in text fields
// ---------------------------------------------------------------------------

#[test]
fn test_complex_edge_case_unicode_in_text_fields_roundtrip() {
    // Multi-line description with Unicode
    let unicode_desc = "Résumé of auth policy changes.\n\
                        Änderungen betreffen alle Nutzer.\n\
                        Требования безопасности обновлены.\n\
                        セキュリティポリシーの変更。\n\
                        안전 정책 업데이트.";

    let node = FactsBuilder::new("auth.unicode.facts")
        .summary("auth policy with international descriptions")
        .items([
            "policy applies globally — 全球适用",
            "supports UTF-8 identifiers: café, naïve, über",
            "emoji are valid in item text: check ✓ fail ✗ warn ⚠",
        ])
        .detail(unicode_desc)
        .notes("See also: RFC 5198 (Unicode Format for Network Interchange).")
        .tags(["unicode", "i18n", "auth"])
        .build()
        .expect("facts with unicode content should be valid");

    let agm_text = node.render_canonical();
    let file = reparse_and_validate(&agm_text);

    let reparsed = &file.nodes[0];
    assert_eq!(reparsed.items.as_deref().unwrap().len(), 3);

    // The items survive round-trip (including mixed-script content)
    let items = reparsed.items.as_deref().unwrap();
    assert!(
        items.iter().any(|i| i.contains("全球")),
        "CJK characters must survive round-trip in items, got: {items:?}"
    );
    assert!(
        items.iter().any(|i| i.contains("café")),
        "Latin-extended characters must survive round-trip"
    );

    // The detail block survives round-trip
    let detail = reparsed.detail.as_deref().unwrap_or("");
    assert!(
        detail.contains("Résumé"),
        "Résumé must survive round-trip in detail, got: {detail:?}"
    );
    assert!(
        detail.contains("Änderungen"),
        "German characters must survive round-trip in detail"
    );
}

// ---------------------------------------------------------------------------
// Test 6b: Edge-case strings — long title, just under V032 boundary (190 chars)
// ---------------------------------------------------------------------------

#[test]
fn test_complex_edge_case_title_190_chars_standard_succeeds() {
    let title_190 = "A".repeat(190);

    let result = TicketBuilder::new("myapp.ticket.longtitle")
        .summary("ticket with a title just under the 200-char V032 limit")
        .title(&title_190)
        .description("Exercising the long-title boundary.")
        .priority(Priority::Normal)
        .build();

    assert!(
        result.is_ok(),
        "190-char title must succeed: {:?}",
        result.unwrap_err()
    );
    let node = result.unwrap();
    assert_eq!(node.title.as_deref().unwrap().len(), 190);
}

// ---------------------------------------------------------------------------
// Test 6c: Edge-case strings — title at exactly 201 chars triggers V032
// ---------------------------------------------------------------------------

#[test]
fn test_complex_edge_case_title_201_chars_is_v032_warning() {
    let title_201 = "B".repeat(201);

    // Standard build: V032 is a Warning — Standard mode should succeed (warnings don't block).
    let std_result = TicketBuilder::new("myapp.ticket.toolong")
        .summary("ticket with title exceeding 200 chars")
        .title(&title_201)
        .description("Exercising the V032 boundary.")
        .priority(Priority::Normal)
        .build_with(EnforcementLevel::Standard);

    // Standard build succeeds (V032 is Warning, not Error)
    assert!(
        std_result.is_ok(),
        "201-char title in Standard mode must succeed (V032 is a warning): {:?}",
        std_result.unwrap_err()
    );

    // Verify V032 is present as a warning in the raw diagnostics
    let node = std_result.unwrap();
    let agm_text = node.render_canonical();
    let file = parser::parse(&agm_text).unwrap();
    let diags = validator::validate(&file, &agm_text, "<test>", &Default::default());
    let v032 = diags
        .diagnostics()
        .iter()
        .find(|d| d.code == ErrorCode::V032);
    assert!(
        v032.is_some(),
        "V032 warning must appear for 201-char title"
    );
    assert!(!diags.has_errors(), "V032 must be a warning, not an error");
}

// ---------------------------------------------------------------------------
// Test 6d: Multi-line description — ≥5 lines round-trips faithfully
// ---------------------------------------------------------------------------

#[test]
fn test_complex_edge_case_multiline_description_roundtrip() {
    let multiline_desc = "Line one: overview of the feature.\n\
                          Line two: detailed rationale for this approach.\n\
                          Line three: dependencies and assumptions.\n\
                          Line four: out-of-scope items excluded from this ticket.\n\
                          Line five: acceptance criteria summary.";

    let node = TicketBuilder::new("myapp.ticket.multiline")
        .summary("ticket with five-line description")
        .title("Multi-Line Description Test")
        .description(multiline_desc)
        .priority(Priority::Low)
        .build()
        .expect("multi-line description ticket should be valid");

    let agm_text = node.render_canonical();
    let file = reparse_and_validate(&agm_text);

    let reparsed = &file.nodes[0];
    let desc = reparsed.description.as_deref().unwrap_or("");
    let line_count = desc.lines().count();
    assert!(
        line_count >= 5,
        "expected ≥5 lines in description after round-trip, got {line_count}: {desc:?}"
    );
    assert!(desc.contains("Line one:"), "first line must survive");
    assert!(desc.contains("Line five:"), "fifth line must survive");
}

// ---------------------------------------------------------------------------
// Test 7: CodeBlock helper variants — focused structural assertions
// ---------------------------------------------------------------------------

#[test]
fn test_complex_code_block_helpers_all_variants_correct_fields() {
    // insert_before with anchor
    let cb_ib = CodeBlockBuilder::insert_before("pub struct Foo {")
        .target("src/lib.rs")
        .body(
            "/// Metadata for Foo instances.\n\
             pub struct FooMeta {\n\
                 pub version: u32,\n\
             }",
        )
        .build()
        .unwrap();
    assert_eq!(cb_ib.action, CodeAction::InsertBefore);
    assert_eq!(cb_ib.anchor.as_deref(), Some("pub struct Foo {"));
    assert!(cb_ib.old.is_none());

    // insert_after with anchor
    let cb_ia = CodeBlockBuilder::insert_after("pub field_a: String,")
        .target("src/lib.rs")
        .body("    pub field_b: Option<u32>,\n    pub field_c: Vec<String>,")
        .build()
        .unwrap();
    assert_eq!(cb_ia.action, CodeAction::InsertAfter);
    assert_eq!(cb_ia.anchor.as_deref(), Some("pub field_a: String,"));
    assert!(cb_ia.old.is_none());

    // replace with old text (not anchor)
    let cb_ro = CodeBlockBuilder::replace()
        .target("src/lib.rs")
        .old("fn old_impl() { todo!() }")
        .body("fn old_impl() { /* real impl */ }")
        .build()
        .unwrap();
    assert_eq!(cb_ro.action, CodeAction::Replace);
    assert!(cb_ro.old.is_some(), "replace-by-old must have `old` set");
    assert!(
        cb_ro.anchor.is_none(),
        "replace-by-old must NOT have `anchor`"
    );

    // replace with anchor must fail — spec §23.4 requires `old` for Replace;
    // `anchor` is for InsertBefore/InsertAfter only.
    let err_ra = CodeBlockBuilder::replace()
        .target("src/lib.rs")
        .anchor("fn new_impl(")
        .body("fn new_impl() { /* done */ }")
        .build()
        .unwrap_err();
    assert!(
        err_ra.is_precondition(),
        "Replace with anchor must return BuildError::Precondition"
    );
    assert!(
        err_ra.to_string().contains("does not accept `anchor`"),
        "error must mention 'does not accept `anchor`', got: {err_ra}"
    );

    // full() shortcut — no target required
    let cb_full = CodeBlockBuilder::full()
        .lang("yaml")
        .body("key: value\nother: true\n")
        .build()
        .unwrap();
    assert_eq!(cb_full.action, CodeAction::Full);
    assert!(
        cb_full.target.is_none(),
        "full() shortcut must not require target"
    );
    assert_eq!(cb_full.lang.as_deref(), Some("yaml"));

    // replace with BOTH old AND anchor must be a Precondition error
    let err = CodeBlockBuilder::replace()
        .target("src/lib.rs")
        .anchor("fn foo(")
        .old("fn foo() {}")
        .body("fn foo() { /* new */ }")
        .build()
        .unwrap_err();
    assert!(
        err.is_precondition(),
        "both old+anchor on Replace must be BuildError::Precondition"
    );
}

// ---------------------------------------------------------------------------
// Test 8: Round-trip semantic equality — double-render idempotent
// ---------------------------------------------------------------------------

#[test]
fn test_complex_workflow_double_render_idempotent_and_node_eq() {
    let cb = CodeBlockBuilder::create()
        .lang("rust")
        .target("src/service.rs")
        .body(
            "pub struct AuthService {\n\
                 config: OAuthConfig,\n\
                 db: PgPool,\n\
             }\n\
             \n\
             impl AuthService {\n\
                 pub async fn login(&self, params: LoginParams) -> Result<Session, Error> {\n\
                     self.validate(&params)?;\n\
                     let code = self.start_pkce_flow(&params).await?;\n\
                     self.exchange_code(code).await\n\
                 }\n\
             }",
        )
        .build()
        .unwrap();

    let checks = vec![
        VerifyCheckBuilder::command("cargo test -- auth::service")
            .expect("exit_code_0")
            .build()
            .unwrap(),
        VerifyCheckBuilder::file_exists("src/service.rs")
            .build()
            .unwrap(),
        VerifyCheckBuilder::file_contains("src/service.rs", "AuthService")
            .build()
            .unwrap(),
    ];

    let original = WorkflowBuilder::new("auth.service.impl")
        .summary("implement AuthService with OAuth2 PKCE backend")
        .steps([
            "validate login parameters",
            "start PKCE flow",
            "exchange authorization code",
            "create session record",
            "return session cookie",
        ])
        .input(["params: LoginParams"])
        .output(["session: Session"])
        .code_blocks([cb])
        .verify(checks)
        .build()
        .expect("complex workflow for double-render test must be valid");

    // First render
    let rendered_1 = original.render_canonical();

    // Parse → re-render (second render)
    let parsed_1 = parser::parse(&rendered_1).expect("first render must parse");
    let reparsed_node = &parsed_1.nodes[0];

    // Re-render from parsed model
    let rendered_2 = reparsed_node.render_canonical();

    // The two renders must be byte-equal (double-render idempotence)
    assert_eq!(
        rendered_1, rendered_2,
        "canonical render is not idempotent: first and second renders differ"
    );

    // The re-parsed node must deep-equal the original
    assert_eq!(reparsed_node.id, original.id, "id must survive round-trip");
    assert_eq!(
        reparsed_node.node_type, original.node_type,
        "node_type must survive"
    );
    assert_eq!(
        reparsed_node.summary, original.summary,
        "summary must survive"
    );
    assert_eq!(
        reparsed_node.steps.as_deref().unwrap().len(),
        original.steps.as_deref().unwrap().len(),
        "step count must survive"
    );
    assert_eq!(
        reparsed_node.code_blocks.as_ref().unwrap().len(),
        original.code_blocks.as_ref().unwrap().len(),
        "code_blocks count must survive"
    );
    assert_eq!(
        reparsed_node.verify.as_ref().unwrap().len(),
        original.verify.as_ref().unwrap().len(),
        "verify count must survive"
    );
}

// ---------------------------------------------------------------------------
// Test 9: Error paths on complex content
// ---------------------------------------------------------------------------

#[test]
fn test_complex_error_workflow_depends_missing_node_returns_v004() {
    // Build a workflow that references a non-existent node in `depends`.
    // build_unchecked to skip single-node validation, then validate the
    // assembled multi-node file to trigger V004.
    let workflow = WorkflowBuilder::new("auth.login.broken")
        .summary("workflow with a dangling dependency")
        .depends(["nonexistent.node.that.does.not.exist"])
        .build_unchecked()
        .expect("build_unchecked must succeed even with dangling dep");

    let file = AgmFile {
        header: test_header("myapp.broken"),
        nodes: vec![workflow],
    };

    let rendered = render_canonical(&file);
    let parsed = parser::parse(&rendered).expect("file must parse");
    let diags = validator::validate(&parsed, &rendered, "<test>", &Default::default());

    // V004 = unresolved reference
    let has_v004 = diags
        .diagnostics()
        .iter()
        .any(|d| d.code == ErrorCode::V004);

    assert!(
        has_v004,
        "expected V004 for dangling dependency, got diagnostics: {}",
        diags
            .diagnostics()
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("; ")
    );
}

#[test]
fn test_complex_error_code_block_replace_both_old_and_anchor_returns_precondition() {
    let err = CodeBlockBuilder::replace()
        .target("src/lib.rs")
        .anchor("fn target_fn(")
        .old("fn target_fn() { todo!() }")
        .body("fn target_fn() { /* implementation */ }")
        .build()
        .unwrap_err();

    assert!(
        matches!(err, BuildError::Precondition(_)),
        "expected BuildError::Precondition, got: {err:?}"
    );
    assert!(
        err.to_string().contains("does not accept `anchor`"),
        "error message must mention 'does not accept `anchor`', got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Optional: Builder vs struct literal equality
// ---------------------------------------------------------------------------

#[test]
fn test_complex_builder_equals_struct_literal_workflow() {
    use agm_core::model::node::Node;

    // Build via builder
    let via_builder = WorkflowBuilder::new("auth.compare")
        .summary("compare builder vs literal")
        .steps(["step one", "step two", "step three"])
        .input(["param_a", "param_b"])
        .output(["result_x"])
        .build()
        .unwrap();

    // Build via struct literal
    let via_literal = Node {
        id: "auth.compare".to_owned(),
        node_type: NodeType::Workflow,
        summary: "compare builder vs literal".to_owned(),
        steps: Some(vec![
            "step one".to_owned(),
            "step two".to_owned(),
            "step three".to_owned(),
        ]),
        input: Some(vec!["param_a".to_owned(), "param_b".to_owned()]),
        output: Some(vec!["result_x".to_owned()]),
        ..Default::default()
    };

    assert_eq!(
        via_builder, via_literal,
        "builder output must be PartialEq-equal to the equivalent struct literal"
    );
}

// ---------------------------------------------------------------------------
// Optional: Perf sanity — 50-node AgmFile renders in < 500ms
// ---------------------------------------------------------------------------

#[test]
fn test_complex_perf_fifty_node_file_renders_under_500ms() {
    use std::time::Instant;

    let nodes: Vec<_> = (0..50)
        .map(|i| {
            FactsBuilder::new(format!("perf.facts.node{i:02}"))
                .summary(format!("performance test facts node {i}"))
                .items([
                    format!("constraint A for node {i}"),
                    format!("constraint B for node {i}"),
                    format!("constraint C for node {i}"),
                ])
                .build_unchecked()
                .unwrap()
        })
        .collect();

    let file = AgmFile {
        header: test_header("perf.test.pkg"),
        nodes,
    };

    let start = Instant::now();
    let rendered = render_canonical(&file);
    let parsed = parser::parse(&rendered).expect("50-node file must parse");
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 500,
        "render+parse of 50 nodes took {}ms, expected < 500ms",
        elapsed.as_millis()
    );
    assert_eq!(parsed.nodes.len(), 50, "all 50 nodes must be present");
}
