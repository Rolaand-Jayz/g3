# Workspace Memory
> Updated: 2026-02-05T14:30:00Z | Size: ~19k chars

### Remember Tool Wiring
- `crates/g3-core/src/tools/memory.rs` [0..5000] - `execute_remember()`, `get_memory_path()`, `merge_memory()`
- `crates/g3-core/src/tool_definitions.rs` [11000..12000] - remember tool in `create_core_tools()`
- `crates/g3-core/src/tool_dispatch.rs` [48] - dispatch case
- `crates/g3-core/src/prompts.rs` [4200..6500] - Workspace Memory prompt section
- `crates/g3-cli/src/project_files.rs` - `read_workspace_memory()` loads `analysis/memory.md`

### Context Window & Compaction
- `crates/g3-core/src/context_window.rs` [0..29568]
  - `ThinResult` [23] - scope, before/after %, chars_saved
  - `ContextWindow` - token tracking, message history
  - `reset_with_summary()` - compact history to summary
  - `should_compact()` - threshold check (80%)
  - `thin_context()` - replace large results with file refs
- `crates/g3-core/src/compaction.rs` [0..11404]
  - `CompactionResult`, `CompactionConfig` - result/config structs
  - `perform_compaction()` - unified for force_compact() and auto-compaction
  - `calculate_capped_summary_tokens()`, `should_disable_thinking()`
  - `build_summary_messages()`, `apply_summary_fallback_sequence()`
- `crates/g3-core/src/lib.rs` - `Agent.force_compact()`, `stream_completion_with_tools()`

### Session Storage & Continuation
- `crates/g3-core/src/session_continuation.rs` [0..541] - `SessionContinuation`, `save_continuation()`, `load_continuation()`
- `crates/g3-core/src/paths.rs` [0..133] - `get_session_logs_dir()`, `get_thinned_dir()`, `get_session_file()`
- `crates/g3-core/src/session.rs` - Session logging utilities

### Tool System
- `crates/g3-core/src/tool_definitions.rs` [0..544] - `create_core_tools()`, `create_tool_definitions()`, `ToolConfig`
- `crates/g3-core/src/tool_dispatch.rs` [0..73] - `dispatch_tool()` routing

### CLI Module Structure
- `crates/g3-cli/src/lib.rs` [0..415] - `run()`, mode dispatch, config loading
- `crates/g3-cli/src/cli_args.rs` [0..133] - `Cli` struct (clap)
- `crates/g3-cli/src/autonomous.rs` [0..785] - `run_autonomous()`, coach-player loop
- `crates/g3-cli/src/agent_mode.rs` [0..284] - `run_agent_mode()`, `Agent::new_with_custom_prompt()`
- `crates/g3-cli/src/accumulative.rs` [0..343] - `run_accumulative_mode()`
- `crates/g3-cli/src/interactive.rs` [0..851] - `run_interactive()`, `run_interactive_machine()`, REPL
- `crates/g3-cli/src/task_execution.rs` [0..212] - `execute_task_with_retry()`, `OutputMode`
- `crates/g3-cli/src/commands.rs` [17..320] - `/help`, `/compact`, `/thinnify`, `/fragments`, `/rehydrate`
- `crates/g3-cli/src/utils.rs` [0..91] - `display_welcome_message()`, `get_workspace_path()`
- `crates/g3-cli/src/display.rs` - `format_workspace_path()`, `LoadedContent`, `print_loaded_status()`

### Auto-Memory System
- `crates/g3-core/src/lib.rs`
  - `send_auto_memory_reminder()` [47800..48800] - MEMORY CHECKPOINT prompt
  - `set_auto_memory()` [1451..1454] - enable/disable
  - `tool_calls_this_turn` [116] - tracks tools per turn
  - `execute_tool_in_dir()` [2843..2855] - records tool calls
- `crates/g3-core/src/prompts.rs` [3800..4500] - Memory Format in system prompt
- `crates/g3-cli/src/lib.rs` [393] - `--auto-memory` CLI flag

### Streaming Markdown Formatter
- `crates/g3-cli/src/streaming_markdown.rs`
  - `format_header()` [21500..22500] - headers with inline formatting
  - `process_in_code_block()` [439..462] - detects closing fence
  - `emit_code_block()` [654..675] - joins buffer, highlights code
  - `flush_incomplete()` [693..735] - handles unclosed blocks at stream end
- `crates/g3-cli/tests/streaming_markdown_test.rs` - header formatting tests
- **Gotcha**: closing ``` without trailing newline must be detected in `flush_incomplete()`

### Retry Infrastructure
- `crates/g3-core/src/retry.rs` [0..12000] - `execute_with_retry()`, `retry_operation()`, `RetryConfig`, `RetryResult`
- `crates/g3-cli/src/task_execution.rs` - `execute_task_with_retry()`

### UI Abstraction Layer
- `crates/g3-core/src/ui_writer.rs` [0..4500] - `UiWriter` trait, `NullUiWriter`, `print_thin_result()`
- `crates/g3-cli/src/ui_writer_impl.rs` [0..14000] - `ConsoleUiWriter`, `print_tool_compact()`
- `crates/g3-cli/src/simple_output.rs` [0..1200] - `SimpleOutput` helper

### Feedback Extraction
- `crates/g3-core/src/feedback_extraction.rs` [0..22000] - `extract_coach_feedback()`, `try_extract_from_session_log()`, `try_extract_from_native_tool_call()`
- `crates/g3-cli/src/coach_feedback.rs` [0..4025] - `extract_from_logs()` for coach-player loop

### Streaming Utilities & State
- `crates/g3-core/src/streaming.rs` [0..26146]
  - `MAX_ITERATIONS` [13] - constant (400)
  - `StreamingState` [16] - cross-iteration: full_response, first_token_time, iteration_count
  - `ToolOutputFormat` [54] - enum: SelfHandled, Compact(String), Regular
  - `IterationState` [166] - per-iteration: parser, current_response, tool_executed
  - `truncate_line()`, `truncate_for_display()`, `log_stream_error()`, `is_connection_error()`
  - `format_tool_result_summary()`, `is_compact_tool()`, `format_compact_tool_summary()`
- `crates/g3-core/src/lib.rs` [1879..2712] - `stream_completion_with_tools()` main loop

### Background Process Management
- `crates/g3-core/src/background_process.rs` [0..3000] - `BackgroundProcessManager`, `start()`, `list()`, `is_running()`, `get()`, `remove()`
- No `stop()` method - use shell `kill <pid>`

### Unified Diff Application
- `crates/g3-core/src/utils.rs` [5000..15000] - `apply_unified_diff_to_string()`, `parse_unified_diff_hunks()`
- Handles multi-hunk diffs, CRLF normalization, range constraints

### Error Classification
- `crates/g3-core/src/error_handling.rs` [0..567] - `classify_error()`, `ErrorType`, `RecoverableError`
- Priority: rate limit > network > server > busy > timeout > token limit > context length
- **Gotcha**: "Connection timeout" → NetworkError (not Timeout) due to "connection" keyword priority

### CLI Metrics
- `crates/g3-cli/src/metrics.rs` [0..5416] - `TurnMetrics`, `format_elapsed_time()`, `generate_turn_histogram()`

### ACD (Aggressive Context Dehydration)
Saves conversation fragments to disk, replaces with stubs.

- `crates/g3-core/src/acd.rs` [0..22830]
  - `Fragment` - `new()`, `save()`, `load()`, `generate_stub()`, `list_fragments()`, `get_latest_fragment_id()`
- `crates/g3-core/src/tools/acd.rs` [0..8500] - `execute_rehydrate()` tool
- `crates/g3-core/src/paths.rs` [3200..3400] - `get_fragments_dir()` → `.g3/sessions/<id>/fragments/`
- `crates/g3-core/src/compaction.rs` [195..240] - ACD integration, creates fragment+stub
- `crates/g3-core/src/context_window.rs` [10100..10700] - `reset_with_summary_and_stub()`
- `crates/g3-cli/src/lib.rs` [157..161] - `--acd` flag; [1476..1525] - `/fragments`, `/rehydrate`

**Fragment JSON**: `fragment_id`, `created_at`, `messages`, `message_count`, `user_message_count`, `assistant_message_count`, `tool_call_summary`, `estimated_tokens`, `topics`, `preceding_fragment_id`

### UTF-8 Safe String Slicing
Rust `&s[..n]` panics on multi-byte chars (emoji, CJK) if sliced mid-character.
**Pattern**: `s.char_indices().nth(n).map(|(i,_)| i).unwrap_or(s.len())`
**Danger zones**: Display truncation, ACD stubs, user input, non-ASCII text.

### Studio - Multi-Agent Workspace Manager
- `crates/studio/src/main.rs` [0..12500] - `cmd_run()`, `cmd_status()`, `cmd_accept()`, `cmd_discard()`, `extract_session_summary()`
- `crates/studio/src/session.rs` - `Session`, `SessionStatus`
- `crates/studio/src/git.rs` - `GitWorktree` for isolated agent sessions

**Session log**: `<worktree>/.g3/sessions/<session_id>/session.json`
**Fields**: `context_window.{conversation_history, percentage_used, total_tokens, used_tokens}`, `session_id`, `status`, `timestamp`

### Racket Code Search Support
- `crates/g3-core/src/code_search/searcher.rs`
  - Racket parser [~45] - `tree_sitter_racket::LANGUAGE`
  - Extensions [~90] - `.rkt`, `.rktl`, `.rktd` → "racket"

### Language-Specific Prompt Injection
Auto-detects languages and injects toolchain guidance.

- `crates/g3-cli/src/language_prompts.rs`
  - `LANGUAGE_PROMPTS` [12..19] - (lang_name, extensions, prompt_content)
  - `AGENT_LANGUAGE_PROMPTS` [21..26] - (agent_name, lang_name, prompt_content)
  - `detect_languages()` [22..32] - scans workspace
  - `scan_directory_for_extensions()` [42..77] - recursive, depth 2, skips hidden/vendor
  - `get_language_prompts_for_workspace()` [88..108]
  - `get_agent_language_prompts_for_workspace()` [124..137]
- `crates/g3-cli/src/agent_mode.rs` [149..159] - appends agent-specific prompts
- `prompts/langs/` - language prompt files

**To add language**: Create `prompts/langs/<lang>.md`, add to `LANGUAGE_PROMPTS`
**To add agent+lang**: Create `prompts/langs/<agent>.<lang>.md`, add to `AGENT_LANGUAGE_PROMPTS`

### MockProvider for Testing
- `crates/g3-providers/src/mock.rs`
  - `MockProvider` [220..320] - response queue, request tracking
  - `MockResponse` [35..200] - configurable chunks and usage
  - `scenarios` module [410..480] - `text_only_response()`, `multi_turn()`, `tool_then_response()`
- `crates/g3-core/tests/mock_provider_integration_test.rs` - integration tests

**Usage**: `MockProvider::new().with_response(MockResponse::text("Hello!"))`

### G3 Status Message Formatting
- `crates/g3-cli/src/g3_status.rs`
  - `Status` [12] - enum: Done, Failed, Error(String), Custom(String), Resolved, Insufficient, NoChanges
  - `G3Status` [44] - static methods for "g3:" prefixed messages
  - `progress()` [48] - "g3: <msg> ..." (no newline)
  - `done()` [72] - bold green "[done]"
  - `failed()` [81] - red "[failed]"
  - `thin_result()` [236] - formats ThinResult with colors

### Prompt Cache Statistics
- `crates/g3-providers/src/lib.rs` [195..210] - `Usage.cache_creation_tokens`, `cache_read_tokens`
- `crates/g3-providers/src/anthropic.rs` [944..956] - parses `cache_creation_input_tokens`, `cache_read_input_tokens`
- `crates/g3-providers/src/openai.rs` [494..510] - parses `prompt_tokens_details.cached_tokens`
- `crates/g3-core/src/lib.rs` [75..90] - `CacheStats` struct; [106] - `Agent.cache_stats`
- `crates/g3-core/src/stats.rs` [189..230] - `format_cache_stats()` with hit rate metrics

### Embedded Provider (Local LLM)
Local inference via llama-cpp-rs with Metal acceleration.

- `crates/g3-providers/src/embedded.rs`
  - `EmbeddedProvider` [22..85] - session, model_name, max_tokens, temperature, context_length
  - `new()` [26..85] - tilde expansion, auto-downloads Qwen if missing
  - `format_messages()` [87..175] - converts to prompt string (Qwen/Mistral/Llama templates)
  - `get_stop_sequences()` [280..340] - model-specific stop tokens
  - `stream()` [560..780] - via spawn_blocking + mpsc

### Chat Template Formats
| Model | Start Token | End Token |
|-------|-------------|----------|
| Qwen | `<\|im_start\|>role\n` | `<\|im_end\|>` |
| GLM-4 | `[gMASK]<sop><\|role\|>\n` | `<\|endoftext\|>` |
| Mistral | `<s>[INST]` | `[/INST]` |
| Llama | `<<SYS>>` | `<</SYS>>` |

### Recommended GGUF Models
| Model | Size | Use Case |
|-------|------|----------|
| GLM-4-9B-Q8_0 | ~10GB | Fast, capable |
| GLM-4-32B-Q6_K_L | ~27GB | Top tier coding/reasoning |
| Qwen3-4B-Q4_K_M | ~2.3GB | Small, rivals 72B |

**Download**: `huggingface-cli download <repo> --include "<file>" --local-dir ~/.g3/models/`

**Config**:
```toml
[providers.embedded.glm4]
model_path = "~/.g3/models/THUDM_GLM-4-32B-0414-Q6_K_L.gguf"
model_type = "glm4"
context_length = 32768
max_tokens = 4096
gpu_layers = 99
```

### Agent Skills System
Portable skill packages with SKILL.md + optional scripts per Agent Skills spec (agentskills.io).

- `crates/g3-core/src/skills/mod.rs` [0..47] - exports: `Skill`, `discover_skills`, `generate_skills_prompt`
- `crates/g3-core/src/skills/parser.rs` [0..363]
  - `Skill` [11..30] - name, description, metadata, body, path
  - `Skill::parse()` [45..100] - parses SKILL.md with YAML frontmatter
  - `validate_name()` [133..175] - 1-64 chars, lowercase+hyphens
- `crates/g3-core/src/skills/discovery.rs` [0..383]
  - `discover_skills()` [38..85] - scans 5 locations: embedded → global → extra → workspace → repo
  - `load_embedded_skills()` [88..102] - synthetic path `<embedded:name>/SKILL.md`
  - `is_embedded_skill()` [161..163] - checks `<embedded:` prefix
- `crates/g3-core/src/skills/embedded.rs` [0..87]
  - `EmbeddedSkill` [22..28] - name, skill_md, scripts[]
  - `EMBEDDED_SKILLS` [32..42] - static array with include_str! for research skill
- `crates/g3-core/src/skills/extraction.rs` [0..234]
  - `extract_script()` [28..85] - extracts to `.g3/bin/`, tracks version hash
  - `needs_update()` [107..118] - compares stored hash vs content
- `crates/g3-core/src/skills/prompt.rs` [0..140]
  - `generate_skills_prompt()` [12..40] - generates `<available_skills>` XML
- `crates/g3-config/src/lib.rs` [180..200] - `SkillsConfig` (enabled, extra_paths)
- `crates/g3-cli/src/project_files.rs` [180..210] - `discover_and_format_skills()`

**Skill Locations** (priority: later overrides earlier):
1. Embedded (compiled in)
2. `~/.g3/skills/` (global)
3. Config extra_paths
4. `.g3/skills/` (workspace)
5. `skills/` (repo root)

**SKILL.md Format**:
```yaml
---
name: skill-name          # Required: 1-64 chars, lowercase + hyphens
description: What it does # Required: 1-1024 chars
license: Apache-2.0       # Optional
compatibility: Requires X # Optional
---
# Instructions...
```

### Research Skill (Embedded)
Async web research via background scout agent. Externalized from core to embedded skill.

- `skills/research/SKILL.md` - skill definition
- `skills/research/g3-research` - bash script for async research
  - `write_status()` - writes status.json
  - `extract_report()` - extracts between markers or filters output

**Usage**:
```bash
background_process("research-topic", ".g3/bin/g3-research 'query'")
shell(".g3/bin/g3-research --status <id>")  # or --list
read_file(".g3/research/<id>/report.md")
```

**Output**: `.g3/research/<id>/status.json` + `report.md`

### Plan Mode
Structured task planning with cognitive forcing - requires happy/negative/boundary checks.

- `crates/g3-core/src/tools/plan.rs`
  - `Plan` [200..240] - plan_id, revision, approved_revision, items[]
  - `PlanItem` [110..145] - id, description, state, touches, checks, evidence, notes
  - `PlanState` [25..45] - enum: Todo, Doing, Done, Blocked
  - `Checks` [90..105] - happy, negative[], boundary[]
  - `get_plan_path()` [280..285] - `.g3/sessions/<id>/plan.g3.md`
  - `read_plan()`, `write_plan()` [290..335] - YAML in markdown
  - `plan_verify()` [659..700] - verifies evidence when complete
  - `execute_plan_read/write/approve()` [395..530] - tool implementations
- `crates/g3-core/src/tool_definitions.rs` [263..330] - plan_read, plan_write, plan_approve
- `crates/g3-core/src/prompts.rs` [21..130] - SHARED_PLAN_SECTION

**Tool names**: `plan_read`, `plan_write`, `plan_approve` (underscores, not dots)

### Plan Verification System
- `crates/g3-core/src/tools/plan.rs`
  - `EvidenceType` [283..300] - CodeLocation, TestReference, Unknown
  - `VerificationStatus` [303..320] - Verified, Warning, Error, Skipped
  - `parse_evidence()` [390..428] - parses `file:line-line` or `file::test_name`
  - `verify_code_location()` [443..495] - checks file exists, lines in range
  - `verify_test_reference()` [496..554] - checks test file, searches for fn

**Evidence formats**: `src/foo.rs:42-118`, `src/foo.rs:42`, `tests/foo.rs::test_bar`

### Invariants System (Rulespec & Envelope)
Machine-readable invariants for Plan Mode verification.

- `crates/g3-core/src/tools/invariants.rs`
  - `Claim` [50..75] - name + selector
  - `PredicateRule` [80..120] - Contains, Equals, Exists, NotExists, GreaterThan, LessThan, MinLength, MaxLength, Matches
  - `Predicate` [125..180] - claim, rule, value, source, notes
  - `Rulespec` [185..240] - claims[] + predicates[]
  - `ActionEnvelope` [245..290] - facts HashMap
  - `Selector` [295..410] - XPath-like: `foo.bar`, `foo[0]`, `foo[*]`
  - `evaluate_rulespec()` [780..850] - evaluates against envelope
  - Paths: `.g3/sessions/<id>/rulespec.yaml`, `envelope.yaml`

### Studio SDLC Pipeline
Orchestrates 7 agents in sequence for codebase maintenance.

- `crates/studio/src/sdlc.rs`
  - `PIPELINE_STAGES` [28..62] - euler → breaker → hopper → fowler → carmack → lamport → huffman
  - `Stage` [18..26] - name, description, focus
  - `StageStatus` [65..80] - Pending, Running, Complete, Failed, Skipped
  - `PipelineState` [108..140] - run_id, stages[], commit_cursor, session_id
  - `display_pipeline()` [354..390] - box display with status icons
- `crates/studio/src/main.rs`
  - `cmd_sdlc_run()` [540..655] - orchestrates pipeline, merges on completion
  - `has_commits_on_branch()` [715..728] - counts commits ahead of main
- `crates/studio/src/git.rs` - `merge_to_main()` (hardcodes 'main')

**State**: `.g3/sdlc/pipeline.json`
**CLI**: `studio sdlc run [-c N]`, `studio sdlc status`, `studio sdlc reset`
