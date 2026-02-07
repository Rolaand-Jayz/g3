# Rulespec YAML Schema

> Canonical reference for `analysis/rulespec.yaml` — the machine-readable invariant specification.

## Overview

A rulespec defines **claims** (selectors into the action envelope) and **predicates** (rules that evaluate those claims). When an agent completes work, it writes an **action envelope** via `write_envelope`, and the rulespec is evaluated against it using datalog verification.

## File Location

```
analysis/rulespec.yaml    # checked into the repo
```

## Top-Level Structure

```yaml
claims:
  - name: <string>        # unique identifier for this claim
    selector: <string>    # path into the action envelope

predicates:
  - claim: <string>       # references a claim by name
    rule: <rule_type>     # one of the 12 rule types below
    value: <any>          # required for most rules (see table)
    source: <source>      # task_prompt | memory
    notes: <string>       # optional human-readable explanation
    when:                 # optional conditional trigger
      claim: <string>     # references a claim by name
      rule: <rule_type>   # condition rule
      value: <any>        # condition value (if needed)
```

## Claims

A claim is a named selector that extracts values from the action envelope.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | ✅ | Unique identifier (referenced by predicates) |
| `selector` | string | ✅ | Dot-notation path into the envelope |

### Selector Syntax

| Syntax | Meaning | Example |
|--------|---------|--------|
| `foo.bar` | Nested field access | `csv_importer.capabilities` |
| `foo[0]` | Array index (0-based) | `tests[0].name` |
| `foo[*]` | All array elements (wildcard) | `items[*].id` |
| `foo.bar.baz` | Deep nesting | `api.endpoints.count` |

**Important**: Selectors operate on the envelope's `facts` content directly. Do NOT include a `facts.` prefix in selectors — the system handles this automatically.

```yaml
# ✅ Correct
claims:
  - name: caps
    selector: csv_importer.capabilities

# ❌ Wrong — don't prefix with "facts."
claims:
  - name: caps
    selector: facts.csv_importer.capabilities
```

## Predicate Rules

### Rule Types Reference

| Rule | Value Required | Value Type | Description |
|------|---------------|------------|-------------|
| `exists` | ❌ | — | Value exists and is not null |
| `not_exists` | ❌ | — | Value is null or missing |
| `equals` | ✅ | any | Value equals exactly |
| `contains` | ✅ | any | Array contains element, or string contains substring |
| `not_contains` | ✅ | any | Negation of `contains` |
| `any_of` | ✅ | array | Value is one of the specified set |
| `none_of` | ✅ | array | Value is none of the specified set |
| `greater_than` | ✅ | number | Numeric comparison |
| `less_than` | ✅ | number | Numeric comparison |
| `min_length` | ✅ | number | Array has at least N elements |
| `max_length` | ✅ | number | Array has at most N elements |
| `matches` | ✅ | string | Value matches a regex pattern |

### Existence Rules

```yaml
# Value must exist (not null, not missing)
predicates:
  - claim: feature_file
    rule: exists
    source: task_prompt

# Value must NOT exist (null or missing)
predicates:
  - claim: breaking_changes
    rule: not_exists
    source: task_prompt
    notes: "No breaking changes allowed"
```

### Equality

```yaml
predicates:
  - claim: api_breaking
    rule: equals
    value: false
    source: task_prompt
```

### Containment

```yaml
# Array contains an element
predicates:
  - claim: capabilities
    rule: contains
    value: "handle_csv"
    source: task_prompt

# Array must NOT contain an element
predicates:
  - claim: capabilities
    rule: not_contains
    value: "deprecated_feature"
    source: task_prompt
```

### Set Membership

```yaml
# Value must be one of these
predicates:
  - claim: output_format
    rule: any_of
    value: [json, yaml, toml]
    source: task_prompt

# Value must NOT be any of these
predicates:
  - claim: output_format
    rule: none_of
    value: [xml, csv]
    source: task_prompt
```

### Numeric Comparisons

```yaml
predicates:
  - claim: test_count
    rule: greater_than
    value: 0
    source: task_prompt
    notes: "Must have at least one test"

  - claim: error_rate
    rule: less_than
    value: 5
    source: memory
```

### Array Length

```yaml
predicates:
  - claim: capabilities
    rule: min_length
    value: 2
    source: task_prompt

  - claim: dependencies
    rule: max_length
    value: 10
    source: memory
    notes: "Keep dependency count manageable"
```

### Regex Matching

```yaml
predicates:
  - claim: file_path
    rule: matches
    value: "^src/.*\\.rs$"
    source: task_prompt
    notes: "File must be a Rust source file in src/"
```

## Conditional Predicates (`when`)

Predicates can have an optional `when` condition. If the condition is **not met**, the predicate is **skipped** (vacuous pass) — it does not fail.

This is useful for rules that only apply in certain contexts.

### When Condition Structure

```yaml
when:
  claim: <string>       # references a defined claim
  rule: <rule_type>      # any predicate rule type
  value: <any>           # optional, depends on rule
```

### Examples

```yaml
# Only enforce endpoint count when there are breaking changes
predicates:
  - claim: api_endpoints
    rule: min_length
    value: 3
    source: task_prompt
    when:
      claim: is_breaking
      rule: equals
      value: true
    notes: "Breaking changes must document all endpoints"

# Only check test coverage when tests exist
predicates:
  - claim: coverage_percent
    rule: greater_than
    value: 80
    source: memory
    when:
      claim: has_tests
      rule: exists

# Only enforce format when feature is present
predicates:
  - claim: output_format
    rule: any_of
    value: [json, yaml]
    source: task_prompt
    when:
      claim: has_output
      rule: exists
```

### When with Regex Matching

```yaml
# Only require reply_to_message_id when subject starts with "Re: "
predicates:
  - claim: reply_to_id
    rule: exists
    source: task_prompt
    when:
      claim: subject_line
      rule: matches
      value: "^Re: "
    notes: Reply emails must have reply_to_message_id set
```

## Null Handling

Null values in the action envelope have specific semantics:

- **`null` is treated as absent** — `exists` returns false, `not_exists` returns true
- This applies to both the invariants evaluator and the datalog compiler
- A fact with value `null` produces **no datalog facts** (it is skipped entirely)

### Common Pattern: Asserting Absence

```yaml
# In the envelope:
facts:
  breaking_changes: null    # explicitly absent

# In the rulespec:
claims:
  - name: breaking
    selector: breaking_changes
predicates:
  - claim: breaking
    rule: not_exists
    source: task_prompt
    notes: "No breaking changes"    # ✅ This passes
```

### Edge Cases

| Envelope Value | `exists` | `not_exists` | `contains "x"` | `equals "y"` |
|---------------|----------|-------------|----------------|-------------|
| `null` | ❌ fail | ✅ pass | ❌ fail | ❌ fail |
| missing key | ❌ fail | ✅ pass | ❌ fail | ❌ fail |
| `""` (empty string) | ✅ pass | ❌ fail | ❌ fail | ❌ fail |
| `[]` (empty array) | ✅ pass | ❌ fail | ❌ fail | ❌ fail |
| `0` | ✅ pass | ❌ fail | ❌ fail | depends |

## Action Envelope Format

The action envelope is written via the `write_envelope` tool. It must have a top-level `facts:` key.

```yaml
facts:
  feature_name:
    capabilities: [cap_a, cap_b]
    file: "src/feature.rs"
    tests: ["test_a", "test_b"]
  api_changes:
    breaking: false
  breaking_changes: null    # Use null to assert absence
```

### Rules for Envelope Facts

1. **Must have `facts:` top-level key** — without it, the envelope is empty
2. **Use file paths as evidence** — `"src/foo.rs"`, `"src/foo.rs:42"`
3. **Use `null` for explicit absence** — triggers `not_exists` predicates
4. **Arrays for lists** — capabilities, tests, endpoints
5. **Nested objects for grouping** — `feature.capabilities`, `feature.file`

## Complete Example

```yaml
# analysis/rulespec.yaml
claims:
  - name: caps
    selector: csv_importer.capabilities
  - name: file
    selector: csv_importer.file
  - name: tests
    selector: csv_importer.tests
  - name: breaking
    selector: api_changes.breaking
  - name: no_breaking
    selector: breaking_changes

predicates:
  # Must have capabilities
  - claim: caps
    rule: exists
    source: task_prompt

  # Must include handle_csv
  - claim: caps
    rule: contains
    value: "handle_csv"
    source: task_prompt

  # Must NOT include deprecated features
  - claim: caps
    rule: not_contains
    value: "legacy_parser"
    source: memory

  # At least 2 capabilities
  - claim: caps
    rule: min_length
    value: 2
    source: task_prompt

  # File must be a Rust source
  - claim: file
    rule: matches
    value: "^src/.*\\.rs$"
    source: task_prompt

  # Must have tests
  - claim: tests
    rule: min_length
    value: 1
    source: task_prompt

  # No breaking changes
  - claim: no_breaking
    rule: not_exists
    source: task_prompt

  # If breaking, must document it
  - claim: caps
    rule: contains
    value: "migration_guide"
    source: task_prompt
    when:
      claim: breaking
      rule: equals
      value: true
    notes: "Breaking changes require a migration guide capability"
```

## Verification Pipeline

1. Agent calls `write_envelope` with facts YAML
2. System writes `envelope.yaml` to session directory
3. System reads `analysis/rulespec.yaml` from working directory
4. Rulespec is compiled into datalog relations
5. Facts are extracted from envelope using claim selectors
6. Datalog rules are executed to fixed point
7. Results are written to `rulespec.compiled.dl` and `datalog_evaluation.txt`
8. Summary is returned to the agent
9. On plan completion, `plan_verify()` checks that the envelope exists

## Source Types

| Source | Meaning |
|--------|---------|
| `task_prompt` | Invariant derived from the user's task description |
| `memory` | Invariant derived from workspace memory (AGENTS.md, memory.md) |
