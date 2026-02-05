# Observed Layering

**Scope**: Changes in commits `b6d2582..9443f933` (10 commits)

## Layer Structure

Observed from dependency direction (higher layers depend on lower):

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 4: Binaries / Entry Points                           │
│  ┌─────────────┐  ┌─────────────┐                          │
│  │  g3-cli     │  │   studio    │                          │
│  └─────────────┘  └─────────────┘                          │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 3: Orchestration                                     │
│  ┌─────────────┐                                           │
│  │ g3-planner  │                                           │
│  └─────────────┘                                           │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 2: Core Engine                                       │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                     g3-core                          │   │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐   │   │
│  │  │ skills  │ │ tools   │ │ prompts │ │ context │   │   │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘   │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 1: Infrastructure                                    │
│  ┌─────────────┐ ┌─────────────┐ ┌───────────────────────┐ │
│  │ g3-config   │ │g3-providers │ │ g3-computer-control   │ │
│  └─────────────┘ └─────────────┘ └───────────────────────┘ │
│  ┌─────────────┐                                           │
│  │g3-execution │                                           │
│  └─────────────┘                                           │
└─────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│  Layer 0: External Assets                                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  skills/research/  (SKILL.md, g3-research script)   │   │
│  └─────────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  prompts/system/   (native.md, etc.)                │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Layer Assignments (Changed Files)

| Layer | File | Evidence |
|-------|------|----------|
| 4 | g3-cli/src/lib.rs | Entry point, depends on g3-core |
| 4 | g3-cli/src/agent_mode.rs | Uses g3-core::Agent |
| 4 | g3-cli/src/interactive.rs | Uses g3-core::Agent |
| 4 | g3-cli/src/project_files.rs | Uses g3-core::skills, g3-config |
| 4 | studio/src/main.rs | Binary entry point |
| 4 | studio/src/sdlc.rs | Orchestrates g3 agents |
| 2 | g3-core/src/lib.rs | Core library root |
| 2 | g3-core/src/skills/mod.rs | Skills subsystem |
| 2 | g3-core/src/skills/parser.rs | SKILL.md parsing |
| 2 | g3-core/src/skills/discovery.rs | Skill directory scanning |
| 2 | g3-core/src/skills/prompt.rs | XML prompt generation |
| 2 | g3-core/src/skills/embedded.rs | Compile-time embedding |
| 2 | g3-core/src/skills/extraction.rs | Script extraction |
| 2 | g3-core/src/prompts.rs | System prompt generation |
| 2 | g3-core/src/tool_definitions.rs | Tool schema definitions |
| 2 | g3-core/src/tool_dispatch.rs | Tool routing |
| 1 | g3-config/src/lib.rs | Configuration structs |
| 0 | skills/research/SKILL.md | External skill definition |
| 0 | skills/research/g3-research | External skill script |
| 0 | prompts/system/native.md | System prompt template |

## Layer Violations

**None detected** in the changed files.

All dependencies flow downward (higher layer → lower layer).

## Skills Module Internal Layering

Within `g3-core/src/skills/`:

```
┌───────────────────────────────────────┐
│  mod.rs (coordinator, re-exports)     │  Layer 2.3
└───────────────────────────────────────┘
              │
              ▼
┌───────────────────────────────────────┐
│  discovery.rs, prompt.rs, extraction  │  Layer 2.2
│  (use parser.rs and/or embedded.rs)   │
└───────────────────────────────────────┘
              │
              ▼
┌───────────────────────────────────────┐
│  parser.rs, embedded.rs (leaf nodes)  │  Layer 2.1
│  (no internal dependencies)           │
└───────────────────────────────────────┘
```

## Derivation Method

Layers derived mechanically from:
1. Cargo.toml `[dependencies]` sections
2. `use` statement analysis
3. `mod` declaration hierarchy
4. `include_str!` compile-time references

No semantic interpretation applied.
