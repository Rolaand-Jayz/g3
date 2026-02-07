//! Invariants system for Plan Mode.
//!
//! This module implements:
//! - **Rulespec**: Machine-readable invariants with claims and predicates
//! - **ActionEnvelope**: Evidence of work done (facts about completed work)
//!
//! The rulespec is checked into `analysis/rulespec.yaml` and read at
//! plan verification time. The action envelope is written per-session
//! and verified against the rulespec.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_yaml::Value as YamlValue;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use crate::paths::get_session_logs_dir;

// ============================================================================
// Invariant Source
// ============================================================================

/// Source of an invariant - where it was extracted from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvariantSource {
    /// Extracted from the user's task prompt
    TaskPrompt,
    /// Extracted from persistent workspace memory (AGENTS.md, memory.md)
    Memory,
}

impl std::fmt::Display for InvariantSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvariantSource::TaskPrompt => write!(f, "task_prompt"),
            InvariantSource::Memory => write!(f, "memory"),
        }
    }
}

// ============================================================================
// Rulespec - Machine-readable invariants
// ============================================================================

/// A claim is a named selector over the action envelope.
/// 
/// Claims use a path-like selector syntax to reference values in the
/// action envelope. For example:
/// - `csv_importer.capabilities` - selects the capabilities array
/// - `csv_importer.file` - selects the file path string
/// - `tests[0]` - selects the first test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    /// Name of this claim (used by predicates to reference it)
    pub name: String,
    /// Selector path into the action envelope (e.g., "csv_importer.capabilities")
    pub selector: String,
}

impl Claim {
    pub fn new(name: impl Into<String>, selector: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            selector: selector.into(),
        }
    }

    /// Validate the claim structure.
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(anyhow!("Claim name cannot be empty"));
        }
        if self.selector.trim().is_empty() {
            return Err(anyhow!("Claim selector cannot be empty"));
        }
        // Basic selector syntax validation
        Selector::parse(&self.selector)?;
        Ok(())
    }
}

/// Predicate rule types for evaluating claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredicateRule {
    /// Value contains the specified element (for arrays) or substring (for strings)
    Contains,
    /// Value equals the specified value exactly
    Equals,
    /// Value exists (is not null/missing)
    Exists,
    /// Value does not exist (is null/missing)
    NotExists,
    /// Value is greater than the specified value
    GreaterThan,
    /// Value is less than the specified value
    LessThan,
    /// Array has at least N elements
    MinLength,
    /// Array has at most N elements
    MaxLength,
    /// Value matches a regex pattern
    Matches,
    /// Value does NOT contain the specified element (negation of Contains)
    NotContains,
    /// Value is one of the specified set of values
    AnyOf,
    /// Value is none of the specified set of values
    NoneOf,
}

impl std::fmt::Display for PredicateRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PredicateRule::Contains => write!(f, "contains"),
            PredicateRule::Equals => write!(f, "equals"),
            PredicateRule::Exists => write!(f, "exists"),
            PredicateRule::NotExists => write!(f, "not_exists"),
            PredicateRule::GreaterThan => write!(f, "greater_than"),
            PredicateRule::LessThan => write!(f, "less_than"),
            PredicateRule::MinLength => write!(f, "min_length"),
            PredicateRule::MaxLength => write!(f, "max_length"),
            PredicateRule::Matches => write!(f, "matches"),
            PredicateRule::NotContains => write!(f, "not_contains"),
            PredicateRule::AnyOf => write!(f, "any_of"),
            PredicateRule::NoneOf => write!(f, "none_of"),
        }
    }
}

/// A predicate defines a rule to evaluate against a claim's value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhenCondition {
    /// Name of the claim to check for the condition
    pub claim: String,
    /// The rule to apply for the condition check
    pub rule: PredicateRule,
    /// Value to compare against (optional, depends on rule)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<YamlValue>,
}

impl WhenCondition {
    pub fn new(claim: impl Into<String>, rule: PredicateRule) -> Self {
        Self {
            claim: claim.into(),
            rule,
            value: None,
        }
    }

    pub fn with_value(mut self, value: YamlValue) -> Self {
        self.value = Some(value);
        self
    }
}

/// A predicate defines a rule to evaluate against a claim's value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Predicate {
    /// Name of the claim this predicate evaluates
    pub claim: String,
    /// The rule to apply
    pub rule: PredicateRule,
    /// Value to compare against (interpretation depends on rule)
    /// For `exists`/`not_exists`, this can be omitted
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<YamlValue>,
    /// Source of this invariant (task_prompt or memory)
    pub source: InvariantSource,
    /// Optional notes explaining the invariant or providing nuance
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Optional condition that must be met for this predicate to be evaluated.
    /// If the condition is not met, the predicate is skipped (vacuous pass).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<WhenCondition>,
}

impl Predicate {
    pub fn new(
        claim: impl Into<String>,
        rule: PredicateRule,
        source: InvariantSource,
    ) -> Self {
        Self {
            claim: claim.into(),
            rule,
            value: None,
            source,
            notes: None,
            when: None,
        }
    }

    pub fn with_value(mut self, value: YamlValue) -> Self {
        self.value = Some(value);
        self
    }

    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    pub fn with_when(mut self, when: WhenCondition) -> Self {
        self.when = Some(when);
        self
    }

    /// Validate the predicate structure.
    pub fn validate(&self) -> Result<()> {
        if self.claim.trim().is_empty() {
            return Err(anyhow!("Predicate claim reference cannot be empty"));
        }
        
        // Some rules require a value
        match self.rule {
            PredicateRule::Exists | PredicateRule::NotExists => {
                // Value is optional for these
            }
            _ => {
                if self.value.is_none() {
                    return Err(anyhow!(
                        "Predicate rule '{}' requires a value",
                        self.rule
                    ));
                }
            }
        }
        
        // Validate when condition if present
        if let Some(when) = &self.when {
            if when.claim.trim().is_empty() {
                return Err(anyhow!("When condition claim reference cannot be empty"));
            }
            // When conditions that need a value must have one
            match when.rule {
                PredicateRule::Exists | PredicateRule::NotExists => {}
                _ => {
                    if when.value.is_none() {
                        return Err(anyhow!(
                            "When condition rule '{}' requires a value",
                            when.rule
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

/// A rulespec contains claims and predicates that define invariants.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Rulespec {
    /// Named claims (selectors over the action envelope)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claims: Vec<Claim>,
    /// Predicates that evaluate claims
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub predicates: Vec<Predicate>,
}

impl Rulespec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_claim(&mut self, claim: Claim) {
        self.claims.push(claim);
    }

    pub fn add_predicate(&mut self, predicate: Predicate) {
        self.predicates.push(predicate);
    }

    /// Validate the rulespec structure.
    pub fn validate(&self) -> Result<()> {
        // Validate all claims
        let mut claim_names = std::collections::HashSet::new();
        for claim in &self.claims {
            claim.validate()?;
            if !claim_names.insert(&claim.name) {
                return Err(anyhow!("Duplicate claim name: {}", claim.name));
            }
        }

        // Validate all predicates and check claim references
        for predicate in &self.predicates {
            predicate.validate()?;
            if !claim_names.contains(&predicate.claim) {
                return Err(anyhow!(
                    "Predicate references unknown claim: {}",
                    predicate.claim
                ));
            }
            // Validate when condition claim reference
            if let Some(when) = &predicate.when {
                if !claim_names.contains(&when.claim) {
                    return Err(anyhow!(
                        "When condition references unknown claim: {}",
                        when.claim
                    ));
                }
            }
        }

        Ok(())
    }

    /// Check if the rulespec is empty (no claims or predicates).
    pub fn is_empty(&self) -> bool {
        self.claims.is_empty() && self.predicates.is_empty()
    }
}

// ============================================================================
// ActionEnvelope - Evidence of work done
// ============================================================================

/// An action envelope contains facts about completed work.
/// 
/// Facts are organized as a flexible YAML structure that can contain:
/// - File paths modified
/// - Test names added
/// - Capabilities implemented
/// - Libraries added
/// - Algorithm locations
/// - Any other evidence of work
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActionEnvelope {
    /// Facts about the completed work (flexible YAML structure)
    #[serde(default)]
    pub facts: HashMap<String, YamlValue>,

    /// Verification token — set by the deterministic verification pipeline.
    /// Format: "g3v1:<base64>" — proves that rulespec evaluation passed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified: Option<String>,
}

impl ActionEnvelope {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a fact to the envelope.
    pub fn add_fact(&mut self, key: impl Into<String>, value: YamlValue) {
        self.facts.insert(key.into(), value);
    }

    /// Get a fact by key.
    pub fn get_fact(&self, key: &str) -> Option<&YamlValue> {
        self.facts.get(key)
    }

    /// Check if the envelope is empty.
    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }

    /// Convert the envelope to a YamlValue for selector evaluation.
    pub fn to_yaml_value(&self) -> YamlValue {
        // Wrap facts in a root object
        let mut root = serde_yaml::Mapping::new();
        for (key, value) in &self.facts {
            root.insert(YamlValue::String(key.clone()), value.clone());
        }
        YamlValue::Mapping(root)
    }
}

// ============================================================================
// Selector - Path-like selector for YAML values
// ============================================================================

/// A parsed selector for navigating YAML structures.
/// 
/// Supports:
/// - Dot notation: `foo.bar.baz`
/// - Array indexing: `foo[0]`, `foo[1].bar`
/// - Wildcards: `foo[*]` (all array elements)
#[derive(Debug, Clone)]
pub struct Selector {
    segments: Vec<SelectorSegment>,
}

#[derive(Debug, Clone)]
enum SelectorSegment {
    /// Access a named field
    Field(String),
    /// Access an array index
    Index(usize),
    /// Access all array elements (wildcard)
    Wildcard,
}

impl Selector {
    /// Parse a selector string into a Selector.
    /// 
    /// Examples:
    /// - `csv_importer.capabilities` -> [Field("csv_importer"), Field("capabilities")]
    /// - `tests[0].name` -> [Field("tests"), Index(0), Field("name")]
    /// - `items[*].id` -> [Field("items"), Wildcard, Field("id")]
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() {
            return Err(anyhow!("Selector cannot be empty"));
        }

        let mut segments = Vec::new();
        let mut current = String::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '.' => {
                    if !current.is_empty() {
                        segments.push(SelectorSegment::Field(current.clone()));
                        current.clear();
                    }
                }
                '[' => {
                    // Push any pending field
                    if !current.is_empty() {
                        segments.push(SelectorSegment::Field(current.clone()));
                        current.clear();
                    }
                    
                    // Parse index or wildcard
                    let mut index_str = String::new();
                    while let Some(&c) = chars.peek() {
                        if c == ']' {
                            chars.next();
                            break;
                        }
                        index_str.push(chars.next().unwrap());
                    }
                    
                    if index_str == "*" {
                        segments.push(SelectorSegment::Wildcard);
                    } else {
                        let index: usize = index_str.parse().map_err(|_| {
                            anyhow!("Invalid array index: {}", index_str)
                        })?;
                        segments.push(SelectorSegment::Index(index));
                    }
                }
                ']' => {
                    return Err(anyhow!("Unexpected ']' in selector"));
                }
                _ => {
                    current.push(c);
                }
            }
        }

        // Push any remaining field
        if !current.is_empty() {
            segments.push(SelectorSegment::Field(current));
        }

        if segments.is_empty() {
            return Err(anyhow!("Selector produced no segments"));
        }

        Ok(Self { segments })
    }

    /// Select values from a YAML value.
    /// 
    /// Returns a vector because wildcards can match multiple values.
    pub fn select(&self, value: &YamlValue) -> Vec<YamlValue> {
        self.select_recursive(value, 0)
    }

    fn select_recursive(&self, value: &YamlValue, segment_idx: usize) -> Vec<YamlValue> {
        if segment_idx >= self.segments.len() {
            return vec![value.clone()];
        }

        match &self.segments[segment_idx] {
            SelectorSegment::Field(name) => {
                if let YamlValue::Mapping(map) = value {
                    if let Some(v) = map.get(YamlValue::String(name.clone())) {
                        return self.select_recursive(v, segment_idx + 1);
                    }
                }
                vec![]
            }
            SelectorSegment::Index(idx) => {
                if let YamlValue::Sequence(seq) = value {
                    if let Some(v) = seq.get(*idx) {
                        return self.select_recursive(v, segment_idx + 1);
                    }
                }
                vec![]
            }
            SelectorSegment::Wildcard => {
                if let YamlValue::Sequence(seq) = value {
                    let mut results = Vec::new();
                    for item in seq {
                        results.extend(self.select_recursive(item, segment_idx + 1));
                    }
                    return results;
                }
                vec![]
            }
        }
    }

    /// Select a single value (returns None if no match or multiple matches).
    pub fn select_one(&self, value: &YamlValue) -> Option<YamlValue> {
        let results = self.select(value);
        if results.len() == 1 {
            Some(results.into_iter().next().unwrap())
        } else {
            None
        }
    }
}

// ============================================================================
// Predicate Evaluation
// ============================================================================

/// Result of evaluating a predicate.
#[derive(Debug, Clone)]
pub struct PredicateResult {
    /// Whether the predicate passed
    pub passed: bool,
    /// Human-readable explanation
    pub reason: String,
}

impl PredicateResult {
    pub fn pass(reason: impl Into<String>) -> Self {
        Self {
            passed: true,
            reason: reason.into(),
        }
    }

    pub fn fail(reason: impl Into<String>) -> Self {
        Self {
            passed: false,
            reason: reason.into(),
        }
    }
}

/// Evaluate a predicate against a claim's selected value.
pub fn evaluate_predicate(
    predicate: &Predicate,
    selected_values: &[YamlValue],
) -> PredicateResult {
    match predicate.rule {
        PredicateRule::Exists => {
            // Filter out null values — null means "absent"
            let non_null: Vec<_> = selected_values.iter()
                .filter(|v| !v.is_null())
                .collect();
            if non_null.is_empty() {
                PredicateResult::fail("Value does not exist")
            } else {
                PredicateResult::pass("Value exists")
            }
        }
        PredicateRule::NotExists => {
            // Filter out null values — null means "absent"
            let non_null: Vec<_> = selected_values.iter()
                .filter(|v| !v.is_null())
                .collect();
            if non_null.is_empty() {
                PredicateResult::pass("Value does not exist as expected")
            } else {
                PredicateResult::fail("Value exists but should not")
            }
        }
        PredicateRule::Contains => {
            let target = match &predicate.value {
                Some(v) => v,
                None => return PredicateResult::fail("No value specified for contains"),
            };
            
            for value in selected_values {
                if value_contains(value, target) {
                    return PredicateResult::pass(format!(
                        "Value contains {:?}",
                        yaml_to_display(target)
                    ));
                }
            }
            PredicateResult::fail(format!(
                "Value does not contain {:?}",
                yaml_to_display(target)
            ))
        }
        PredicateRule::Equals => {
            let target = match &predicate.value {
                Some(v) => v,
                None => return PredicateResult::fail("No value specified for equals"),
            };
            
            if selected_values.len() != 1 {
                return PredicateResult::fail(format!(
                    "Expected single value for equals, got {}",
                    selected_values.len()
                ));
            }
            
            if &selected_values[0] == target {
                PredicateResult::pass("Values are equal")
            } else {
                PredicateResult::fail(format!(
                    "Values not equal: {:?} != {:?}",
                    yaml_to_display(&selected_values[0]),
                    yaml_to_display(target)
                ))
            }
        }
        PredicateRule::MinLength => {
            let min = match &predicate.value {
                Some(YamlValue::Number(n)) => n.as_u64().unwrap_or(0) as usize,
                _ => return PredicateResult::fail("min_length requires a numeric value"),
            };
            
            for value in selected_values {
                if let YamlValue::Sequence(seq) = value {
                    if seq.len() >= min {
                        return PredicateResult::pass(format!(
                            "Array has {} elements (min: {})",
                            seq.len(),
                            min
                        ));
                    } else {
                        return PredicateResult::fail(format!(
                            "Array has {} elements (min: {})",
                            seq.len(),
                            min
                        ));
                    }
                }
            }
            PredicateResult::fail("Value is not an array")
        }
        PredicateRule::MaxLength => {
            let max = match &predicate.value {
                Some(YamlValue::Number(n)) => n.as_u64().unwrap_or(0) as usize,
                _ => return PredicateResult::fail("max_length requires a numeric value"),
            };
            
            for value in selected_values {
                if let YamlValue::Sequence(seq) = value {
                    if seq.len() <= max {
                        return PredicateResult::pass(format!(
                            "Array has {} elements (max: {})",
                            seq.len(),
                            max
                        ));
                    } else {
                        return PredicateResult::fail(format!(
                            "Array has {} elements (max: {})",
                            seq.len(),
                            max
                        ));
                    }
                }
            }
            PredicateResult::fail("Value is not an array")
        }
        PredicateRule::GreaterThan => {
            let target = match &predicate.value {
                Some(YamlValue::Number(n)) => n.as_f64().unwrap_or(0.0),
                _ => return PredicateResult::fail("greater_than requires a numeric value"),
            };
            
            for value in selected_values {
                if let YamlValue::Number(n) = value {
                    let v = n.as_f64().unwrap_or(0.0);
                    if v > target {
                        return PredicateResult::pass(format!("{} > {}", v, target));
                    } else {
                        return PredicateResult::fail(format!("{} is not > {}", v, target));
                    }
                }
            }
            PredicateResult::fail("Value is not a number")
        }
        PredicateRule::LessThan => {
            let target = match &predicate.value {
                Some(YamlValue::Number(n)) => n.as_f64().unwrap_or(0.0),
                _ => return PredicateResult::fail("less_than requires a numeric value"),
            };
            
            for value in selected_values {
                if let YamlValue::Number(n) = value {
                    let v = n.as_f64().unwrap_or(0.0);
                    if v < target {
                        return PredicateResult::pass(format!("{} < {}", v, target));
                    } else {
                        return PredicateResult::fail(format!("{} is not < {}", v, target));
                    }
                }
            }
            PredicateResult::fail("Value is not a number")
        }
        PredicateRule::Matches => {
            let pattern = match &predicate.value {
                Some(YamlValue::String(s)) => s,
                _ => return PredicateResult::fail("matches requires a string pattern"),
            };
            
            let regex = match regex::Regex::new(pattern) {
                Ok(r) => r,
                Err(e) => return PredicateResult::fail(format!("Invalid regex: {}", e)),
            };
            
            for value in selected_values {
                if let YamlValue::String(s) = value {
                    if regex.is_match(s) {
                        return PredicateResult::pass(format!("'{}' matches pattern", s));
                    }
                }
            }
            PredicateResult::fail(format!("No value matches pattern '{}'", pattern))
        }
        PredicateRule::NotContains => {
            let target = match &predicate.value {
                Some(v) => v,
                None => return PredicateResult::fail("No value specified for not_contains"),
            };
            
            for value in selected_values {
                if value_contains(value, target) {
                    return PredicateResult::fail(format!(
                        "Value contains {:?} but should not",
                        yaml_to_display(target)
                    ));
                }
            }
            PredicateResult::pass(format!(
                "Value does not contain {:?}",
                yaml_to_display(target)
            ))
        }
        PredicateRule::AnyOf => {
            let allowed = match &predicate.value {
                Some(YamlValue::Sequence(seq)) => seq,
                Some(_) => return PredicateResult::fail("any_of requires an array value"),
                None => return PredicateResult::fail("No value specified for any_of"),
            };
            
            for value in selected_values {
                if allowed.contains(value) {
                    return PredicateResult::pass(format!(
                        "Value {:?} is in allowed set",
                        yaml_to_display(value)
                    ));
                }
            }
            PredicateResult::fail(format!(
                "Value is not in allowed set [{}]",
                allowed.iter().map(yaml_to_display).collect::<Vec<_>>().join(", ")
            ))
        }
        PredicateRule::NoneOf => {
            let forbidden = match &predicate.value {
                Some(YamlValue::Sequence(seq)) => seq,
                Some(_) => return PredicateResult::fail("none_of requires an array value"),
                None => return PredicateResult::fail("No value specified for none_of"),
            };
            
            for value in selected_values {
                if forbidden.contains(value) {
                    return PredicateResult::fail(format!(
                        "Value {:?} is in forbidden set",
                        yaml_to_display(value)
                    ));
                }
            }
            PredicateResult::pass(format!(
                "Value is not in forbidden set [{}]",
                forbidden.iter().map(yaml_to_display).collect::<Vec<_>>().join(", ")
            ))
        }
    }
}

/// Check if a YAML value contains another value.
fn value_contains(haystack: &YamlValue, needle: &YamlValue) -> bool {
    match haystack {
        YamlValue::Sequence(seq) => {
            // Check if array contains the needle
            seq.iter().any(|item| item == needle)
        }
        YamlValue::String(s) => {
            // Check if string contains the needle (if needle is also a string)
            if let YamlValue::String(needle_str) = needle {
                s.contains(needle_str.as_str())
            } else {
                false
            }
        }
        YamlValue::Mapping(map) => {
            // Check if map contains the needle as a value
            map.values().any(|v| v == needle)
        }
        _ => haystack == needle,
    }
}

/// Convert a YAML value to a display string.
fn yaml_to_display(value: &YamlValue) -> String {
    match value {
        YamlValue::Null => "null".to_string(),
        YamlValue::Bool(b) => b.to_string(),
        YamlValue::Number(n) => n.to_string(),
        YamlValue::String(s) => s.clone(),
        YamlValue::Sequence(_) => "[array]".to_string(),
        YamlValue::Mapping(_) => "{object}".to_string(),
        YamlValue::Tagged(t) => format!("!{} ...", t.tag),
    }
}

// ============================================================================
// File Storage
// ============================================================================

/// Get the path to the envelope.yaml file for a session.
pub fn get_envelope_path(session_id: &str) -> PathBuf {
    get_session_logs_dir(session_id).join("envelope.yaml")
}

/// Read a rulespec from `analysis/rulespec.yaml` relative to the working directory.
pub fn read_rulespec(working_dir: &Path) -> Result<Option<Rulespec>> {
    let path = working_dir.join("analysis").join("rulespec.yaml");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let rulespec: Rulespec = serde_yaml::from_str(&content)?;
    Ok(Some(rulespec))
}

/// Read an action envelope from the session's envelope.yaml file.
pub fn read_envelope(session_id: &str) -> Result<Option<ActionEnvelope>> {
    let path = get_envelope_path(session_id);
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let envelope: ActionEnvelope = serde_yaml::from_str(&content)?;
    Ok(Some(envelope))
}

/// Write an action envelope to the session's envelope.yaml file.
pub fn write_envelope(session_id: &str, envelope: &ActionEnvelope) -> Result<()> {
    let path = get_envelope_path(session_id);
    let content = format_envelope_yaml(envelope);
    std::fs::write(&path, content)?;
    Ok(())
}

/// Format an action envelope as pretty YAML with comments.
fn format_envelope_yaml(envelope: &ActionEnvelope) -> String {
    let mut output = String::new();
    output.push_str("# Action Envelope - Evidence of work done\n");
    output.push_str("# Generated by g3 Plan Mode\n\n");
    
    let yaml = serde_yaml::to_string(envelope)
        .unwrap_or_else(|_| "# Error serializing envelope".to_string());
    output.push_str(&yaml);
    
    output
}

// ============================================================================
// Rulespec Evaluation
// ============================================================================

/// Result of evaluating a single predicate in context.
#[derive(Debug, Clone)]
pub struct PredicateEvaluation {
    /// The predicate that was evaluated
    pub predicate: Predicate,
    /// The claim name
    pub claim_name: String,
    /// Values selected by the claim
    pub selected_values: Vec<YamlValue>,
    /// Result of evaluation
    pub result: PredicateResult,
}

/// Result of evaluating an entire rulespec against an envelope.
#[derive(Debug, Clone)]
pub struct RulespecEvaluation {
    /// Results for each predicate
    pub predicate_results: Vec<PredicateEvaluation>,
    /// Number of predicates that passed
    pub passed_count: usize,
    /// Number of predicates that failed
    pub failed_count: usize,
}

impl RulespecEvaluation {
    /// Check if all predicates passed.
    pub fn all_passed(&self) -> bool {
        self.failed_count == 0
    }
}

/// Evaluate a rulespec against an action envelope.
pub fn evaluate_rulespec(rulespec: &Rulespec, envelope: &ActionEnvelope) -> RulespecEvaluation {
    let envelope_value = envelope.to_yaml_value();
    let mut predicate_results = Vec::new();
    let mut passed_count = 0;
    let mut failed_count = 0;

    // Build claim lookup
    let claims: HashMap<&str, &Claim> = rulespec
        .claims
        .iter()
        .map(|c| (c.name.as_str(), c))
        .collect();

    for predicate in &rulespec.predicates {
        // Check when condition — if present and not met, skip (vacuous pass)
        if let Some(when) = &predicate.when {
            let when_claim = claims.get(when.claim.as_str());
            let when_met = match when_claim {
                Some(claim) => {
                    match Selector::parse(&claim.selector) {
                        Ok(selector) => {
                            let when_values = selector.select(&envelope_value);
                            let when_pred = Predicate {
                                claim: when.claim.clone(),
                                rule: when.rule.clone(),
                                value: when.value.clone(),
                                source: predicate.source,
                                notes: None,
                                when: None,
                            };
                            evaluate_predicate(&when_pred, &when_values).passed
                        }
                        Err(_) => false,
                    }
                }
                None => false,
            };
            if !when_met {
                passed_count += 1;
                predicate_results.push(PredicateEvaluation {
                    predicate: predicate.clone(),
                    claim_name: predicate.claim.clone(),
                    selected_values: vec![],
                    result: PredicateResult::pass("Skipped (when condition not met)"),
                });
                continue;
            }
        }

        let claim = claims.get(predicate.claim.as_str());
        
        let (selected_values, result) = match claim {
            Some(claim) => {
                match Selector::parse(&claim.selector) {
                    Ok(selector) => {
                        let values = selector.select(&envelope_value);
                        let result = evaluate_predicate(predicate, &values);
                        (values, result)
                    }
                    Err(e) => {
                        (vec![], PredicateResult::fail(format!("Invalid selector: {}", e)))
                    }
                }
            }
            None => (vec![], PredicateResult::fail(format!(
                "Unknown claim: {}",
                predicate.claim
            ))),
        };

        if result.passed {
            passed_count += 1;
        } else {
            failed_count += 1;
        }

        predicate_results.push(PredicateEvaluation {
            predicate: predicate.clone(),
            claim_name: predicate.claim.clone(),
            selected_values,
            result,
        });
    }

    RulespecEvaluation {
        predicate_results,
        passed_count,
        failed_count,
    }
}

/// Format rulespec evaluation results for display.
pub fn format_evaluation_results(eval: &RulespecEvaluation) -> String {
    let mut output = String::new();
    
    output.push_str("\n");
    output.push_str(&"─".repeat(60));
    output.push_str("\n");
    output.push_str("📜 INVARIANT VERIFICATION\n");
    output.push_str(&"─".repeat(60));
    output.push_str("\n\n");

    for pe in &eval.predicate_results {
        let status = if pe.result.passed { "✅" } else { "❌" };
        output.push_str(&format!(
            "{} [{}] {} {:?}\n",
            status,
            pe.predicate.source,
            pe.predicate.rule,
            pe.claim_name
        ));
        output.push_str(&format!("   {}\n", pe.result.reason));
        if let Some(notes) = &pe.predicate.notes {
            output.push_str(&format!("   📝 {}\n", notes));
        }
        output.push('\n');
    }

    output.push_str(&"─".repeat(60));
    output.push_str("\n");
    if eval.all_passed() {
        output.push_str(&format!(
            "✅ All {} invariant(s) satisfied\n",
            eval.passed_count
        ));
    } else {
        output.push_str(&format!(
            "⚠️  {}/{} invariant(s) satisfied, {} failed\n",
            eval.passed_count,
            eval.passed_count + eval.failed_count,
            eval.failed_count
        ));
    }
    output.push_str(&"─".repeat(60));
    output.push_str("\n");

    output
}

/// Format an action envelope as human-readable markdown.
/// 
/// This produces a rich, readable format suitable for tool output,
/// showing the facts recorded about completed work.
pub fn format_envelope_markdown(envelope: &ActionEnvelope) -> String {
    let mut output = String::new();
    
    output.push_str("\n");
    output.push_str("### Action Envelope\n\n");
    
    if envelope.facts.is_empty() {
        output.push_str("_No facts recorded._\n");
        return output;
    }
    
    // Sort facts by key for consistent output
    let mut keys: Vec<_> = envelope.facts.keys().collect();
    keys.sort();
    
    for key in keys {
        if let Some(value) = envelope.facts.get(key) {
            output.push_str(&format!("**{}**:\n", key));
            format_yaml_value_markdown(&mut output, value, 0);
            output.push_str("\n");
        }
    }
    
    output
}

/// Format a YAML value as indented markdown.
fn format_yaml_value_markdown(output: &mut String, value: &YamlValue, indent: usize) {
    let prefix = "  ".repeat(indent);
    match value {
        YamlValue::Null => output.push_str(&format!("{}  - _null_\n", prefix)),
        YamlValue::Bool(b) => output.push_str(&format!("{}  - `{}`\n", prefix, b)),
        YamlValue::Number(n) => output.push_str(&format!("{}  - `{}`\n", prefix, n)),
        YamlValue::String(s) => output.push_str(&format!("{}  - `{}`\n", prefix, s)),
        YamlValue::Sequence(seq) => {
            for item in seq {
                match item {
                    YamlValue::String(s) => output.push_str(&format!("{}  - `{}`\n", prefix, s)),
                    YamlValue::Number(n) => output.push_str(&format!("{}  - `{}`\n", prefix, n)),
                    YamlValue::Bool(b) => output.push_str(&format!("{}  - `{}`\n", prefix, b)),
                    _ => format_yaml_value_markdown(output, item, indent + 1),
                }
            }
        }
        YamlValue::Mapping(map) => {
            for (k, v) in map {
                let key_str = yaml_to_display(k);
                match v {
                    YamlValue::String(s) => output.push_str(&format!("{}  - {}: `{}`\n", prefix, key_str, s)),
                    YamlValue::Number(n) => output.push_str(&format!("{}  - {}: `{}`\n", prefix, key_str, n)),
                    YamlValue::Bool(b) => output.push_str(&format!("{}  - {}: `{}`\n", prefix, key_str, b)),
                    YamlValue::Null => output.push_str(&format!("{}  - {}: _null_\n", prefix, key_str)),
                    YamlValue::Sequence(_) | YamlValue::Mapping(_) => {
                        output.push_str(&format!("{}  - {}:\n", prefix, key_str));
                        format_yaml_value_markdown(output, v, indent + 2);
                    }
                    YamlValue::Tagged(t) => output.push_str(&format!("{}  - {}: !{} ...\n", prefix, key_str, t.tag)),
                }
            }
        }
        YamlValue::Tagged(t) => output.push_str(&format!("{}  - !{} ...\n", prefix, t.tag)),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Selector Tests
    // ========================================================================

    #[test]
    fn test_selector_parse_simple_field() {
        let selector = Selector::parse("foo").unwrap();
        assert_eq!(selector.segments.len(), 1);
    }

    #[test]
    fn test_selector_parse_nested_fields() {
        let selector = Selector::parse("foo.bar.baz").unwrap();
        assert_eq!(selector.segments.len(), 3);
    }

    #[test]
    fn test_selector_parse_array_index() {
        let selector = Selector::parse("foo[0]").unwrap();
        assert_eq!(selector.segments.len(), 2);
    }

    #[test]
    fn test_selector_parse_wildcard() {
        let selector = Selector::parse("items[*].id").unwrap();
        assert_eq!(selector.segments.len(), 3);
    }

    #[test]
    fn test_selector_parse_empty_fails() {
        assert!(Selector::parse("").is_err());
        assert!(Selector::parse("   ").is_err());
    }

    #[test]
    fn test_selector_select_simple() {
        let yaml: YamlValue = serde_yaml::from_str(r#"
            foo: bar
        "#).unwrap();
        
        let selector = Selector::parse("foo").unwrap();
        let results = selector.select(&yaml);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], YamlValue::String("bar".to_string()));
    }

    #[test]
    fn test_selector_select_nested() {
        let yaml: YamlValue = serde_yaml::from_str(r#"
            csv_importer:
              capabilities:
                - handle_headers
                - handle_tsv
        "#).unwrap();
        
        let selector = Selector::parse("csv_importer.capabilities").unwrap();
        let results = selector.select(&yaml);
        assert_eq!(results.len(), 1);
        if let YamlValue::Sequence(seq) = &results[0] {
            assert_eq!(seq.len(), 2);
        } else {
            panic!("Expected sequence");
        }
    }

    #[test]
    fn test_selector_select_array_index() {
        let yaml: YamlValue = serde_yaml::from_str(r#"
            items:
              - name: first
              - name: second
        "#).unwrap();
        
        let selector = Selector::parse("items[1].name").unwrap();
        let results = selector.select(&yaml);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], YamlValue::String("second".to_string()));
    }

    #[test]
    fn test_selector_select_wildcard() {
        let yaml: YamlValue = serde_yaml::from_str(r#"
            items:
              - id: 1
              - id: 2
              - id: 3
        "#).unwrap();
        
        let selector = Selector::parse("items[*].id").unwrap();
        let results = selector.select(&yaml);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_selector_select_missing_path() {
        let yaml: YamlValue = serde_yaml::from_str(r#"
            foo: bar
        "#).unwrap();
        
        let selector = Selector::parse("nonexistent.path").unwrap();
        let results = selector.select(&yaml);
        assert!(results.is_empty());
    }

    // ========================================================================
    // Predicate Tests
    // ========================================================================

    #[test]
    fn test_predicate_exists() {
        let predicate = Predicate::new("test", PredicateRule::Exists, InvariantSource::TaskPrompt);
        
        let result = evaluate_predicate(&predicate, &[YamlValue::String("value".to_string())]);
        assert!(result.passed);
        
        let result = evaluate_predicate(&predicate, &[]);
        assert!(!result.passed);
    }

    #[test]
    fn test_predicate_not_exists() {
        let predicate = Predicate::new("test", PredicateRule::NotExists, InvariantSource::TaskPrompt);
        
        let result = evaluate_predicate(&predicate, &[]);
        assert!(result.passed);
        
        let result = evaluate_predicate(&predicate, &[YamlValue::String("value".to_string())]);
        assert!(!result.passed);
    }

    #[test]
    fn test_predicate_contains_array() {
        let predicate = Predicate::new("test", PredicateRule::Contains, InvariantSource::TaskPrompt)
            .with_value(YamlValue::String("handle_tsv".to_string()));
        
        let array = YamlValue::Sequence(vec![
            YamlValue::String("handle_headers".to_string()),
            YamlValue::String("handle_tsv".to_string()),
        ]);
        
        let result = evaluate_predicate(&predicate, &[array]);
        assert!(result.passed);
    }

    #[test]
    fn test_predicate_contains_string() {
        let predicate = Predicate::new("test", PredicateRule::Contains, InvariantSource::TaskPrompt)
            .with_value(YamlValue::String("csv".to_string()));
        
        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("csv_importer".to_string())],
        );
        assert!(result.passed);
    }

    #[test]
    fn test_predicate_equals() {
        let predicate = Predicate::new("test", PredicateRule::Equals, InvariantSource::Memory)
            .with_value(YamlValue::String("expected".to_string()));
        
        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("expected".to_string())],
        );
        assert!(result.passed);
        
        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("different".to_string())],
        );
        assert!(!result.passed);
    }

    #[test]
    fn test_predicate_min_length() {
        let predicate = Predicate::new("test", PredicateRule::MinLength, InvariantSource::TaskPrompt)
            .with_value(YamlValue::Number(2.into()));
        
        let array = YamlValue::Sequence(vec![
            YamlValue::String("a".to_string()),
            YamlValue::String("b".to_string()),
            YamlValue::String("c".to_string()),
        ]);
        
        let result = evaluate_predicate(&predicate, &[array]);
        assert!(result.passed);
    }

    // ========================================================================
    // Rulespec Tests
    // ========================================================================

    #[test]
    fn test_rulespec_validation() {
        let mut rulespec = Rulespec::new();
        
        // Empty rulespec is valid
        assert!(rulespec.validate().is_ok());
        
        // Add a claim
        rulespec.add_claim(Claim::new("caps", "csv_importer.capabilities"));
        assert!(rulespec.validate().is_ok());
        
        // Add a predicate referencing the claim
        rulespec.add_predicate(
            Predicate::new("caps", PredicateRule::Exists, InvariantSource::TaskPrompt)
        );
        assert!(rulespec.validate().is_ok());
        
        // Add a predicate referencing unknown claim
        rulespec.add_predicate(
            Predicate::new("unknown", PredicateRule::Exists, InvariantSource::TaskPrompt)
        );
        assert!(rulespec.validate().is_err());
    }

    #[test]
    fn test_rulespec_duplicate_claim_names() {
        let mut rulespec = Rulespec::new();
        rulespec.add_claim(Claim::new("test", "foo"));
        rulespec.add_claim(Claim::new("test", "bar")); // Duplicate name
        
        assert!(rulespec.validate().is_err());
    }

    // ========================================================================
    // ActionEnvelope Tests
    // ========================================================================

    #[test]
    fn test_envelope_to_yaml_value() {
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact(
            "csv_importer",
            serde_yaml::from_str(r#"
                capabilities:
                  - handle_headers
                  - handle_tsv
                file: src/import/csv.rs
            "#).unwrap(),
        );
        
        let yaml = envelope.to_yaml_value();
        let selector = Selector::parse("csv_importer.capabilities").unwrap();
        let results = selector.select(&yaml);
        
        assert_eq!(results.len(), 1);
        if let YamlValue::Sequence(seq) = &results[0] {
            assert_eq!(seq.len(), 2);
        } else {
            panic!("Expected sequence");
        }
    }

    // ========================================================================
    // Full Evaluation Tests
    // ========================================================================

    #[test]
    fn test_evaluate_rulespec() {
        let mut rulespec = Rulespec::new();
        rulespec.add_claim(Claim::new("caps", "csv_importer.capabilities"));
        rulespec.add_predicate(
            Predicate::new("caps", PredicateRule::Contains, InvariantSource::TaskPrompt)
                .with_value(YamlValue::String("handle_tsv".to_string()))
                .with_notes("User requested TSV support")
        );
        
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact(
            "csv_importer",
            serde_yaml::from_str(r#"
                capabilities:
                  - handle_headers
                  - handle_tsv
            "#).unwrap(),
        );
        
        let eval = evaluate_rulespec(&rulespec, &envelope);
        assert!(eval.all_passed());
        assert_eq!(eval.passed_count, 1);
        assert_eq!(eval.failed_count, 0);
    }

    #[test]
    fn test_evaluate_rulespec_failure() {
        let mut rulespec = Rulespec::new();
        rulespec.add_claim(Claim::new("caps", "csv_importer.capabilities"));
        rulespec.add_predicate(
            Predicate::new("caps", PredicateRule::Contains, InvariantSource::TaskPrompt)
                .with_value(YamlValue::String("handle_xlsx".to_string()))
        );
        
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact(
            "csv_importer",
            serde_yaml::from_str(r#"
                capabilities:
                  - handle_headers
                  - handle_tsv
            "#).unwrap(),
        );
        
        let eval = evaluate_rulespec(&rulespec, &envelope);
        assert!(!eval.all_passed());
        assert_eq!(eval.passed_count, 0);
        assert_eq!(eval.failed_count, 1);
    }

    // ========================================================================
    // Serialization Tests
    // ========================================================================

    #[test]
    fn test_rulespec_yaml_roundtrip() {
        let mut rulespec = Rulespec::new();
        rulespec.add_claim(Claim::new("caps", "csv_importer.capabilities"));
        rulespec.add_predicate(
            Predicate::new("caps", PredicateRule::Contains, InvariantSource::TaskPrompt)
                .with_value(YamlValue::String("handle_tsv".to_string()))
                .with_notes("User requested TSV support")
        );
        
        let yaml = serde_yaml::to_string(&rulespec).unwrap();
        let parsed: Rulespec = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(parsed.claims.len(), 1);
        assert_eq!(parsed.predicates.len(), 1);
        assert_eq!(parsed.predicates[0].source, InvariantSource::TaskPrompt);
        assert_eq!(parsed.predicates[0].notes, Some("User requested TSV support".to_string()));
    }

    #[test]
    fn test_envelope_yaml_roundtrip() {
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact("test_key", YamlValue::String("test_value".to_string()));
        
        let yaml = serde_yaml::to_string(&envelope).unwrap();
        let parsed: ActionEnvelope = serde_yaml::from_str(&yaml).unwrap();
        
        assert_eq!(parsed.facts.len(), 1);
        assert!(parsed.facts.contains_key("test_key"));
    }

    #[test]
    fn test_empty_rulespec_serializes() {
        let rulespec = Rulespec::new();
        let yaml = serde_yaml::to_string(&rulespec).unwrap();
        // Empty rulespec should serialize (may be {} or empty fields)
        
        // Should deserialize back
        let _: Rulespec = serde_yaml::from_str(&yaml).unwrap();
    }

    #[test]
    fn test_empty_envelope_serializes() {
        let envelope = ActionEnvelope::new();
        let yaml = serde_yaml::to_string(&envelope).unwrap();
        
        // Should deserialize back
        let _: ActionEnvelope = serde_yaml::from_str(&yaml).unwrap();
    }

    // ========================================================================
    // Format Rulespec Markdown Tests
        // ========================================================================
    // Format Envelope Markdown Tests
    // ========================================================================

    #[test]
    fn test_format_envelope_markdown_empty() {
        let envelope = ActionEnvelope::new();
        let output = format_envelope_markdown(&envelope);
        
        assert!(output.contains("### Action Envelope"));
        assert!(output.contains("_No facts recorded._"));
    }

    #[test]
    fn test_format_envelope_markdown_with_facts() {
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact(
            "csv_importer",
            serde_yaml::from_str(r#"
                capabilities:
                  - handle_headers
                  - handle_tsv
                file: src/import/csv.rs
            "#).unwrap(),
        );
        
        let output = format_envelope_markdown(&envelope);
        
        assert!(output.contains("### Action Envelope"));
        assert!(output.contains("**csv_importer**:"));
        assert!(output.contains("`handle_headers`"));
        assert!(output.contains("`handle_tsv`"));
        assert!(output.contains("`src/import/csv.rs`"));
    }

    #[test]
    fn test_format_envelope_markdown_with_null_value() {
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact("breaking_changes", YamlValue::Null);
        
        let output = format_envelope_markdown(&envelope);
        
        assert!(output.contains("### Action Envelope"));
        assert!(output.contains("**breaking_changes**:"));
        assert!(output.contains("_null_"));
    }

    // ========================================================================
    // ActionEnvelope Deserialization Validation Tests
    // ========================================================================

    #[test]
    fn test_envelope_deser_with_facts_key() {
        // Valid: YAML with facts: top-level key
        let yaml = r#"
facts:
  csv_importer:
    capabilities:
      - handle_headers
      - handle_tsv
    file: src/import/csv.rs
"#;
        let envelope: ActionEnvelope = serde_yaml::from_str(yaml).unwrap();
        assert!(!envelope.facts.is_empty());
        assert!(envelope.facts.contains_key("csv_importer"));
    }

    #[test]
    fn test_envelope_deser_without_facts_key_is_empty() {
        // Bug scenario: YAML without facts: wrapper silently produces empty facts
        let yaml = r#"
csv_importer:
  capabilities:
    - handle_headers
    - handle_tsv
  file: src/import/csv.rs
"#;
        let envelope: ActionEnvelope = serde_yaml::from_str(yaml).unwrap();
        // serde silently ignores unknown fields, facts defaults to empty
        assert!(envelope.facts.is_empty(), "Expected empty facts when 'facts:' key is missing");
    }

    #[test]
    fn test_envelope_deser_empty_facts_is_empty() {
        let yaml = "facts: {}";
        let envelope: ActionEnvelope = serde_yaml::from_str(yaml).unwrap();
        assert!(envelope.facts.is_empty());
    }

    #[test]
    fn test_envelope_deser_facts_with_null_values() {
        let yaml = r#"
facts:
  breaking_changes: null
  csv_importer:
    file: src/import/csv.rs
"#;
        let envelope: ActionEnvelope = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(envelope.facts.len(), 2);
        assert!(envelope.facts.contains_key("breaking_changes"));
        assert_eq!(envelope.facts.get("breaking_changes").unwrap(), &YamlValue::Null);
        assert!(envelope.facts.contains_key("csv_importer"));
    }

    #[test]
    fn test_envelope_deser_facts_single_key() {
        let yaml = r#"
facts:
  my_feature:
    file: src/my_feature.rs
"#;
        let envelope: ActionEnvelope = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(envelope.facts.len(), 1);
        assert!(envelope.facts.contains_key("my_feature"));
    }

    #[test]
    fn test_envelope_roundtrip_preserves_null_facts() {
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact("breaking_changes", YamlValue::Null);
        envelope.add_fact("feature", YamlValue::String("done".to_string()));

        let yaml = serde_yaml::to_string(&envelope).unwrap();
        let parsed: ActionEnvelope = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(parsed.facts.len(), 2);
        assert_eq!(parsed.facts.get("breaking_changes").unwrap(), &YamlValue::Null);
        assert_eq!(parsed.facts.get("feature").unwrap(), &YamlValue::String("done".to_string()));
    }

    // ========================================================================
    // New Predicate Rules: NotContains, AnyOf, NoneOf
    // ========================================================================

    #[test]
    fn test_predicate_not_contains_array_pass() {
        let predicate = Predicate::new("test", PredicateRule::NotContains, InvariantSource::TaskPrompt)
            .with_value(YamlValue::String("deprecated".to_string()));

        let array = YamlValue::Sequence(vec![
            YamlValue::String("handle_csv".to_string()),
            YamlValue::String("handle_tsv".to_string()),
        ]);

        let result = evaluate_predicate(&predicate, &[array]);
        assert!(result.passed, "not_contains should pass when element is absent");
    }

    #[test]
    fn test_predicate_not_contains_array_fail() {
        let predicate = Predicate::new("test", PredicateRule::NotContains, InvariantSource::TaskPrompt)
            .with_value(YamlValue::String("handle_csv".to_string()));

        let array = YamlValue::Sequence(vec![
            YamlValue::String("handle_csv".to_string()),
            YamlValue::String("handle_tsv".to_string()),
        ]);

        let result = evaluate_predicate(&predicate, &[array]);
        assert!(!result.passed, "not_contains should fail when element is present");
    }

    #[test]
    fn test_predicate_not_contains_string_pass() {
        let predicate = Predicate::new("test", PredicateRule::NotContains, InvariantSource::TaskPrompt)
            .with_value(YamlValue::String("xml".to_string()));

        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("csv_importer".to_string())],
        );
        assert!(result.passed, "not_contains should pass when substring is absent");
    }

    #[test]
    fn test_predicate_not_contains_string_fail() {
        let predicate = Predicate::new("test", PredicateRule::NotContains, InvariantSource::TaskPrompt)
            .with_value(YamlValue::String("csv".to_string()));

        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("csv_importer".to_string())],
        );
        assert!(!result.passed, "not_contains should fail when substring is present");
    }

    #[test]
    fn test_predicate_not_contains_empty_array() {
        let predicate = Predicate::new("test", PredicateRule::NotContains, InvariantSource::TaskPrompt)
            .with_value(YamlValue::String("anything".to_string()));

        let array = YamlValue::Sequence(vec![]);
        let result = evaluate_predicate(&predicate, &[array]);
        assert!(result.passed, "not_contains on empty array should pass");
    }

    #[test]
    fn test_predicate_any_of_pass() {
        let predicate = Predicate::new("test", PredicateRule::AnyOf, InvariantSource::TaskPrompt)
            .with_value(YamlValue::Sequence(vec![
                YamlValue::String("json".to_string()),
                YamlValue::String("yaml".to_string()),
                YamlValue::String("toml".to_string()),
            ]));

        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("yaml".to_string())],
        );
        assert!(result.passed, "any_of should pass when value is in set");
    }

    #[test]
    fn test_predicate_any_of_fail() {
        let predicate = Predicate::new("test", PredicateRule::AnyOf, InvariantSource::TaskPrompt)
            .with_value(YamlValue::Sequence(vec![
                YamlValue::String("json".to_string()),
                YamlValue::String("yaml".to_string()),
            ]));

        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("xml".to_string())],
        );
        assert!(!result.passed, "any_of should fail when value is not in set");
    }

    #[test]
    fn test_predicate_any_of_non_array_value_fails() {
        let predicate = Predicate::new("test", PredicateRule::AnyOf, InvariantSource::TaskPrompt)
            .with_value(YamlValue::String("not_an_array".to_string()));

        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("anything".to_string())],
        );
        assert!(!result.passed, "any_of with non-array value should fail");
    }

    #[test]
    fn test_predicate_any_of_single_element() {
        let predicate = Predicate::new("test", PredicateRule::AnyOf, InvariantSource::TaskPrompt)
            .with_value(YamlValue::Sequence(vec![
                YamlValue::String("only".to_string()),
            ]));

        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("only".to_string())],
        );
        assert!(result.passed, "any_of with single-element set should work");
    }

    #[test]
    fn test_predicate_none_of_pass() {
        let predicate = Predicate::new("test", PredicateRule::NoneOf, InvariantSource::TaskPrompt)
            .with_value(YamlValue::Sequence(vec![
                YamlValue::String("xml".to_string()),
                YamlValue::String("csv".to_string()),
            ]));

        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("json".to_string())],
        );
        assert!(result.passed, "none_of should pass when value is not in forbidden set");
    }

    #[test]
    fn test_predicate_none_of_fail() {
        let predicate = Predicate::new("test", PredicateRule::NoneOf, InvariantSource::TaskPrompt)
            .with_value(YamlValue::Sequence(vec![
                YamlValue::String("xml".to_string()),
                YamlValue::String("csv".to_string()),
            ]));

        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("xml".to_string())],
        );
        assert!(!result.passed, "none_of should fail when value is in forbidden set");
    }

    #[test]
    fn test_predicate_none_of_non_array_value_fails() {
        let predicate = Predicate::new("test", PredicateRule::NoneOf, InvariantSource::TaskPrompt)
            .with_value(YamlValue::String("not_an_array".to_string()));

        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("anything".to_string())],
        );
        assert!(!result.passed, "none_of with non-array value should fail");
    }

    #[test]
    fn test_predicate_none_of_empty_forbidden_set() {
        let predicate = Predicate::new("test", PredicateRule::NoneOf, InvariantSource::TaskPrompt)
            .with_value(YamlValue::Sequence(vec![]));

        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::String("anything".to_string())],
        );
        assert!(result.passed, "none_of with empty forbidden set should pass");
    }

    // ========================================================================
    // Null Handling Tests
    // ========================================================================

    #[test]
    fn test_predicate_exists_fails_for_null() {
        let predicate = Predicate::new("test", PredicateRule::Exists, InvariantSource::TaskPrompt);

        let result = evaluate_predicate(&predicate, &[YamlValue::Null]);
        assert!(!result.passed, "exists should fail for null value");
    }

    #[test]
    fn test_predicate_not_exists_passes_for_null() {
        let predicate = Predicate::new("test", PredicateRule::NotExists, InvariantSource::TaskPrompt);

        let result = evaluate_predicate(&predicate, &[YamlValue::Null]);
        assert!(result.passed, "not_exists should pass for null value");
    }

    #[test]
    fn test_predicate_exists_passes_for_empty_string() {
        let predicate = Predicate::new("test", PredicateRule::Exists, InvariantSource::TaskPrompt);

        let result = evaluate_predicate(&predicate, &[YamlValue::String(String::new())]);
        assert!(result.passed, "exists should pass for empty string (not null)");
    }

    #[test]
    fn test_predicate_exists_passes_for_empty_array() {
        let predicate = Predicate::new("test", PredicateRule::Exists, InvariantSource::TaskPrompt);

        let result = evaluate_predicate(&predicate, &[YamlValue::Sequence(vec![])]);
        assert!(result.passed, "exists should pass for empty array (not null)");
    }

    #[test]
    fn test_predicate_contains_on_null_fails() {
        let predicate = Predicate::new("test", PredicateRule::Contains, InvariantSource::TaskPrompt)
            .with_value(YamlValue::String("x".to_string()));

        let result = evaluate_predicate(&predicate, &[YamlValue::Null]);
        assert!(!result.passed, "contains on null should fail");
    }

    #[test]
    fn test_predicate_exists_with_mixed_null_and_value() {
        let predicate = Predicate::new("test", PredicateRule::Exists, InvariantSource::TaskPrompt);

        // If selected_values has both null and a real value, exists should pass
        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::Null, YamlValue::String("real".to_string())],
        );
        assert!(result.passed, "exists should pass when at least one non-null value");
    }

    #[test]
    fn test_predicate_not_exists_fails_with_mixed_null_and_value() {
        let predicate = Predicate::new("test", PredicateRule::NotExists, InvariantSource::TaskPrompt);

        let result = evaluate_predicate(
            &predicate,
            &[YamlValue::Null, YamlValue::String("real".to_string())],
        );
        assert!(!result.passed, "not_exists should fail when at least one non-null value");
    }

    // ========================================================================
    // When Condition Tests
    // ========================================================================

    #[test]
    fn test_when_condition_met_evaluates_predicate() {
        let mut rulespec = Rulespec::new();
        rulespec.add_claim(Claim::new("is_breaking", "api_changes.breaking"));
        rulespec.add_claim(Claim::new("caps", "feature.capabilities"));
        rulespec.add_predicate(
            Predicate::new("caps", PredicateRule::MinLength, InvariantSource::TaskPrompt)
                .with_value(YamlValue::Number(2.into()))
                .with_when(WhenCondition::new("is_breaking", PredicateRule::Equals)
                    .with_value(YamlValue::Bool(true)))
        );

        let mut envelope = ActionEnvelope::new();
        envelope.add_fact("api_changes", serde_yaml::from_str("breaking: true").unwrap());
        envelope.add_fact("feature", serde_yaml::from_str("capabilities: [a, b, c]").unwrap());

        let eval = evaluate_rulespec(&rulespec, &envelope);
        assert_eq!(eval.passed_count, 1);
        assert_eq!(eval.failed_count, 0);
    }

    #[test]
    fn test_when_condition_not_met_vacuous_pass() {
        let mut rulespec = Rulespec::new();
        rulespec.add_claim(Claim::new("is_breaking", "api_changes.breaking"));
        rulespec.add_claim(Claim::new("caps", "feature.capabilities"));
        rulespec.add_predicate(
            Predicate::new("caps", PredicateRule::MinLength, InvariantSource::TaskPrompt)
                .with_value(YamlValue::Number(100.into())) // would fail if evaluated
                .with_when(WhenCondition::new("is_breaking", PredicateRule::Equals)
                    .with_value(YamlValue::Bool(true)))
        );

        let mut envelope = ActionEnvelope::new();
        envelope.add_fact("api_changes", serde_yaml::from_str("breaking: false").unwrap());
        envelope.add_fact("feature", serde_yaml::from_str("capabilities: [a]").unwrap());

        let eval = evaluate_rulespec(&rulespec, &envelope);
        assert_eq!(eval.passed_count, 1, "When not met should be vacuous pass");
        assert_eq!(eval.failed_count, 0);
        assert!(eval.predicate_results[0].result.reason.contains("Skipped"));
    }

    #[test]
    fn test_when_condition_with_exists() {
        let mut rulespec = Rulespec::new();
        rulespec.add_claim(Claim::new("has_tests", "feature.tests"));
        rulespec.add_claim(Claim::new("coverage", "feature.coverage"));
        rulespec.add_predicate(
            Predicate::new("coverage", PredicateRule::GreaterThan, InvariantSource::TaskPrompt)
                .with_value(YamlValue::Number(80.into()))
                .with_when(WhenCondition::new("has_tests", PredicateRule::Exists))
        );

        // No tests field → when condition not met → vacuous pass
        let mut envelope = ActionEnvelope::new();
        envelope.add_fact("feature", serde_yaml::from_str("coverage: 50").unwrap());

        let eval = evaluate_rulespec(&rulespec, &envelope);
        assert_eq!(eval.passed_count, 1, "When exists not met should skip");
        assert_eq!(eval.failed_count, 0);
    }

    #[test]
    fn test_when_unknown_claim_fails_validation() {
        let mut rulespec = Rulespec::new();
        rulespec.add_claim(Claim::new("caps", "feature.capabilities"));
        rulespec.add_predicate(
            Predicate::new("caps", PredicateRule::Exists, InvariantSource::TaskPrompt)
                .with_when(WhenCondition::new("nonexistent", PredicateRule::Exists))
        );

        let result = rulespec.validate();
        assert!(result.is_err(), "When referencing unknown claim should fail validation");
        assert!(result.unwrap_err().to_string().contains("unknown claim"));
    }

    #[test]
    fn test_predicate_without_when_always_evaluated() {
        // Backward compatibility: no when field means always evaluated
        let mut rulespec = Rulespec::new();
        rulespec.add_claim(Claim::new("caps", "feature.capabilities"));
        rulespec.add_predicate(
            Predicate::new("caps", PredicateRule::Exists, InvariantSource::TaskPrompt)
        );

        let mut envelope = ActionEnvelope::new();
        envelope.add_fact("feature", serde_yaml::from_str("capabilities: [a]").unwrap());

        let eval = evaluate_rulespec(&rulespec, &envelope);
        assert_eq!(eval.passed_count, 1);
        assert_eq!(eval.failed_count, 0);
    }

    #[test]
    fn test_when_condition_validate_requires_value() {
        let when = WhenCondition::new("test", PredicateRule::Equals);
        // No value set — validation should catch this
        let predicate = Predicate::new("test", PredicateRule::Exists, InvariantSource::TaskPrompt)
            .with_when(when);
        let result = predicate.validate();
        assert!(result.is_err(), "When condition with equals but no value should fail");
    }

    #[test]
    fn test_when_condition_exists_no_value_ok() {
        let when = WhenCondition::new("test", PredicateRule::Exists);
        let predicate = Predicate::new("test", PredicateRule::Exists, InvariantSource::TaskPrompt)
            .with_when(when);
        let result = predicate.validate();
        assert!(result.is_ok(), "When condition with exists and no value should be ok");
    }

    #[test]
    fn test_evaluate_rulespec_with_new_rules_full() {
        let mut rulespec = Rulespec::new();
        rulespec.add_claim(Claim::new("caps", "feature.capabilities"));
        rulespec.add_claim(Claim::new("format", "feature.output_format"));
        rulespec.add_claim(Claim::new("breaking", "breaking_changes"));

        // not_contains: must not have deprecated
        rulespec.add_predicate(
            Predicate::new("caps", PredicateRule::NotContains, InvariantSource::TaskPrompt)
                .with_value(YamlValue::String("deprecated".to_string()))
        );
        // any_of: format must be json or yaml
        rulespec.add_predicate(
            Predicate::new("format", PredicateRule::AnyOf, InvariantSource::TaskPrompt)
                .with_value(YamlValue::Sequence(vec![
                    YamlValue::String("json".to_string()),
                    YamlValue::String("yaml".to_string()),
                ]))
        );
        // none_of: format must not be xml or csv
        rulespec.add_predicate(
            Predicate::new("format", PredicateRule::NoneOf, InvariantSource::TaskPrompt)
                .with_value(YamlValue::Sequence(vec![
                    YamlValue::String("xml".to_string()),
                    YamlValue::String("csv".to_string()),
                ]))
        );
        // not_exists: no breaking changes
        rulespec.add_predicate(
            Predicate::new("breaking", PredicateRule::NotExists, InvariantSource::TaskPrompt)
        );

        let mut envelope = ActionEnvelope::new();
        envelope.add_fact("feature", serde_yaml::from_str(r#"
            capabilities: [handle_csv, handle_tsv]
            output_format: json
        "#).unwrap());
        envelope.add_fact("breaking_changes", YamlValue::Null);

        let eval = evaluate_rulespec(&rulespec, &envelope);
        assert_eq!(eval.passed_count, 4, "All 4 predicates should pass");
        assert_eq!(eval.failed_count, 0);
    }
}
