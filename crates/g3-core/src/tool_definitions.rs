//! Tool definitions for the agent's available tools.
//!
//! This module contains the JSON schema definitions for all tools that can be
//! used by the agent when interacting with LLM providers that support native
//! tool calling.

use g3_providers::Tool;
use serde_json::json;

/// Configuration for which optional tool sets to enable
#[derive(Debug, Clone, Copy, Default)]
pub struct ToolConfig {
    pub computer_control: bool,
    pub exclude_research: bool,
}

impl ToolConfig {
    pub fn new(computer_control: bool) -> Self {
        Self {
            computer_control,
            exclude_research: false,
        }
    }

    /// Create a config with the research tool excluded.
    /// Used for scout agent to prevent recursion.
    pub fn with_research_excluded(mut self) -> Self {
        self.exclude_research = true;
        self
    }
}

/// Create tool definitions for native tool calling providers.
///
/// Returns a vector of Tool definitions that describe the available tools
/// and their input schemas.
pub fn create_tool_definitions(config: ToolConfig) -> Vec<Tool> {
    // Webdriver tools are now JIT-loaded via load_toolset("webdriver")
    create_core_tools(config.exclude_research)
}

/// Create the core tools that are always available
fn create_core_tools(exclude_research: bool) -> Vec<Tool> {
    let mut tools = vec![
        Tool {
            name: "shell".to_string(),
            description: "Execute shell commands in the current working directory. Do NOT prefix commands with `cd <path> &&` - commands already run in the workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }),
        },
        Tool {
            name: "background_process".to_string(),
            description: "Launch a long-running process in the background (e.g., game servers, dev servers). The process runs independently and logs are captured to a file. Use the regular 'shell' tool to read logs (cat/tail), check status (ps), or stop the process (kill). Returns the PID and log file path.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "A unique name for this process (e.g., 'game_server', 'my_app'). Used to identify the process and its log file."
                    },
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute in the background"
                    },
                    "working_dir": {
                        "type": "string",
                        "description": "Optional working directory. Defaults to current directory if not specified."
                    }
                },
                "required": ["name", "command"]
            }),
        },
        Tool {
            name: "read_file".to_string(),
            description: "Read the contents of a file. Optionally read a specific character range.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file to read"
                    },
                    "start": {
                        "type": "integer",
                        "description": "Starting character position (0-indexed, inclusive). If omitted, reads from beginning."
                    },
                    "end": {
                        "type": "integer",
                        "description": "Ending character position (0-indexed, EXCLUSIVE). If omitted, reads to end of file."
                    }
                },
                "required": ["file_path"]
            }),
        },
        Tool {
            name: "read_image".to_string(),
            description: "Read one or more image files and send them to the LLM for visual analysis. Supports PNG, JPEG, GIF, and WebP formats. Use this when you need to visually inspect images (e.g., find sprites, analyze UI, read diagrams). The images will be included in your next response for analysis.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Array of paths to image files to read"
                    }
                },
                "required": ["file_paths"]
            }),
        },
        Tool {
            name: "write_file".to_string(),
            description: "Write content to a file (creates or overwrites). You MUST provide all arguments".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["file_path", "content"]
            }),
        },
        Tool {
            name: "str_replace".to_string(),
            description: "Apply a unified diff to a file. Supports multiple hunks and context lines. Optionally constrain the search to a [start, end) character range (0-indexed; end is EXCLUSIVE). Useful to disambiguate matches or limit scope in large files.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file to edit"
                    },
                    "diff": {
                        "type": "string",
                        "description": "A unified diff showing what to replace. Supports @@ hunk headers, context lines, and multiple hunks (---/+++ headers optional for minimal diffs)."
                    },
                    "start": {
                        "type": "integer",
                        "description": "Starting character position in the file (0-indexed, inclusive). If omitted, searches from beginning."
                    },
                    "end": {
                        "type": "integer",
                        "description": "Ending character position in the file (0-indexed, EXCLUSIVE - character at this position is NOT included). If omitted, searches to end of file."
                    }
                },
                "required": ["file_path", "diff"]
            }),
        },
        Tool {
            name: "code_search".to_string(),
            description: "Syntax-aware code search that understands code structure, not just text. Finds actual functions, classes, methods, and other code constructs - ignores matches in comments and strings. Much more accurate than grep for code searches. Supports batch searches (up to 20 parallel) with structured results and context lines. Languages: Rust, Python, JavaScript, TypeScript, Go, Java, C, C++, Racket. Uses tree-sitter query syntax.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "searches": {
                        "type": "array",
                        "maxItems": 20,
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string", "description": "Label for this search." },
                                "query": { "type": "string", "description": "tree-sitter query in S-expression format (e.g., \"(function_item name: (identifier) @name)\")" },
                                "language": { "type": "string", "enum": ["rust", "python", "javascript", "typescript", "go", "java", "c", "cpp", "racket"], "description": "Programming language to search." },
                                "paths": { "type": "array", "items": { "type": "string" }, "description": "Paths/dirs to search. Defaults to current dir if empty." },
                                "context_lines": { "type": "integer", "minimum": 0, "maximum": 20, "default": 0, "description": "Lines of context to include around each match." }
                            },
                            "required": ["name", "query", "language"]
                        }
                    },
                    "max_concurrency": { "type": "integer", "minimum": 1, "default": 4 },
                    "max_matches_per_search": { "type": "integer", "minimum": 1, "default": 500 }
                },
                "required": ["searches"]
            }),
        },
    ];

    // Plan Mode tools
    tools.push(Tool {
        name: "plan_read".to_string(),
        description: "Read the current Plan for this session. Shows the plan structure with items, their states, checks (happy/negative/boundary), evidence, and notes. Use this to review the plan before making updates.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    });

    tools.push(Tool {
        name: "plan_write".to_string(),
        description: "Create or update the Plan for this session. For NEW plans, you MUST provide both 'plan' and 'rulespec' arguments. The rulespec defines invariants (constraints that must/must not hold) extracted from the task and memory. For plan UPDATES, rulespec is optional.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "plan": {
                    "type": "string",
                    "description": "The plan as YAML. Must include plan_id and items array."
                },
                "rulespec": {
                    "type": "string",
                    "description": "The rulespec as YAML with claims and predicates. REQUIRED for new plans, optional for updates. Defines invariants from task_prompt and memory."
                }
            },
            "required": ["plan"]
        }),
    });

    tools.push(Tool {
        name: "plan_approve".to_string(),
        description: "Mark the current plan revision as approved. This is called by the user (not the agent) to approve a drafted plan before implementation begins. Once approved, plan items cannot be removed (only marked as blocked). The agent should ask for approval after drafting a plan.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    });

    // Workspace memory tool (memory is auto-loaded at startup, only remember is needed)
    tools.push(Tool {
        name: "remember".to_string(),
        description: "Update the workspace memory with new discoveries. Call this at the END of your turn (before your summary) if you discovered something worth noting. Provide your notes in markdown format - they will be merged with existing memory.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "notes": {
                    "type": "string",
                    "description": "New discoveries to add to memory in markdown format. Use the format:\n### Feature Name\n- `file/path.rs` [start..end] - `function_name()`, `StructName`\n\nDo not include content already in memory."
                }
            },
            "required": ["notes"]
        }),
    });

    // ACD rehydration tool
    tools.push(Tool {
        name: "rehydrate".to_string(),
        description: "Restore dehydrated conversation history from a previous context segment. Use this when you see a DEHYDRATED CONTEXT stub and need to recall the full conversation details from that segment.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "fragment_id": {
                    "type": "string",
                    "description": "The fragment ID to restore (from a DEHYDRATED CONTEXT stub message)"
                }
            },
            "required": ["fragment_id"]
        }),
    });

    // Conditionally add the research tool (excluded for scout agent to prevent recursion)
    if !exclude_research {
        tools.push(Tool {
            name: "research".to_string(),
            description: "Initiate web-based research on a topic. This tool is ASYNCHRONOUS - it spawns a research agent in the background and returns immediately with a research_id. Results are automatically injected into the conversation when ready. Use this when you need to research APIs, SDKs, libraries, approaches, bugs, or documentation. If you need the results before continuing, say so and yield the turn to the user. Check status with research_status tool.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The research question or topic to investigate. Be specific about what you need to know."
                    }
                },
                "required": ["query"]
            }),
        });

        // research_status tool - check status of pending research
        tools.push(Tool {
            name: "research_status".to_string(),
            description: "Check the status of pending research tasks. Call without arguments to list all pending research, or with a research_id to check a specific task. Use this to see if research has completed before it's automatically injected.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "research_id": {
                        "type": "string",
                        "description": "Optional: specific research_id to check. If omitted, lists all pending research tasks."
                    }
                },
                "required": []
            }),
        });
    }

    // Toolset loading tool - allows dynamic loading of additional tools
    tools.push(Tool {
        name: "load_toolset".to_string(),
        description: "Load additional tools from a named toolset. Returns the tool definitions so you learn how to use them. Use this when you need specialized tools like browser automation.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "The name of the toolset to load (e.g., 'webdriver')"
                }
            },
            "required": ["name"]
        }),
    });

    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_tools_count() {
        let tools = create_core_tools(false);
        // Core tools (with research): shell, background_process, read_file, read_image,
        // write_file, str_replace, code_search, plan_read, plan_write, plan_approve,
        // remember, rehydrate, research, research_status, load_toolset
        // (15 total)
        assert_eq!(tools.len(), 15);
    }

    #[test]
    fn test_create_tool_definitions_core_only() {
        let config = ToolConfig::default();
        let tools = create_tool_definitions(config);
        // 15 core tools (webdriver is now JIT-loaded)
        assert_eq!(tools.len(), 15);
    }

    #[test]
    fn test_create_tool_definitions() {
        let config = ToolConfig::new(true);
        let tools = create_tool_definitions(config);
        // Webdriver tools are now JIT-loaded, so only core tools are included
        assert!(tools.len() >= 14); // At least 14 core tools
    }

    #[test]
    fn test_tool_has_required_fields() {
        let tools = create_core_tools(false);
        for tool in tools {
            assert!(!tool.name.is_empty(), "Tool name should not be empty");
            assert!(!tool.description.is_empty(), "Tool description should not be empty");
            assert!(tool.input_schema.is_object(), "Tool input_schema should be an object");
        }
    }

    #[test]
    fn test_exclude_research_reduces_tool_count() {
        let with_research = create_core_tools(false);
        let without_research = create_core_tools(true);
        // Excluding research removes 2 tools (research and research_status)
        assert_eq!(with_research.len() - without_research.len(), 2);
    }
}
