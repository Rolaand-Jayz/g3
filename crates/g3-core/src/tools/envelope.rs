//! Action Envelope tool - writes and verifies the action envelope.
//!
//! The `write_envelope` tool is the agent's explicit final step before
//! completing a plan. It:
//! 1. Parses the provided YAML facts into an ActionEnvelope
//! 2. Writes it to the session's `envelope.yaml`
//! 3. Runs `verify_envelope()` which compiles the rulespec and executes
//!    datalog verification in shadow form (results written to files, not
//!    injected into context)
//! 4. If all predicates pass, stamps the envelope with a verification token
//!    (`verified: "g3v1:<base64>"`) that proves deterministic checks passed.
//!
//! This creates a clear happens-before edge: envelope creation + verification
//! must complete before `plan_verify()` (triggered on plan completion) checks
//! that the envelope exists.

use anyhow::{anyhow, Result};
use base64::Engine;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tracing::debug;

use crate::paths::get_session_logs_dir;
use crate::ui_writer::UiWriter;
use crate::ToolCall;

use super::executor::ToolContext;
use super::invariants::{
    format_envelope_markdown, get_envelope_path, read_envelope, read_rulespec,
    write_envelope, ActionEnvelope, Rulespec,
};
use super::datalog::{
    compile_rulespec, execute_rules, extract_facts, format_datalog_program,
    format_datalog_results,
};

// ============================================================================
// Verification Key Management
// ============================================================================

const VERIFICATION_KEY_FILENAME: &str = "verification.key";
const VERIFICATION_KEY_LEN: usize = 32;
const TOKEN_PREFIX: &str = "g3v1:";

/// Get the path to the global verification key: `~/.g3/verification.key`
fn get_verification_key_path() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| anyhow!("Cannot determine home directory"))?;
    Ok(PathBuf::from(home).join(".g3").join(VERIFICATION_KEY_FILENAME))
}

/// Read or create the verification key at `~/.g3/verification.key`.
///
/// - If the key file exists, reads and returns it.
/// - If it doesn't exist, generates 32 random bytes, writes them with
///   mode 600 (Unix), and returns the key.
/// - The key is raw bytes, never logged or shown to the LLM.
pub fn get_or_create_verification_key() -> Result<Vec<u8>> {
    let path = get_verification_key_path()?;

    // If key exists, read and return it
    if path.exists() {
        let key = std::fs::read(&path)?;
        if key.len() == VERIFICATION_KEY_LEN {
            return Ok(key);
        }
        // Key file is wrong size — regenerate
        debug!("Verification key has wrong size ({}), regenerating", key.len());
    }

    // Ensure ~/.g3/ directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Generate 32 random bytes
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut key = vec![0u8; VERIFICATION_KEY_LEN];
    rng.fill(&mut key[..]);

    // Write key file
    std::fs::write(&path, &key)?;

    // Set permissions to 600 (owner read/write only) on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }

    debug!("Generated new verification key at {}", path.display());
    Ok(key)
}

/// Read the verification key. Returns None if it doesn't exist.
pub fn read_verification_key() -> Result<Option<Vec<u8>>> {
    let path = get_verification_key_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let key = std::fs::read(&path)?;
    if key.len() != VERIFICATION_KEY_LEN {
        return Ok(None);
    }
    Ok(Some(key))
}

// ============================================================================
// Token Computation
// ============================================================================

/// Compute a canonical YAML representation of the envelope facts.
///
/// This produces a deterministic string by sorting keys, ensuring
/// the same facts always produce the same canonical form.
fn canonical_facts_yaml(envelope: &ActionEnvelope) -> String {
    // Use serde_yaml which produces deterministic output for the same structure.
    // We serialize only the facts (not the verified field) to get a stable input.
    let mut sorted_facts: Vec<(&String, &serde_yaml::Value)> =
        envelope.facts.iter().collect();
    sorted_facts.sort_by_key(|(k, _)| *k);

    let mut mapping = serde_yaml::Mapping::new();
    for (k, v) in sorted_facts {
        mapping.insert(
            serde_yaml::Value::String(k.clone()),
            v.clone(),
        );
    }
    serde_yaml::to_string(&mapping).unwrap_or_default()
}

/// Compute a canonical YAML representation of the rulespec.
fn canonical_rulespec_yaml(rulespec: &Rulespec) -> String {
    serde_yaml::to_string(rulespec).unwrap_or_default()
}

/// Mint a verification token using a keyed SipHash MAC.
///
/// The token is computed as:
///   SipHash-2-4(key[0..16], canonical_facts || "\x00" || canonical_rulespec)
///
/// Then encoded as: `g3v1:<base64(8-byte hash)>`
///
/// This is a keyed PRF (pseudo-random function) — not a plain hash.
/// The key is never exposed to the LLM, making the token unguessable.
pub fn mint_token(key: &[u8], envelope: &ActionEnvelope, rulespec: &Rulespec) -> String {
    let facts_yaml = canonical_facts_yaml(envelope);
    let rulespec_yaml = canonical_rulespec_yaml(rulespec);

    let hash = compute_keyed_hash(key, &facts_yaml, &rulespec_yaml);
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash.to_le_bytes());

    format!("{}{}", TOKEN_PREFIX, encoded)
}

/// Compute a keyed SipHash over the canonical content.
///
/// Uses the first 16 bytes of the key as SipHash key (k0, k1).
/// The message is: facts_yaml + NUL separator + rulespec_yaml.
fn compute_keyed_hash(key: &[u8], facts_yaml: &str, rulespec_yaml: &str) -> u64 {
    // Extract k0 and k1 from the key (first 16 bytes)
    let k0 = if key.len() >= 8 {
        u64::from_le_bytes(key[0..8].try_into().unwrap())
    } else {
        0
    };
    let k1 = if key.len() >= 16 {
        u64::from_le_bytes(key[8..16].try_into().unwrap())
    } else {
        0
    };

    #[allow(deprecated)] // SipHasher is deprecated but we need keyed hashing
    let mut hasher = std::hash::SipHasher::new_with_keys(k0, k1);
    facts_yaml.hash(&mut hasher);
    0u8.hash(&mut hasher); // NUL separator
    rulespec_yaml.hash(&mut hasher);
    hasher.finish()
}

// ============================================================================
// Token Verification (cross-process)
// ============================================================================

/// Verify the token in an envelope against the verification key and rulespec.
///
/// This is the cross-process verification entry point. It:
/// 1. Reads `~/.g3/verification.key`
/// 2. Reads the envelope from the session
/// 3. Reads the rulespec from the working directory
/// 4. Recomputes the token and compares
///
/// Returns:
/// - `Ok(true)` if the token matches
/// - `Ok(false)` if the token doesn't match or is missing
/// - `Err(...)` if required files are missing
pub fn verify_token(session_id: &str, working_dir: &Path) -> Result<bool> {
    // Read verification key
    let key = match read_verification_key()? {
        Some(k) => k,
        None => return Err(anyhow!("Verification key not found at ~/.g3/verification.key")),
    };

    // Read envelope
    let envelope = match read_envelope(session_id)? {
        Some(e) => e,
        None => return Err(anyhow!("Envelope not found for session {}", session_id)),
    };

    // Check that envelope has a verified field
    let stored_token = match &envelope.verified {
        Some(t) => t.clone(),
        None => return Ok(false),
    };

    // Read rulespec
    let rulespec = match read_rulespec(working_dir)? {
        Some(rs) => rs,
        None => return Err(anyhow!("Rulespec not found at {}/analysis/rulespec.yaml", working_dir.display())),
    };

    // Recompute token (using envelope without the verified field for computation)
    let mut clean_envelope = envelope.clone();
    clean_envelope.verified = None;
    let expected_token = mint_token(&key, &clean_envelope, &rulespec);

    Ok(stored_token == expected_token)
}

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

    // Write the envelope to disk (without verified token initially)
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
/// 6. If all predicates pass, stamps the envelope with a verification token
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

    // If all predicates passed, stamp the envelope with a verification token
    if result.failed_count == 0 && result.passed_count > 0 {
        match stamp_envelope(session_id, &envelope, &rulespec) {
            Ok(_) => {
                eprintln!("🔏 Envelope stamped with verification token");
            }
            Err(e) => {
                eprintln!("⚠️  Failed to stamp envelope: {}", e);
            }
        }
    }

    // Return a summary for the tool output
    // NOTE: The token value is intentionally NOT included in the output
    // returned to the LLM. It exists only in the envelope.yaml file.
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

/// Stamp an envelope with a verification token and re-write it to disk.
///
/// This is called only when all rulespec predicates pass. It:
/// 1. Gets or creates the verification key
/// 2. Computes the token over the canonical facts + rulespec
/// 3. Sets the `verified` field on the envelope
/// 4. Re-writes the envelope to disk
fn stamp_envelope(
    session_id: &str,
    envelope: &ActionEnvelope,
    rulespec: &Rulespec,
) -> Result<()> {
    let key = get_or_create_verification_key()?;

    // Compute token over the original envelope (without any previous verified field)
    let mut clean_envelope = envelope.clone();
    clean_envelope.verified = None;
    let token = mint_token(&key, &clean_envelope, rulespec);

    // Set the verified field and re-write
    let mut stamped = envelope.clone();
    stamped.verified = Some(token);
    write_envelope(session_id, &stamped)?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::Value as YamlValue;

    fn make_test_key() -> Vec<u8> {
        vec![1u8; VERIFICATION_KEY_LEN]
    }

    fn make_different_key() -> Vec<u8> {
        vec![2u8; VERIFICATION_KEY_LEN]
    }

    fn make_test_envelope() -> ActionEnvelope {
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact(
            "feature",
            serde_yaml::from_str("capabilities: [a, b]\nfile: src/foo.rs").unwrap(),
        );
        envelope
    }

    fn make_test_rulespec() -> Rulespec {
        serde_yaml::from_str(
            r#"
            claims:
              - name: caps
                selector: feature.capabilities
            predicates:
              - claim: caps
                rule: exists
                source: task_prompt
            "#,
        )
        .unwrap()
    }

    #[test]
    fn test_mint_token_deterministic() {
        let key = make_test_key();
        let envelope = make_test_envelope();
        let rulespec = make_test_rulespec();

        let token1 = mint_token(&key, &envelope, &rulespec);
        let token2 = mint_token(&key, &envelope, &rulespec);

        assert_eq!(token1, token2, "Same inputs must produce same token");
        assert!(token1.starts_with(TOKEN_PREFIX), "Token must start with g3v1:");
    }

    #[test]
    fn test_mint_token_different_key() {
        let envelope = make_test_envelope();
        let rulespec = make_test_rulespec();

        let token1 = mint_token(&make_test_key(), &envelope, &rulespec);
        let token2 = mint_token(&make_different_key(), &envelope, &rulespec);

        assert_ne!(token1, token2, "Different keys must produce different tokens");
    }

    #[test]
    fn test_mint_token_different_facts() {
        let key = make_test_key();
        let rulespec = make_test_rulespec();

        let envelope1 = make_test_envelope();
        let mut envelope2 = make_test_envelope();
        envelope2.add_fact("extra", YamlValue::String("tampered".to_string()));

        let token1 = mint_token(&key, &envelope1, &rulespec);
        let token2 = mint_token(&key, &envelope2, &rulespec);

        assert_ne!(token1, token2, "Different facts must produce different tokens");
    }

    #[test]
    fn test_mint_token_different_rulespec() {
        let key = make_test_key();
        let envelope = make_test_envelope();

        let rulespec1 = make_test_rulespec();
        let rulespec2: Rulespec = serde_yaml::from_str(
            r#"
            claims:
              - name: caps
                selector: feature.capabilities
            predicates:
              - claim: caps
                rule: min_length
                value: 5
                source: task_prompt
            "#,
        )
        .unwrap();

        let token1 = mint_token(&key, &envelope, &rulespec1);
        let token2 = mint_token(&key, &envelope, &rulespec2);

        assert_ne!(token1, token2, "Different rulespec must produce different tokens");
    }

    #[test]
    fn test_mint_token_format() {
        let key = make_test_key();
        let envelope = make_test_envelope();
        let rulespec = make_test_rulespec();

        let token = mint_token(&key, &envelope, &rulespec);

        assert!(token.starts_with("g3v1:"));
        let b64_part = &token[5..];
        // base64 URL-safe no-pad should decode to 8 bytes (u64)
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(b64_part)
            .expect("Token should be valid base64");
        assert_eq!(decoded.len(), 8, "SipHash produces 8 bytes");
    }

    #[test]
    fn test_fabricated_token_fails() {
        let key = make_test_key();
        let envelope = make_test_envelope();
        let rulespec = make_test_rulespec();

        let real_token = mint_token(&key, &envelope, &rulespec);
        let fake_token = "g3v1:AAAAAAAAAA".to_string();

        assert_ne!(real_token, fake_token, "Fabricated token should not match");
    }

    #[test]
    fn test_envelope_verified_field_serialization() {
        let mut envelope = make_test_envelope();
        envelope.verified = Some("g3v1:test123".to_string());

        let yaml = serde_yaml::to_string(&envelope).unwrap();
        assert!(yaml.contains("verified:"), "YAML should contain verified field");
        assert!(yaml.contains("g3v1:test123"), "YAML should contain token value");
    }

    #[test]
    fn test_envelope_without_verified_field_backward_compat() {
        // Simulate an old envelope YAML without verified field
        let yaml = r#"
            facts:
              feature:
                capabilities: [a, b]
        "#;

        let envelope: ActionEnvelope = serde_yaml::from_str(yaml).unwrap();
        assert!(envelope.verified.is_none(), "Old envelopes should parse with verified=None");
        assert!(!envelope.facts.is_empty());
    }

    #[test]
    fn test_envelope_verified_not_in_to_yaml_value() {
        let mut envelope = make_test_envelope();
        envelope.verified = Some("g3v1:test123".to_string());

        let yaml_value = envelope.to_yaml_value();
        let yaml_str = serde_yaml::to_string(&yaml_value).unwrap();

        assert!(!yaml_str.contains("verified"),
            "to_yaml_value() must not include verified field");
        assert!(!yaml_str.contains("g3v1"),
            "to_yaml_value() must not include token");
    }

    #[test]
    fn test_envelope_verified_none_not_serialized() {
        let envelope = make_test_envelope();
        assert!(envelope.verified.is_none());

        let yaml = serde_yaml::to_string(&envelope).unwrap();
        assert!(!yaml.contains("verified"),
            "None verified should not appear in YAML");
    }

    #[test]
    fn test_canonical_facts_yaml_deterministic() {
        let envelope = make_test_envelope();

        let yaml1 = canonical_facts_yaml(&envelope);
        let yaml2 = canonical_facts_yaml(&envelope);

        assert_eq!(yaml1, yaml2, "Canonical YAML must be deterministic");
    }

    #[test]
    fn test_canonical_facts_yaml_sorted_keys() {
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact("zebra", YamlValue::String("z".to_string()));
        envelope.add_fact("alpha", YamlValue::String("a".to_string()));
        envelope.add_fact("middle", YamlValue::String("m".to_string()));

        let yaml = canonical_facts_yaml(&envelope);

        let alpha_pos = yaml.find("alpha").unwrap();
        let middle_pos = yaml.find("middle").unwrap();
        let zebra_pos = yaml.find("zebra").unwrap();

        assert!(alpha_pos < middle_pos, "Keys should be sorted");
        assert!(middle_pos < zebra_pos, "Keys should be sorted");
    }
}
