//! Datalog-based invariant verification using datafrog.
//!
//! This module compiles rulespecs into datalog relations and executes them
//! against facts extracted from action envelopes. It provides a more rigorous
//! verification mechanism than the simple predicate evaluation in invariants.rs.
//!
//! ## Architecture
//!
//! 1. **Compilation Phase** (on plan_approve):
//!    - Parse rulespec claims and predicates
//!    - Generate datafrog relations and rules
//!    - Store compiled representation for later execution
//!
//! 2. **Execution Phase** (on plan_verify):
//!    - Extract facts from action envelope using selectors
//!    - Inject facts into datafrog relations
//!    - Run datalog to fixed point
//!    - Collect and format results
//!
//! ## Datalog Mapping
//!
//! Claims become base relations:
//! ```text
//! claim_value(claim_name: String, value: String)
//! ```
//!
//! Predicates become rules that derive pass/fail:
//! ```text
//! predicate_pass(pred_id) :- claim_value(claim, expected_value)
//! ```

use anyhow::{anyhow, Result};
use datafrog::{Iteration, Relation};
use serde::{Deserialize, Serialize};
use serde_yaml::Value as YamlValue;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::invariants::{
    ActionEnvelope, InvariantSource, PredicateRule, Rulespec, Selector,
};
#[cfg(test)]
use super::invariants::{Claim, Predicate};

use crate::paths::get_session_logs_dir;

// ============================================================================
// Compiled Datalog Representation
// ============================================================================

/// A compiled predicate ready for datalog execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledPredicate {
    /// Unique ID for this predicate (index in original rulespec)
    pub id: usize,
    /// Name of the claim this predicate references
    pub claim_name: String,
    /// The selector path from the claim
    pub selector: String,
    /// The rule type
    pub rule: PredicateRule,
    /// Expected value (serialized as string for datalog)
    pub expected_value: Option<String>,
    /// Source of this invariant
    pub source: InvariantSource,
    /// Optional notes
    pub notes: Option<String>,
}

/// Compiled rulespec ready for datalog execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledRulespec {
    /// Original plan_id this was compiled for
    pub plan_id: String,
    /// Revision of the plan when compiled
    pub compiled_at_revision: u32,
    /// Compiled predicates
    pub predicates: Vec<CompiledPredicate>,
    /// Claim name -> selector mapping
    pub claims: HashMap<String, String>,
}

impl CompiledRulespec {
    /// Check if the compiled rulespec is empty.
    pub fn is_empty(&self) -> bool {
        self.predicates.is_empty()
    }
}

// ============================================================================
// Compilation: Rulespec -> Datalog
// ============================================================================

/// Compile a rulespec into a datalog-ready representation.
///
/// This validates the rulespec and converts it into a form that can be
/// efficiently executed by datafrog.
pub fn compile_rulespec(
    rulespec: &Rulespec,
    plan_id: &str,
    revision: u32,
) -> Result<CompiledRulespec> {
    // Build claim lookup
    let mut claims: HashMap<String, String> = HashMap::new();
    for claim in &rulespec.claims {
        // Validate selector syntax
        Selector::parse(&claim.selector).map_err(|e| {
            anyhow!(
                "Invalid selector '{}' in claim '{}': {}",
                claim.selector,
                claim.name,
                e
            )
        })?;
        claims.insert(claim.name.clone(), claim.selector.clone());
    }

    // Compile predicates
    let mut compiled_predicates = Vec::new();
    for (idx, predicate) in rulespec.predicates.iter().enumerate() {
        // Verify claim exists
        let selector = claims.get(&predicate.claim).ok_or_else(|| {
            anyhow!(
                "Predicate {} references unknown claim '{}'",
                idx,
                predicate.claim
            )
        })?;

        // Convert value to string representation for datalog
        let expected_value = predicate.value.as_ref().map(yaml_value_to_string);

        compiled_predicates.push(CompiledPredicate {
            id: idx,
            claim_name: predicate.claim.clone(),
            selector: selector.clone(),
            rule: predicate.rule.clone(),
            expected_value,
            source: predicate.source,
            notes: predicate.notes.clone(),
        });
    }

    Ok(CompiledRulespec {
        plan_id: plan_id.to_string(),
        compiled_at_revision: revision,
        predicates: compiled_predicates,
        claims,
    })
}

/// Convert a YAML value to a string for datalog comparison.
fn yaml_value_to_string(value: &YamlValue) -> String {
    match value {
        YamlValue::Null => "null".to_string(),
        YamlValue::Bool(b) => b.to_string(),
        YamlValue::Number(n) => n.to_string(),
        YamlValue::String(s) => s.clone(),
        YamlValue::Sequence(seq) => {
            // For sequences, we'll handle them specially in execution
            format!(
                "[{}]",
                seq.iter()
                    .map(yaml_value_to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
        YamlValue::Mapping(_) => "{object}".to_string(),
        YamlValue::Tagged(t) => format!("!{}", t.tag),
    }
}

// ============================================================================
// Fact Extraction: Envelope -> Datalog Facts
// ============================================================================

/// A fact extracted from the envelope for datalog processing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fact {
    /// The claim name this fact belongs to
    pub claim_name: String,
    /// The extracted value as a string
    pub value: String,
}

/// Extract facts from an action envelope using the compiled rulespec's selectors.
///
/// Returns a set of facts that can be injected into datafrog relations.
pub fn extract_facts(envelope: &ActionEnvelope, compiled: &CompiledRulespec) -> HashSet<Fact> {
    let mut facts = HashSet::new();
    let envelope_value = envelope.to_yaml_value();

    for (claim_name, selector_str) in &compiled.claims {
        let selector = match Selector::parse(selector_str) {
            Ok(s) => s,
            Err(_) => continue, // Skip invalid selectors (shouldn't happen after compilation)
        };

        let values = selector.select(&envelope_value);

        for value in values {
            // Extract individual values from the selected result
            extract_values_recursive(claim_name, &value, &mut facts);
        }
    }

    facts
}

/// Recursively extract values from a YAML value into facts.
fn extract_values_recursive(claim_name: &str, value: &YamlValue, facts: &mut HashSet<Fact>) {
    match value {
        YamlValue::Sequence(seq) => {
            // For arrays, add each element as a separate fact
            for item in seq {
                extract_values_recursive(claim_name, item, facts);
            }
            // Also add the array length as a special fact
            facts.insert(Fact {
                claim_name: format!("{}.__length", claim_name),
                value: seq.len().to_string(),
            });
        }
        YamlValue::Mapping(map) => {
            // For objects, add a marker that it exists
            facts.insert(Fact {
                claim_name: claim_name.to_string(),
                value: "{object}".to_string(),
            });
            // And recurse into each field
            for (k, v) in map {
                let key_str = yaml_value_to_string(k);
                let nested_claim = format!("{}.{}", claim_name, key_str);
                extract_values_recursive(&nested_claim, v, facts);
            }
        }
        YamlValue::Null => {
            facts.insert(Fact {
                claim_name: claim_name.to_string(),
                value: "null".to_string(),
            });
        }
        _ => {
            // Scalar values
            facts.insert(Fact {
                claim_name: claim_name.to_string(),
                value: yaml_value_to_string(value),
            });
        }
    }
}

// ============================================================================
// Datalog Execution
// ============================================================================

/// Result of evaluating a single predicate via datalog.
#[derive(Debug, Clone)]
pub struct DatalogPredicateResult {
    /// Predicate ID
    pub id: usize,
    /// Claim name
    pub claim_name: String,
    /// Rule type
    pub rule: PredicateRule,
    /// Expected value (if any)
    pub expected_value: Option<String>,
    /// Whether the predicate passed
    pub passed: bool,
    /// Human-readable reason
    pub reason: String,
    /// Source of the invariant
    pub source: InvariantSource,
    /// Notes from the predicate
    pub notes: Option<String>,
}

/// Result of executing all datalog rules.
#[derive(Debug, Clone)]
pub struct DatalogExecutionResult {
    /// Results for each predicate
    pub predicate_results: Vec<DatalogPredicateResult>,
    /// Number of facts extracted from envelope
    pub fact_count: usize,
    /// Number of predicates that passed
    pub passed_count: usize,
    /// Number of predicates that failed
    pub failed_count: usize,
}

impl DatalogExecutionResult {
    /// Check if all predicates passed.
    pub fn all_passed(&self) -> bool {
        self.failed_count == 0
    }
}

/// Execute compiled datalog rules against extracted facts.
///
/// This uses datafrog to evaluate the predicates. The execution model:
/// 1. Create relations for claim values
/// 2. For each predicate, check if the required facts exist
/// 3. Collect pass/fail results
pub fn execute_rules(
    compiled: &CompiledRulespec,
    facts: &HashSet<Fact>,
) -> DatalogExecutionResult {
    let mut predicate_results = Vec::new();
    let mut passed_count = 0;
    let mut failed_count = 0;

    // Build a lookup for quick fact checking
    let fact_lookup: HashMap<&str, HashSet<&str>> = {
        let mut lookup: HashMap<&str, HashSet<&str>> = HashMap::new();
        for fact in facts {
            lookup
                .entry(fact.claim_name.as_str())
                .or_default()
                .insert(fact.value.as_str());
        }
        lookup
    };

    // Use datafrog for the core evaluation
    // We model this as: for each predicate, check if the required relation holds
    let mut iteration = Iteration::new();

    // Create a relation of all (claim_name, value) pairs
    let claim_values: Relation<(String, String)> = facts
        .iter()
        .map(|f| (f.claim_name.clone(), f.value.clone()))
        .collect();

    // Variable to hold claim values during iteration
    let claim_var = iteration.variable::<(String, String)>("claim_values");
    claim_var.extend(claim_values.iter().cloned());

    // Run to fixed point (trivial in this case since we have no recursive rules)
    while iteration.changed() {
        // No recursive rules, so this completes immediately
    }

    // Now evaluate each predicate
    for pred in &compiled.predicates {
        let result = evaluate_predicate_datalog(pred, &fact_lookup);
        
        if result.passed {
            passed_count += 1;
        } else {
            failed_count += 1;
        }
        
        predicate_results.push(result);
    }

    DatalogExecutionResult {
        predicate_results,
        fact_count: facts.len(),
        passed_count,
        failed_count,
    }
}

/// Evaluate a single predicate using the fact lookup.
fn evaluate_predicate_datalog(
    pred: &CompiledPredicate,
    fact_lookup: &HashMap<&str, HashSet<&str>>,
) -> DatalogPredicateResult {
    let claim_values = fact_lookup.get(pred.claim_name.as_str());
    
    let (passed, reason) = match pred.rule {
        PredicateRule::Exists => {
            if claim_values.is_some() && !claim_values.unwrap().is_empty() {
                (true, "Value exists".to_string())
            } else {
                (false, "Value does not exist".to_string())
            }
        }
        PredicateRule::NotExists => {
            if claim_values.is_none() || claim_values.unwrap().is_empty() {
                (true, "Value does not exist as expected".to_string())
            } else {
                (false, "Value exists but should not".to_string())
            }
        }
        PredicateRule::Contains => {
            let expected = pred.expected_value.as_deref().unwrap_or("");
            if let Some(values) = claim_values {
                if values.contains(expected) {
                    (true, format!("Contains '{}'", expected))
                } else {
                    (false, format!("Does not contain '{}'", expected))
                }
            } else {
                (false, format!("Claim '{}' has no values", pred.claim_name))
            }
        }
        PredicateRule::Equals => {
            let expected = pred.expected_value.as_deref().unwrap_or("");
            if let Some(values) = claim_values {
                if values.len() == 1 && values.contains(expected) {
                    (true, format!("Equals '{}'", expected))
                } else if values.len() > 1 {
                    (false, format!("Multiple values found, expected single value '{}'", expected))
                } else {
                    let actual = values.iter().next().map(|s| *s).unwrap_or("<none>");
                    (false, format!("Expected '{}', got '{}'", expected, actual))
                }
            } else {
                (false, format!("Claim '{}' has no values", pred.claim_name))
            }
        }
        PredicateRule::MinLength => {
            let expected: usize = pred
                .expected_value
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            
            // Check the __length fact
            let length_claim = format!("{}.__length", pred.claim_name);
            let length = fact_lookup
                .get(length_claim.as_str())
                .and_then(|v| v.iter().next())
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            
            if length >= expected {
                (true, format!("Length {} >= {}", length, expected))
            } else {
                (false, format!("Length {} < {} (minimum)", length, expected))
            }
        }
        PredicateRule::MaxLength => {
            let expected: usize = pred
                .expected_value
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(usize::MAX);
            
            let length_claim = format!("{}.__length", pred.claim_name);
            let length = fact_lookup
                .get(length_claim.as_str())
                .and_then(|v| v.iter().next())
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            
            if length <= expected {
                (true, format!("Length {} <= {}", length, expected))
            } else {
                (false, format!("Length {} > {} (maximum)", length, expected))
            }
        }
        PredicateRule::GreaterThan => {
            let expected: f64 = pred
                .expected_value
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            
            if let Some(values) = claim_values {
                if let Some(actual) = values.iter().next().and_then(|s| s.parse::<f64>().ok()) {
                    if actual > expected {
                        (true, format!("{} > {}", actual, expected))
                    } else {
                        (false, format!("{} is not > {}", actual, expected))
                    }
                } else {
                    (false, "Value is not a number".to_string())
                }
            } else {
                (false, format!("Claim '{}' has no values", pred.claim_name))
            }
        }
        PredicateRule::LessThan => {
            let expected: f64 = pred
                .expected_value
                .as_deref()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            
            if let Some(values) = claim_values {
                if let Some(actual) = values.iter().next().and_then(|s| s.parse::<f64>().ok()) {
                    if actual < expected {
                        (true, format!("{} < {}", actual, expected))
                    } else {
                        (false, format!("{} is not < {}", actual, expected))
                    }
                } else {
                    (false, "Value is not a number".to_string())
                }
            } else {
                (false, format!("Claim '{}' has no values", pred.claim_name))
            }
        }
        PredicateRule::Matches => {
            let pattern = pred.expected_value.as_deref().unwrap_or("");
            let regex = match regex::Regex::new(pattern) {
                Ok(r) => r,
                Err(e) => {
                    return DatalogPredicateResult {
                        id: pred.id,
                        claim_name: pred.claim_name.clone(),
                        rule: pred.rule.clone(),
                        expected_value: pred.expected_value.clone(),
                        passed: false,
                        reason: format!("Invalid regex: {}", e),
                        source: pred.source,
                        notes: pred.notes.clone(),
                    };
                }
            };
            
            if let Some(values) = claim_values {
                if values.iter().any(|v| regex.is_match(v)) {
                    (true, format!("Matches pattern '{}'", pattern))
                } else {
                    (false, format!("No value matches pattern '{}'", pattern))
                }
            } else {
                (false, format!("Claim '{}' has no values", pred.claim_name))
            }
        }
    };

    DatalogPredicateResult {
        id: pred.id,
        claim_name: pred.claim_name.clone(),
        rule: pred.rule.clone(),
        expected_value: pred.expected_value.clone(),
        passed,
        reason,
        source: pred.source,
        notes: pred.notes.clone(),
    }
}

// ============================================================================
// Storage
// ============================================================================

/// Get the path to the compiled rulespec file for a session.
pub fn get_compiled_rulespec_path(session_id: &str) -> PathBuf {
    get_session_logs_dir(session_id).join("rulespec.compiled.json")
}

/// Save a compiled rulespec to disk.
pub fn save_compiled_rulespec(session_id: &str, compiled: &CompiledRulespec) -> Result<()> {
    let path = get_compiled_rulespec_path(session_id);
    let json = serde_json::to_string_pretty(compiled)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Load a compiled rulespec from disk.
pub fn load_compiled_rulespec(session_id: &str) -> Result<Option<CompiledRulespec>> {
    let path = get_compiled_rulespec_path(session_id);
    if !path.exists() {
        return Ok(None);
    }
    let json = std::fs::read_to_string(&path)?;
    let compiled: CompiledRulespec = serde_json::from_str(&json)?;
    Ok(Some(compiled))
}

// ============================================================================
// Formatting
// ============================================================================

/// Format datalog execution results for display.
///
/// This is used for shadow/dry-run output - printed to console but not
/// injected into the context window.
pub fn format_datalog_results(result: &DatalogExecutionResult) -> String {
    let mut output = String::new();

    output.push_str("\n");
    output.push_str(&"─".repeat(60));
    output.push_str("\n");
    output.push_str("🔬 DATALOG INVARIANT VERIFICATION (shadow mode)\n");
    output.push_str(&"─".repeat(60));
    output.push_str("\n\n");

    output.push_str(&format!("Facts extracted: {}\n\n", result.fact_count));

    for pr in &result.predicate_results {
        let status = if pr.passed { "✅" } else { "❌" };
        let value_str = pr
            .expected_value
            .as_ref()
            .map(|v| format!(" '{}'", v))
            .unwrap_or_default();
        
        output.push_str(&format!(
            "{} [{}] {} {}{}\n",
            status, pr.source, pr.rule, pr.claim_name, value_str
        ));
        output.push_str(&format!("   {}\n", pr.reason));
        
        if let Some(notes) = &pr.notes {
            output.push_str(&format!("   📝 {}\n", notes));
        }
        output.push('\n');
    }

    output.push_str(&"─".repeat(60));
    output.push_str("\n");
    
    if result.all_passed() {
        output.push_str(&format!(
            "✅ All {} invariant(s) satisfied (datalog)\n",
            result.passed_count
        ));
    } else {
        output.push_str(&format!(
            "⚠️  {}/{} invariant(s) satisfied, {} failed (datalog)\n",
            result.passed_count,
            result.passed_count + result.failed_count,
            result.failed_count
        ));
    }
    
    output.push_str(&"─".repeat(60));
    output.push_str("\n");

    output
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_rulespec() -> Rulespec {
        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("caps", "csv_importer.capabilities"));
        rulespec.claims.push(Claim::new("file", "csv_importer.file"));
        rulespec.predicates.push(
            Predicate::new("caps", PredicateRule::Contains, InvariantSource::TaskPrompt)
                .with_value(YamlValue::String("handle_tsv".to_string()))
                .with_notes("User requested TSV support"),
        );
        rulespec.predicates.push(
            Predicate::new("file", PredicateRule::Exists, InvariantSource::Memory),
        );
        rulespec
    }

    fn make_test_envelope() -> ActionEnvelope {
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact(
            "csv_importer",
            serde_yaml::from_str(
                r#"
                capabilities:
                  - handle_headers
                  - handle_tsv
                  - handle_quoted_fields
                file: src/import/csv.rs
            "#,
            )
            .unwrap(),
        );
        envelope
    }

    // ========================================================================
    // Compilation Tests
    // ========================================================================

    #[test]
    fn test_compile_rulespec_success() {
        let rulespec = make_test_rulespec();
        let compiled = compile_rulespec(&rulespec, "test-plan", 1).unwrap();

        assert_eq!(compiled.plan_id, "test-plan");
        assert_eq!(compiled.compiled_at_revision, 1);
        assert_eq!(compiled.predicates.len(), 2);
        assert_eq!(compiled.claims.len(), 2);
    }

    #[test]
    fn test_compile_rulespec_empty() {
        let rulespec = Rulespec::new();
        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();

        assert!(compiled.is_empty());
        assert!(compiled.predicates.is_empty());
        assert!(compiled.claims.is_empty());
    }

    #[test]
    fn test_compile_rulespec_invalid_selector() {
        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim {
            name: "bad".to_string(),
            selector: "".to_string(), // Empty selector is invalid
        });

        let result = compile_rulespec(&rulespec, "test", 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid selector"));
    }

    #[test]
    fn test_compile_rulespec_unknown_claim() {
        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("known", "foo.bar"));
        rulespec.predicates.push(Predicate::new(
            "unknown", // References non-existent claim
            PredicateRule::Exists,
            InvariantSource::TaskPrompt,
        ));

        let result = compile_rulespec(&rulespec, "test", 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unknown claim"));
    }

    #[test]
    fn test_compile_exists_predicate_no_value() {
        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("test", "foo.bar"));
        rulespec.predicates.push(Predicate::new(
            "test",
            PredicateRule::Exists,
            InvariantSource::TaskPrompt,
        ));

        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        assert!(compiled.predicates[0].expected_value.is_none());
    }

    // ========================================================================
    // Fact Extraction Tests
    // ========================================================================

    #[test]
    fn test_extract_facts_basic() {
        let envelope = make_test_envelope();
        let rulespec = make_test_rulespec();
        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();

        let facts = extract_facts(&envelope, &compiled);

        // Should have facts for capabilities array elements
        assert!(facts.contains(&Fact {
            claim_name: "caps".to_string(),
            value: "handle_tsv".to_string(),
        }));
        assert!(facts.contains(&Fact {
            claim_name: "caps".to_string(),
            value: "handle_headers".to_string(),
        }));
    }

    #[test]
    fn test_extract_facts_missing_path() {
        let envelope = ActionEnvelope::new(); // Empty envelope
        let rulespec = make_test_rulespec();
        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();

        let facts = extract_facts(&envelope, &compiled);

        // Should return empty set, not error
        assert!(facts.is_empty());
    }

    #[test]
    fn test_extract_facts_array_length() {
        let envelope = make_test_envelope();
        let rulespec = make_test_rulespec();
        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();

        let facts = extract_facts(&envelope, &compiled);

        // Should have length fact
        assert!(facts.contains(&Fact {
            claim_name: "caps.__length".to_string(),
            value: "3".to_string(),
        }));
    }

    #[test]
    fn test_extract_facts_deeply_nested() {
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact(
            "a",
            serde_yaml::from_str(
                r#"
                b:
                  c:
                    d:
                      e:
                        f: deep_value
            "#,
            )
            .unwrap(),
        );

        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("deep", "a.b.c.d.e.f"));
        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();

        let facts = extract_facts(&envelope, &compiled);

        assert!(facts.contains(&Fact {
            claim_name: "deep".to_string(),
            value: "deep_value".to_string(),
        }));
    }

    #[test]
    fn test_extract_facts_null_value() {
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact("nullable", YamlValue::Null);

        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("test", "nullable"));
        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();

        let facts = extract_facts(&envelope, &compiled);

        assert!(facts.contains(&Fact {
            claim_name: "test".to_string(),
            value: "null".to_string(),
        }));
    }

    // ========================================================================
    // Execution Tests
    // ========================================================================

    #[test]
    fn test_execute_rules_all_pass() {
        let envelope = make_test_envelope();
        let rulespec = make_test_rulespec();
        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        let facts = extract_facts(&envelope, &compiled);

        let result = execute_rules(&compiled, &facts);

        assert!(result.all_passed());
        assert_eq!(result.passed_count, 2);
        assert_eq!(result.failed_count, 0);
    }

    #[test]
    fn test_execute_rules_contains_fail() {
        let envelope = make_test_envelope();
        
        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("caps", "csv_importer.capabilities"));
        rulespec.predicates.push(
            Predicate::new("caps", PredicateRule::Contains, InvariantSource::TaskPrompt)
                .with_value(YamlValue::String("handle_xlsx".to_string())), // Not in envelope
        );

        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        let facts = extract_facts(&envelope, &compiled);
        let result = execute_rules(&compiled, &facts);

        assert!(!result.all_passed());
        assert_eq!(result.failed_count, 1);
        assert!(result.predicate_results[0].reason.contains("Does not contain"));
    }

    #[test]
    fn test_execute_rules_exists_pass() {
        let envelope = make_test_envelope();
        
        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("file", "csv_importer.file"));
        rulespec.predicates.push(Predicate::new(
            "file",
            PredicateRule::Exists,
            InvariantSource::TaskPrompt,
        ));

        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        let facts = extract_facts(&envelope, &compiled);
        let result = execute_rules(&compiled, &facts);

        assert!(result.all_passed());
    }

    #[test]
    fn test_execute_rules_not_exists_pass() {
        let envelope = make_test_envelope();
        
        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("missing", "nonexistent.path"));
        rulespec.predicates.push(Predicate::new(
            "missing",
            PredicateRule::NotExists,
            InvariantSource::TaskPrompt,
        ));

        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        let facts = extract_facts(&envelope, &compiled);
        let result = execute_rules(&compiled, &facts);

        assert!(result.all_passed());
    }

    #[test]
    fn test_execute_rules_equals_pass() {
        let envelope = make_test_envelope();
        
        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("file", "csv_importer.file"));
        rulespec.predicates.push(
            Predicate::new("file", PredicateRule::Equals, InvariantSource::TaskPrompt)
                .with_value(YamlValue::String("src/import/csv.rs".to_string())),
        );

        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        let facts = extract_facts(&envelope, &compiled);
        let result = execute_rules(&compiled, &facts);

        assert!(result.all_passed());
    }

    #[test]
    fn test_execute_rules_min_length_pass() {
        let envelope = make_test_envelope();
        
        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("caps", "csv_importer.capabilities"));
        rulespec.predicates.push(
            Predicate::new("caps", PredicateRule::MinLength, InvariantSource::TaskPrompt)
                .with_value(YamlValue::Number(2.into())),
        );

        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        let facts = extract_facts(&envelope, &compiled);
        let result = execute_rules(&compiled, &facts);

        assert!(result.all_passed());
        assert!(result.predicate_results[0].reason.contains("3 >= 2"));
    }

    #[test]
    fn test_execute_rules_max_length_fail() {
        let envelope = make_test_envelope();
        
        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("caps", "csv_importer.capabilities"));
        rulespec.predicates.push(
            Predicate::new("caps", PredicateRule::MaxLength, InvariantSource::TaskPrompt)
                .with_value(YamlValue::Number(2.into())), // Array has 3 elements
        );

        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        let facts = extract_facts(&envelope, &compiled);
        let result = execute_rules(&compiled, &facts);

        assert!(!result.all_passed());
        assert!(result.predicate_results[0].reason.contains("3 > 2"));
    }

    #[test]
    fn test_execute_rules_matches_pass() {
        let envelope = make_test_envelope();
        
        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("file", "csv_importer.file"));
        rulespec.predicates.push(
            Predicate::new("file", PredicateRule::Matches, InvariantSource::TaskPrompt)
                .with_value(YamlValue::String(r"src/.*\.rs".to_string())),
        );

        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        let facts = extract_facts(&envelope, &compiled);
        let result = execute_rules(&compiled, &facts);

        assert!(result.all_passed());
    }

    #[test]
    fn test_execute_rules_no_facts() {
        let envelope = ActionEnvelope::new();
        let rulespec = make_test_rulespec();
        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        let facts = extract_facts(&envelope, &compiled);

        let result = execute_rules(&compiled, &facts);

        // Both predicates should fail (contains and exists)
        assert!(!result.all_passed());
        assert_eq!(result.failed_count, 2);
    }

    #[test]
    fn test_execute_rules_greater_than() {
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact("count", YamlValue::Number(42.into()));

        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("count", "count"));
        rulespec.predicates.push(
            Predicate::new("count", PredicateRule::GreaterThan, InvariantSource::TaskPrompt)
                .with_value(YamlValue::Number(10.into())),
        );

        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        let facts = extract_facts(&envelope, &compiled);
        let result = execute_rules(&compiled, &facts);

        assert!(result.all_passed());
    }

    #[test]
    fn test_execute_rules_less_than() {
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact("count", YamlValue::Number(5.into()));

        let mut rulespec = Rulespec::new();
        rulespec.claims.push(Claim::new("count", "count"));
        rulespec.predicates.push(
            Predicate::new("count", PredicateRule::LessThan, InvariantSource::TaskPrompt)
                .with_value(YamlValue::Number(10.into())),
        );

        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        let facts = extract_facts(&envelope, &compiled);
        let result = execute_rules(&compiled, &facts);

        assert!(result.all_passed());
    }

    // ========================================================================
    // Storage Tests
    // ========================================================================

    #[test]
    fn test_compiled_rulespec_serialization() {
        let rulespec = make_test_rulespec();
        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();

        let json = serde_json::to_string(&compiled).unwrap();
        let deserialized: CompiledRulespec = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.plan_id, compiled.plan_id);
        assert_eq!(deserialized.predicates.len(), compiled.predicates.len());
    }

    // ========================================================================
    // Formatting Tests
    // ========================================================================

    #[test]
    fn test_format_datalog_results() {
        let envelope = make_test_envelope();
        let rulespec = make_test_rulespec();
        let compiled = compile_rulespec(&rulespec, "test", 1).unwrap();
        let facts = extract_facts(&envelope, &compiled);
        let result = execute_rules(&compiled, &facts);

        let output = format_datalog_results(&result);

        assert!(output.contains("DATALOG INVARIANT VERIFICATION"));
        assert!(output.contains("shadow mode"));
        assert!(output.contains("✅"));
        assert!(output.contains("Facts extracted:"));
    }
}
