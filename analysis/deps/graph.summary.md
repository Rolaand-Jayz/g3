# Dependency Graph Summary

**Scope**: Changes in commits `b6d2582..9443f933` (10 commits)  
**Generated**: 2025-02-05

## Metrics

| Metric | Count |
|--------|-------|
| Crates (total) | 8 |
| Crates (changed) | 4 |
| Files (changed) | 29 |
| Files (added) | 8 |
| Files (deleted) | 2 |
| Files (modified) | 19 |
| Crate-level edges | 12 |
| File-level edges | 21 |

## Changed Crates

| Crate | Path | Role |
|-------|------|------|
| g3-core | crates/g3-core | Core engine, skills module added |
| g3-cli | crates/g3-cli | CLI interface, skills integration |
| g3-config | crates/g3-config | Configuration, SkillsConfig added |
| studio | crates/studio | Multi-agent workspace, SDLC changes |

## Entrypoints

| Entrypoint | Type | Evidence |
|------------|------|----------|
| g3-cli/src/lib.rs | Library root | `pub fn run()` |
| studio/src/main.rs | Binary | `fn main()` |
| g3-core/src/lib.rs | Library root | Re-exports skills module |

## Top Fan-In Nodes (most depended upon)

| Node | Fan-In | Dependents |
|------|--------|------------|
| g3-core/src/skills/parser.rs | 3 | discovery.rs, prompt.rs, mod.rs |
| g3-core/src/skills/embedded.rs | 3 | discovery.rs, extraction.rs, mod.rs |
| g3-core/src/skills/mod.rs | 3 | lib.rs, prompts.rs, project_files.rs |
| g3-config/src/lib.rs | 2 | g3-core (crate), g3-cli (crate) |
| g3-cli/src/project_files.rs | 2 | lib.rs, agent_mode.rs |

## Top Fan-Out Nodes (most dependencies)

| Node | Fan-Out | Dependencies |
|------|---------|-------------|
| g3-cli (crate) | 5 | g3-core, g3-config, g3-providers, g3-planner, g3-computer-control |
| g3-core/src/skills/mod.rs | 5 | parser.rs, discovery.rs, prompt.rs, embedded.rs, extraction.rs |
| g3-core/src/skills/discovery.rs | 2 | parser.rs, embedded.rs |
| g3-cli/src/project_files.rs | 2 | g3-core::skills, g3-config::SkillsConfig |
| studio/src/main.rs | 3 | sdlc.rs, git.rs, session.rs |

## Major Structural Changes

### Added: Skills Module (`g3-core/src/skills/`)

New module implementing Agent Skills specification:

```
g3-core/src/skills/
├── mod.rs        # Module root, re-exports
├── parser.rs     # SKILL.md YAML frontmatter parser
├── discovery.rs  # Skill directory scanning
├── prompt.rs     # XML prompt generation
├── embedded.rs   # Compile-time embedded skills
└── extraction.rs # Script extraction to .g3/bin/
```

**Internal dependency flow**:
```
mod.rs
  ├── parser.rs (Skill struct)
  ├── discovery.rs → parser.rs, embedded.rs
  ├── prompt.rs → parser.rs
  ├── embedded.rs (standalone)
  └── extraction.rs → embedded.rs
```

### Removed: Research Tool (hardcoded)

- `g3-core/src/pending_research.rs` (540 lines deleted)
- `g3-core/src/tools/research.rs` (710 lines deleted)

### Added: Research Skill (external)

- `skills/research/SKILL.md` (144 lines)
- `skills/research/g3-research` (338 lines, bash script)

Research functionality moved from hardcoded tool to external skill.

### Modified: SDLC Pipeline

- State storage moved from `analysis/sdlc/` to `.g3/sdlc/`
- Added merge-to-main on successful completion
- Worktree preserved on failure for debugging

## Extraction Limitations

- Dynamic imports not detected (none expected in Rust)
- Test-only dependencies not distinguished from production
- Conditional compilation (`#[cfg(...)]`) not analyzed
- External crate dependencies (from crates.io) not enumerated
