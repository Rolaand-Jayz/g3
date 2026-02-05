//! Toolset loading tool implementation.
//!
//! This module provides the `load_toolset` tool which allows the agent to
//! dynamically load additional tool definitions at runtime.

use anyhow::Result;
use tracing::debug;

use crate::toolsets;
use crate::tools::executor::ToolContext;
use crate::ui_writer::UiWriter;
use crate::ToolCall;

/// Execute the load_toolset tool.
///
/// This tool loads a named toolset and returns the tool definitions so the
/// agent learns how to use the newly available tools.
///
/// The tool definitions are returned as formatted text describing each tool,
/// its purpose, and its parameters.
pub async fn execute_load_toolset<W: UiWriter>(
    tool_call: &ToolCall,
    ctx: &mut ToolContext<'_, W>,
) -> Result<String> {
    let toolset_name = tool_call
        .args
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    debug!("Loading toolset: {}", toolset_name);

    // Get the toolset from the registry
    let toolset = match toolsets::get_toolset(toolset_name) {
        Ok(ts) => ts,
        Err(err) => {
            return Ok(format!("❌ {}", err));
        }
    };

    // Check if already loaded (idempotent)
    if ctx.loaded_toolsets.contains(&toolset_name.to_string()) {
        return Ok(format!(
            "✅ Toolset '{}' is already loaded. You can use its tools.",
            toolset_name
        ));
    }

    // Get the tool definitions
    let tools = toolset.get_tools();

    // Mark as loaded
    ctx.loaded_toolsets.insert(toolset_name.to_string());

    // Format the tool definitions for the agent
    let mut output = String::new();
    output.push_str(&format!(
        "✅ Loaded toolset '{}' with {} tools:\n\n",
        toolset_name,
        tools.len()
    ));

    for tool in &tools {
        output.push_str(&format!("## {}", tool.name));
        output.push_str("\n\n");
        output.push_str(&tool.description);
        output.push_str("\n\n");
        
        // Format the input schema in a readable way
        if let Some(props) = tool.input_schema.get("properties") {
            if let Some(obj) = props.as_object() {
                if !obj.is_empty() {
                    output.push_str("**Parameters:**\n");
                    for (param_name, param_schema) in obj {
                        let param_type = param_schema
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("any");
                        let param_desc = param_schema
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("");
                        
                        // Check if required
                        let required = tool.input_schema
                            .get("required")
                            .and_then(|r| r.as_array())
                            .map(|arr| arr.iter().any(|v| v.as_str() == Some(param_name)))
                            .unwrap_or(false);
                        
                        let req_marker = if required { " (required)" } else { " (optional)" };
                        
                        output.push_str(&format!(
                            "- `{}` ({}){}: {}\n",
                            param_name, param_type, req_marker, param_desc
                        ));
                    }
                    output.push_str("\n");
                }
            }
        }
    }

    output.push_str(&format!(
        "---\nYou can now use these {} tools. They are available for the rest of this session.",
        tools.len()
    ));

    Ok(output)
}
