# Coupling Hotspots

**Scope**: Changes in commits `b6d2582..9443f933` (10 commits)

## High Fan-In Files (Most Depended Upon)

Files that many other files depend on. Changes here have wide impact.

| File | Fan-In | Dependents | Risk |
|------|--------|------------|------|
| `g3-core/src/skills/parser.rs` | 3 | discovery.rs, prompt.rs, mod.rs | Medium |
| `g3-core/src/skills/embedded.rs` | 3 | discovery.rs, extraction.rs, mod.rs | Medium |
| `g3-core/src/skills/mod.rs` | 3 | lib.rs, prompts.rs, project_files.rs (cross-crate) | High |
| `g3-config/src/lib.rs` | 2 | g3-core, g3-cli (cross-crate) | High |
| `g3-cli/src/project_files.rs` | 2 | lib.rs, agent_mode.rs | Medium |

### Analysis

**`g3-core/src/skills/mod.rs`** (Fan-In: 3, Cross-Crate: Yes)
- Re-exports `Skill`, `discover_skills`, `generate_skills_prompt`, `EmbeddedSkill`
- Used by `g3-core/src/lib.rs` (re-export), `g3-core/src/prompts.rs`, `g3-cli/src/project_files.rs`
- **Evidence**: `pub use parser::Skill`, `pub use discovery::discover_skills`
- **Impact**: API changes affect both g3-core internals and g3-cli

**`g3-core/src/skills/parser.rs`** (Fan-In: 3, Cross-Crate: No)
- Defines `Skill` struct used throughout skills module
- **Evidence**: `use super::parser::Skill` in discovery.rs, prompt.rs
- **Impact**: Struct field changes ripple through entire skills subsystem

**`g3-config/src/lib.rs`** (Fan-In: 2, Cross-Crate: Yes)
- Added `SkillsConfig` struct
- **Evidence**: `use g3_config::SkillsConfig` in project_files.rs
- **Impact**: Config schema changes affect CLI startup

## High Fan-Out Files (Most Dependencies)

Files that depend on many others. Complex, potentially fragile.

| File | Fan-Out | Dependencies | Risk |
|------|---------|--------------|------|
| `g3-core/src/skills/mod.rs` | 5 | parser, discovery, prompt, embedded, extraction | Medium |
| `g3-core/src/skills/discovery.rs` | 2 | parser.rs, embedded.rs | Low |
| `g3-cli/src/project_files.rs` | 2 | g3-core::skills, g3-config | Medium |
| `studio/src/main.rs` | 3 | sdlc.rs, git.rs, session.rs | Low |

### Analysis

**`g3-core/src/skills/mod.rs`** (Fan-Out: 5)
- Module root that coordinates all skills submodules
- **Evidence**: `mod parser; mod discovery; mod prompt; mod embedded; pub mod extraction`
- **Impact**: Central coordination point, but each submodule is relatively independent

**`g3-cli/src/project_files.rs`** (Fan-Out: 2, Cross-Crate: Yes)
- Bridges g3-core skills and g3-config
- **Evidence**: `use g3_core::{discover_skills, ...}`, `use g3_config::SkillsConfig`
- **Impact**: Integration point for skills feature in CLI

## Cross-Crate Coupling

Edges that cross crate boundaries. Higher coordination cost for changes.

| From | To | Type | Evidence |
|------|----|------|----------|
| g3-cli/src/project_files.rs | g3-core::skills | use_external | `use g3_core::{discover_skills, generate_skills_prompt, Skill}` |
| g3-cli/src/project_files.rs | g3-config | use_external | `use g3_config::SkillsConfig` |
| g3-core/src/lib.rs | g3-core::skills | pub_use | `pub use skills::{Skill, discover_skills, generate_skills_prompt}` |

## Compile-Time Coupling (include_str!)

Files embedded at compile time. Build breaks if missing.

| Source | Embedded File | Evidence |
|--------|---------------|----------|
| g3-core/src/skills/embedded.rs | skills/research/SKILL.md | `include_str!("../../../../skills/research/SKILL.md")` |
| g3-core/src/skills/embedded.rs | skills/research/g3-research | `include_str!("../../../../skills/research/g3-research")` |

**Impact**: 
- Moving or renaming `skills/research/` breaks g3-core compilation
- Content changes require g3-core recompilation
- Relative path `../../../../` is fragile to directory restructuring

## Deleted Code Impact

Removed files and their former dependents.

| Deleted File | Lines | Former Dependents |
|--------------|-------|-------------------|
| g3-core/src/pending_research.rs | 540 | g3-core/src/lib.rs, tools/research.rs |
| g3-core/src/tools/research.rs | 710 | tool_dispatch.rs, tools/mod.rs |

**Impact**: 
- Research functionality moved to external skill
- `tool_dispatch.rs` and `tools/mod.rs` modified to remove research tool dispatch
- CLI commands related to research removed from `commands.rs`

## Recommendations for Monitoring

1. **`g3-core/src/skills/mod.rs`**: Watch for API surface changes
2. **`g3-config/src/lib.rs`**: Watch for `SkillsConfig` schema changes
3. **`skills/research/`**: Watch for path changes (compile-time dependency)
4. **`g3-cli/src/project_files.rs`**: Integration point, test after skills changes
