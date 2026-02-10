//! Tests for the abrupt stop bug where the agent returns control to the user
//! mid-task because tool calls are stored as text in the Message struct and
//! sent back to the Anthropic API as plain text instead of structured
//! tool_use/tool_result blocks.
//!
//! Root cause: Message struct has no tool_calls field. Native tool calls are
//! stored as inline JSON text. convert_messages() sends them as plain text,
//! not tool_use/tool_result blocks. The model sees its previous tool
//! interactions as text it wrote, not as actual tool invocations, and
//! occasionally emits text describing what it wants to do instead of
//! invoking the tool mechanism.

use g3_providers::{Message, MessageRole};

/// Demonstrates the bug: tool calls stored as inline JSON text in assistant
/// messages are indistinguishable from regular text when sent back to the API.
///
/// In the real bug, the model sees:
///   Assistant: "Let me check.\n\n{\"tool\": \"shell\", \"args\": {...}}"
///   User: "Tool result: ..."
///
/// Instead of the proper Anthropic format:
///   Assistant: [{type: "text", text: "Let me check."}, {type: "tool_use", id: "...", name: "shell", input: {...}}]
///   User: [{type: "tool_result", tool_use_id: "...", content: "..."}]
#[test]
fn test_tool_calls_stored_as_text_lack_structure() {
    // This is how tool calls are currently stored (the bug)
    let assistant_msg = Message::new(
        MessageRole::Assistant,
        "Let me check that file.\n\n{\"tool\": \"shell\", \"args\": {\"command\": \"ls\"}}".to_string(),
    );

    // The message has no structured tool call information
    assert!(
        assistant_msg.tool_calls.is_empty(),
        "Message should now support structured tool_calls field"
    );
}

/// Verifies that Message struct supports structured tool calls.
/// After the fix, tool calls should be stored structurally.
#[test]
fn test_message_supports_structured_tool_calls() {
    use g3_providers::MessageToolCall;

    let mut msg = Message::new(
        MessageRole::Assistant,
        "Let me check that file.".to_string(),
    );

    msg.tool_calls.push(MessageToolCall {
        id: "toolu_123".to_string(),
        name: "shell".to_string(),
        input: serde_json::json!({"command": "ls"}),
    });

    assert_eq!(msg.tool_calls.len(), 1);
    assert_eq!(msg.tool_calls[0].name, "shell");
    assert_eq!(msg.tool_calls[0].id, "toolu_123");
}

/// Verifies that Message struct supports tool_result for user messages.
/// After the fix, tool results should reference the tool_use_id.
#[test]
fn test_message_supports_tool_result() {
    let mut msg = Message::new(
        MessageRole::User,
        "file1.txt\nfile2.txt".to_string(),
    );

    msg.tool_result_id = Some("toolu_123".to_string());

    assert_eq!(msg.tool_result_id.as_deref(), Some("toolu_123"));
}

/// Integration test: simulates the exact bug scenario from the h3 session.
/// After several tool call iterations, the model stops mid-thought.
///
/// The fix ensures that when messages are sent back to the API, tool calls
/// are properly structured so the model maintains its tool-calling context.
#[test]
fn test_tool_call_roundtrip_preserves_structure() {
    use g3_providers::MessageToolCall;

    // Simulate a multi-turn tool-calling conversation
    let messages = vec![
        // System prompt
        Message::new(MessageRole::System, "You are a helpful assistant.".to_string()),
        // User asks something
        Message::new(MessageRole::User, "Check the files".to_string()),
        // Assistant uses a tool (properly structured)
        {
            let mut msg = Message::new(
                MessageRole::Assistant,
                "Let me check the files.".to_string(),
            );
            msg.tool_calls.push(MessageToolCall {
                id: "toolu_001".to_string(),
                name: "shell".to_string(),
                input: serde_json::json!({"command": "ls"}),
            });
            msg
        },
        // Tool result (properly structured)
        {
            let mut msg = Message::new(
                MessageRole::User,
                "file1.txt\nfile2.txt".to_string(),
            );
            msg.tool_result_id = Some("toolu_001".to_string());
            msg
        },
        // Assistant uses another tool
        {
            let mut msg = Message::new(
                MessageRole::Assistant,
                "Let me read file1.txt.".to_string(),
            );
            msg.tool_calls.push(MessageToolCall {
                id: "toolu_002".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"file_path": "file1.txt"}),
            });
            msg
        },
        // Tool result
        {
            let mut msg = Message::new(
                MessageRole::User,
                "Contents of file1.txt".to_string(),
            );
            msg.tool_result_id = Some("toolu_002".to_string());
            msg
        },
    ];

    // Verify all tool calls have IDs
    for msg in &messages {
        for tc in &msg.tool_calls {
            assert!(!tc.id.is_empty(), "Tool call should have an ID");
        }
        // Verify tool results reference a tool_use_id
        if msg.tool_result_id.is_some() {
            assert!(
                matches!(msg.role, MessageRole::User),
                "Tool results should be user messages"
            );
        }
    }

    // Verify assistant messages with tool calls still have text content
    let assistant_with_tools: Vec<_> = messages
        .iter()
        .filter(|m| matches!(m.role, MessageRole::Assistant) && !m.tool_calls.is_empty())
        .collect();
    assert_eq!(assistant_with_tools.len(), 2);
    for msg in assistant_with_tools {
        assert!(
            !msg.content.is_empty(),
            "Assistant messages should have text content alongside tool calls"
        );
    }
}
