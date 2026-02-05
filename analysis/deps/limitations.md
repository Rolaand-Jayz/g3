# Analysis Limitations

**Scope**: Changes in commits `b6d2582..9443f933` (10 commits)

## What Could Not Be Observed

| Limitation | Impact | Mitigation |
|------------|--------|------------|
| Runtime dispatch | Tool dispatch uses string matching, not static imports | Analyzed `tool_dispatch.rs` manually |
| Conditional compilation | `#[cfg(...)]` blocks not analyzed | May miss platform-specific deps |
| Macro-generated code | `include_str!` detected, other macros not | Limited to explicit macros |
| External crate deps | crates.io dependencies not enumerated | Focus on workspace crates only |
| Test-only imports | Not distinguished from production | May overcount dependencies |
| Dynamic skill loading | Skills loaded at runtime from filesystem | Only compile-time embedded skills tracked |

## What Was Inferred

| Inference | Confidence | Rationale |
|-----------|------------|----------|
| Layer assignments | High | Based on Cargo.toml dependency direction |
| Fan-in/fan-out counts | High | Direct count of `use`/`mod` statements |
| Cross-crate edges | High | Explicit `use external_crate::` statements |
| Deleted file impact | Medium | Based on git diff, former imports not verified |

## Potential Invalidators

Conditions that would invalidate this analysis:

1. **Feature flags**: If `Cargo.toml` uses `[features]` to conditionally include dependencies, the graph may be incomplete for non-default configurations.

2. **Workspace-level dependencies**: The `[workspace.dependencies]` section in root `Cargo.toml` was not analyzed for version constraints.

3. **Build scripts**: `build.rs` files may generate code or modify dependencies at build time.

4. **Proc macros**: Procedural macros in dependencies may generate additional imports not visible in source.

5. **Path aliases**: If `Cargo.toml` uses `[patch]` or path aliases, actual dependency resolution may differ.

## Scope Boundaries

- **Included**: All files changed in commits `b6d2582..9443f933`
- **Excluded**: Unchanged files, even if they depend on changed files
- **Excluded**: Files outside `crates/` and `skills/` directories (except prompts/)

## Tool Versions

| Tool | Version | Purpose |
|------|---------|--------|
| git | system | Commit range, diff |
| rg (ripgrep) | system | Import pattern matching |
| Manual analysis | - | Cargo.toml parsing |

## Reproducibility

To reproduce this analysis:

```bash
# Get changed files
git diff --name-only 9443f933~10..9443f933

# Extract imports from Rust files
rg "^use |^mod |use g3_|use crate::" crates/*/src/*.rs

# Check Cargo.toml dependencies
cat crates/*/Cargo.toml | grep -A20 "\[dependencies\]"
```
