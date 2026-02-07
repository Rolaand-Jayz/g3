//! Action Envelope tool - writes and verifies the action envelope.
//!
//! The `write_envelope` tool is the agent's explicit final step before
//! completing a plan. It:
//! 1. Parses the provided YAML facts into an ActionEnvelope
//! 2. Writes it to the session's `envelope.yaml`
//! 3. Runs `verify_envelope()` which compiles the rulespec and executes
//!    datalog verification in shadow form (results written to files, not
//!    injected into context)
//!
//! This creates a clear happens-before edge: envelope creation + verification
//! must complete before `plan_verify()` (triggered on plan completion) checks
//! that the envelope exists.

use anyhow::Result;
use std::path::Path;
use tracing::debug;

use crate::paths::get_session_logs_dir;
use crate::ui_writer::UiWriter;
use crate::ToolCall;

use super::executor::ToolContext;
use super::invariants::{
    format_envelope_markdown, get_envelope_path, read_envelope, read_rulespec,
    write_envelope, ActionEnvelope,
};
use super::datalog::{compile_rulespec, extract_facts, execute_rules, format_datalog_program, format_datalog_results};

// ============================================================================
// Tool Implementation
// ============================================================================

/// Execute the `write_envelope` tool.
///
/// Accepts YAML facts, writes the action envelope, and runs verification.
pub async fn execute_write_envelope<W: UiWriter>(
    tool_call: &ToolCall,
    ctx: &mut ToolContext<'_, W>,
) -> Result<String> {
    debug!("Processing write_envelope tool call");

    let session_id = match ctx.session_id {
        Some(id) => id,
        None => return Ok("❌ No active session - envelopes are session-scoped.".to_string()),
    };

    // Get the facts YAML from args
    let facts_yaml = match tool_call.args.get("facts").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return Ok("❌ Missing 'facts' argument. Provide the envelope facts as YAML.".to_string()),
    };

    // Parse the YAML into an ActionEnvelope
    let envelope: ActionEnvelope = match serde_yaml::from_str(facts_yaml) {
        Ok(e) => e,
        Err(e) => return Ok(format!("❌ Invalid envelope YAML: {}", e)),
    };

    // Validate that facts is non-empty. This catches the common mistake where
    // the agent sends a raw YAML map without the required `facts:` top-level key.
    // serde silently ignores unknown fields and defaults `facts` to an empty HashMap,
    // so we must check explicitly.
    if envelope.facts.is_empty() {
        return Ok(
            "❌ Envelope has empty facts. The YAML must contain a non-empty `facts` top-level key. Example:\n\n\
             ```yaml\n\
             facts:\n\
             \x20 my_feature:\n\
             \x20   capabilities: [feature_a, feature_b]\n\
             \x20   file: \"src/my_feature.rs\"\n\
             ```".to_string()
        );
    }

    // Write the envelope to disk
    if let Err(e) = write_envelope(session_id, &envelope) {
        return Ok(format!("❌ Failed to write envelope: {}", e));
    }

    let envelope_path = get_envelope_path(session_id);
    let mut output = format!(
        "✅ Envelope written: {}\n{}",
        envelope_path.display(),
        format_envelope_markdown(&envelope),
    );

    // Run verification against rulespec (shadow mode)
    let effective_wd = ctx.working_dir
        .map(Path::new)
        .unwrap_or_else(|| Path::new("."));
    let verification_note = verify_envelope(session_id, effective_wd);
    output.push_str(&verification_note);

    Ok(output)
}

// ============================================================================
// Envelope Verification
// ============================================================================

/// Verify the action envelope against the compiled rulespec using datalog.
///
/// This is the core verification step that:
/// 1. Reads `analysis/rulespec.yaml` from the working directory
/// 2. Compiles it into datalog relations
/// 3. Loads the envelope from the session
/// 4. Extracts facts and runs datalog rules
/// 5. Writes results to session artifacts (shadow mode - stderr + files)
///
/// Returns a short status string for inclusion in tool output.
pub fn verify_envelope(session_id: &str, working_dir: &Path) -> String {
    // Read rulespec from analysis/rulespec.yaml
    let rulespec = match read_rulespec(working_dir) {
        Ok(Some(rs)) => rs,
        Ok(None) => {
            eprintln!("\nℹ️  No analysis/rulespec.yaml found - skipping datalog verification");
            return "\nℹ️  No rulespec found — skipping invariant verification.\n".to_string();
        }
        Err(e) => {
            eprintln!("\n⚠️  Failed to read analysis/rulespec.yaml: {}", e);
            return format!("\n⚠️  Failed to read rulespec: {}\n", e);
        }
    };

    // Compile rulespec on-the-fly
    let compiled = match compile_rulespec(&rulespec, "envelope-verify", 0) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("\n⚠️  Failed to compile rulespec: {}", e);
            return format!("\n⚠️  Failed to compile rulespec: {}\n", e);
        }
    };

    if compiled.is_empty() {
        eprintln!("\nℹ️  Rulespec has no predicates - skipping datalog verification");
        return "\nℹ️  Rulespec has no predicates — skipping invariant verification.\n".to_string();
    }

    // Load envelope
    let envelope = match read_envelope(session_id) {
        Ok(Some(e)) => e,
        Ok(None) => {
            eprintln!("\n⚠️  No envelope found - skipping datalog verification");
            return "\n⚠️  No envelope found — skipping invariant verification.\n".to_string();
        }
        Err(e) => {
            eprintln!("\n⚠️  Failed to load envelope: {}", e);
            return format!("\n⚠️  Failed to load envelope: {}\n", e);
        }
    };

    // Extract facts from envelope
    let facts = extract_facts(&envelope, &compiled);

    // Execute datalog rules
    let result = execute_rules(&compiled, &facts);

    // Format results
    let output = format_datalog_results(&result);

    let session_dir = get_session_logs_dir(session_id);

    // Write compiled rules to .dl file
    let dl_path = session_dir.join("rulespec.compiled.dl");
    let datalog_program = format_datalog_program(&compiled, &facts);
    if let Err(e) = std::fs::write(&dl_path, &datalog_program) {
        eprintln!("⚠️  Failed to write compiled rules: {}", e);
    }

    // Write evaluation report
    let eval_path = session_dir.join("datalog_evaluation.txt");
    match std::fs::write(&eval_path, &output) {
        Ok(_) => {
            eprintln!("📊 Compiled rules: {}", dl_path.display());
            eprintln!("📊 Evaluation report: {}", eval_path.display());
        }
        Err(e) => {
            eprintln!("⚠️  Failed to write datalog evaluation: {}", e);
        }
    }

    // Return a summary for the tool output
    let summary = if result.failed_count == 0 {
        format!(
            "\n✅ Invariant verification: {}/{} passed\n",
            result.passed_count,
            result.passed_count + result.failed_count,
        )
    } else {
        format!(
            "\n⚠️  Invariant verification: {}/{} passed, {} failed\n",
            result.passed_count,
            result.passed_count + result.failed_count,
            result.failed_count,
        )
    };

    summary
}
