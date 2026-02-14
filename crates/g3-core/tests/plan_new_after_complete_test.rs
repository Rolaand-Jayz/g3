//! Plan New-After-Complete Integration Tests
//!
//! These tests verify that when an approved plan is fully complete (all items
//! done or blocked), a new plan can be written with different item IDs.
//!
//! Bug: execute_plan_write always preserves approved_revision from the existing
//! plan and blocks item removal, even when the existing plan is fully complete.
//! This prevents starting a new plan after finishing the previous one.

use g3_core::ui_writer::NullUiWriter;
use g3_core::{Agent, ToolCall};
use serial_test::serial;
use tempfile::TempDir;

// =============================================================================
// Test Helpers
// =============================================================================

async fn create_test_agent(temp_dir: &TempDir) -> Agent<NullUiWriter> {
    std::env::set_current_dir(temp_dir.path()).unwrap();
    let config = g3_config::Config::default();
    let ui_writer = NullUiWriter;
    Agent::new(config, ui_writer).await.unwrap()
}

fn make_tool_call(tool: &str, args: serde_json::Value) -> ToolCall {
    ToolCall {
        tool: tool.to_string(),
        args,
        id: String::new(),
    }
}

/// Helper: YAML for a simple plan with one item.
fn plan_yaml(plan_id: &str, item_id: &str, state: &str, with_evidence: bool) -> String {
    let evidence_section = if with_evidence {
        format!(
            r#"
    evidence: ["src/test.rs:1"]
    notes: "Done implementing""#
        )
    } else {
        String::new()
    };

    format!(
        r#"plan_id: {plan_id}
revision: 1
items:
  - id: {item_id}
    description: "Task for {item_id}"
    state: {state}
    touches: ["src/test.rs"]
    checks:
      happy:
        desc: Works
        target: test
      negative:
        - desc: Errors
          target: test
      boundary:
        - desc: Edge
          target: test{evidence_section}"#
    )
}

/// Helper: YAML for a plan with two items, each with independent state.
fn plan_yaml_two_items(
    plan_id: &str,
    id1: &str,
    state1: &str,
    evidence1: bool,
    id2: &str,
    state2: &str,
    evidence2: bool,
) -> String {
    let ev = |has: bool| -> String {
        if has {
            "\n    evidence: [\"src/test.rs:1\"]\n    notes: \"Done\"".to_string()
        } else {
            String::new()
        }
    };

    format!(
        r#"plan_id: {plan_id}
revision: 1
items:
  - id: {id1}
    description: "Task {id1}"
    state: {state1}
    touches: ["src/test.rs"]
    checks:
      happy: {{desc: Works, target: test}}
      negative: [{{desc: Errors, target: test}}]
      boundary: [{{desc: Edge, target: test}}]{ev1}
  - id: {id2}
    description: "Task {id2}"
    state: {state2}
    touches: ["src/test.rs"]
    checks:
      happy: {{desc: Works, target: test}}
      negative: [{{desc: Errors, target: test}}]
      boundary: [{{desc: Edge, target: test}}]{ev2}"#,
        ev1 = ev(evidence1),
        ev2 = ev(evidence2),
    )
}

// =============================================================================
// Happy path: new plan after completed plan should succeed
// =============================================================================

/// Reproduces the bug: create plan A, approve it, mark all items done,
/// then write plan B with completely different item IDs.
/// Before the fix, this fails with "Cannot remove item 'I1' from approved plan".
#[tokio::test]
#[serial]
async fn test_new_plan_after_completed_plan_succeeds() {
    let temp_dir = TempDir::new().unwrap();

    // Create evidence file so verification doesn't complain
    let src_dir = temp_dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("test.rs"), "// test").unwrap();

    let mut agent = create_test_agent(&temp_dir).await;
    agent.init_session_id_for_test("new-plan-after-complete");

    // Step 1: Write plan A
    let write_a = make_tool_call(
        "plan_write",
        serde_json::json!({ "plan": plan_yaml("plan-a", "I1", "todo", false) }),
    );
    let result = agent.execute_tool(&write_a).await.unwrap();
    assert!(result.contains("✅"), "Plan A write should succeed: {}", result);

    // Step 2: Approve plan A
    let approve = make_tool_call("plan_approve", serde_json::json!({}));
    let result = agent.execute_tool(&approve).await.unwrap();
    assert!(result.contains("approved"), "Plan A should be approved: {}", result);

    // Step 3: Mark plan A item as done
    let done_a = make_tool_call(
        "plan_write",
        serde_json::json!({ "plan": plan_yaml("plan-a", "I1", "done", true) }),
    );
    let result = agent.execute_tool(&done_a).await.unwrap();
    assert!(
        result.contains("✅"),
        "Marking plan A done should succeed: {}",
        result
    );

    // Step 4: Write plan B with completely different item IDs
    let write_b = make_tool_call(
        "plan_write",
        serde_json::json!({ "plan": plan_yaml("plan-b", "J1", "todo", false) }),
    );
    let result = agent.execute_tool(&write_b).await.unwrap();

    // THIS IS THE BUG: before the fix, this returns an error about removing item I1
    assert!(
        result.contains("✅"),
        "New plan B after completed plan A should succeed, but got: {}",
        result
    );
    assert!(
        !result.contains("Cannot remove item"),
        "Should NOT block item removal when old plan is complete: {}",
        result
    );

    // Verify plan B is stored correctly
    let read = make_tool_call("plan_read", serde_json::json!({}));
    let result = agent.execute_tool(&read).await.unwrap();
    assert!(
        result.contains("plan-b"),
        "Should now contain plan-b: {}",
        result
    );
    assert!(
        result.contains("J1"),
        "Should contain new item J1: {}",
        result
    );
}

// =============================================================================
// Negative: in-progress approved plan still blocks item removal
// =============================================================================

/// Confirms that removing items from an in-progress (not complete) approved plan
/// is still blocked — the fix must not weaken this protection.
#[tokio::test]
#[serial]
async fn test_inprogress_approved_plan_blocks_item_removal() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("test.rs"), "// test").unwrap();

    let mut agent = create_test_agent(&temp_dir).await;
    agent.init_session_id_for_test("inprogress-blocks-removal");

    // Write plan with two items
    let write = make_tool_call(
        "plan_write",
        serde_json::json!({
            "plan": plan_yaml_two_items("plan-x", "I1", "todo", false, "I2", "todo", false)
        }),
    );
    agent.execute_tool(&write).await.unwrap();

    // Approve
    let approve = make_tool_call("plan_approve", serde_json::json!({}));
    agent.execute_tool(&approve).await.unwrap();

    // Mark only I1 as done (I2 still todo — plan is NOT complete)
    let partial = make_tool_call(
        "plan_write",
        serde_json::json!({
            "plan": plan_yaml_two_items("plan-x", "I1", "done", true, "I2", "todo", false)
        }),
    );
    let result = agent.execute_tool(&partial).await.unwrap();
    assert!(result.contains("✅"), "Partial update should succeed: {}", result);

    // Now try to write a new plan that removes I2 — should be BLOCKED
    let remove_attempt = make_tool_call(
        "plan_write",
        serde_json::json!({ "plan": plan_yaml("plan-x", "I1", "done", true) }),
    );
    let result = agent.execute_tool(&remove_attempt).await.unwrap();
    assert!(
        result.contains("Cannot remove item"),
        "Should block removal of I2 from in-progress approved plan: {}",
        result
    );
}

// =============================================================================
// Boundary: plan where all items are blocked also allows new plan
// =============================================================================

/// A plan where every item is blocked (not done) is still "complete" per
/// is_complete() — verify a new plan can be started.
#[tokio::test]
#[serial]
async fn test_new_plan_after_all_blocked_plan_succeeds() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("test.rs"), "// test").unwrap();

    let mut agent = create_test_agent(&temp_dir).await;
    agent.init_session_id_for_test("new-plan-after-all-blocked");

    // Write plan with one item
    let write = make_tool_call(
        "plan_write",
        serde_json::json!({ "plan": plan_yaml("plan-blocked", "I1", "todo", false) }),
    );
    agent.execute_tool(&write).await.unwrap();

    // Approve
    let approve = make_tool_call("plan_approve", serde_json::json!({}));
    agent.execute_tool(&approve).await.unwrap();

    // Mark item as blocked (not done — no evidence needed)
    let block = make_tool_call(
        "plan_write",
        serde_json::json!({ "plan": plan_yaml("plan-blocked", "I1", "blocked", false) }),
    );
    let result = agent.execute_tool(&block).await.unwrap();
    assert!(result.contains("✅"), "Blocking should succeed: {}", result);

    // Now write a completely new plan — should succeed
    let write_new = make_tool_call(
        "plan_write",
        serde_json::json!({ "plan": plan_yaml("plan-fresh", "K1", "todo", false) }),
    );
    let result = agent.execute_tool(&write_new).await.unwrap();
    assert!(
        result.contains("✅"),
        "New plan after all-blocked plan should succeed: {}",
        result
    );
    assert!(
        !result.contains("Cannot remove item"),
        "Should NOT block item removal when old plan is fully blocked: {}",
        result
    );
}

// =============================================================================
// Boundary: completed plan with mix of done and blocked allows new plan
// =============================================================================

/// A plan with some items done and some blocked is complete — new plan allowed.
#[tokio::test]
#[serial]
async fn test_new_plan_after_mixed_done_blocked_succeeds() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("test.rs"), "// test").unwrap();

    let mut agent = create_test_agent(&temp_dir).await;
    agent.init_session_id_for_test("new-plan-mixed-complete");

    // Write plan with two items
    let write = make_tool_call(
        "plan_write",
        serde_json::json!({
            "plan": plan_yaml_two_items("plan-mix", "I1", "todo", false, "I2", "todo", false)
        }),
    );
    agent.execute_tool(&write).await.unwrap();

    // Approve
    let approve = make_tool_call("plan_approve", serde_json::json!({}));
    agent.execute_tool(&approve).await.unwrap();

    // Mark I1 done, I2 blocked — plan is complete
    let complete = make_tool_call(
        "plan_write",
        serde_json::json!({
            "plan": plan_yaml_two_items("plan-mix", "I1", "done", true, "I2", "blocked", false)
        }),
    );
    let result = agent.execute_tool(&complete).await.unwrap();
    assert!(result.contains("✅"), "Completing plan should succeed: {}", result);

    // Write a new plan with different IDs
    let write_new = make_tool_call(
        "plan_write",
        serde_json::json!({ "plan": plan_yaml("plan-next", "N1", "todo", false) }),
    );
    let result = agent.execute_tool(&write_new).await.unwrap();
    assert!(
        result.contains("✅"),
        "New plan after mixed done/blocked should succeed: {}",
        result
    );
}
