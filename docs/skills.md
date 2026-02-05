# Agent Skills Guide

**Last updated**: February 2025  
**Source of truth**: `crates/g3-core/src/skills/`, `skills/`

## Purpose

This document describes g3's Agent Skills system - a mechanism for extending agent capabilities through portable skill packages. Skills allow g3 to learn new abilities without code changes.

## Overview

g3 implements the [Agent Skills](https://agentskills.io) specification, an open format for portable skill packages. Each skill is a directory containing:

- `SKILL.md` - Skill definition with YAML frontmatter and instructions
- `scripts/` (optional) - Executable scripts the skill can use
- `references/` (optional) - Additional documentation
- `assets/` (optional) - Templates, data files, etc.

At startup, g3 discovers skills from multiple locations and injects a summary into the system prompt. When the agent needs a skill, it reads the full `SKILL.md` using the `read_file` tool.

## Quick Start

### Using Existing Skills

Skills are automatically discovered. To see available skills, check the system prompt or look in:

```bash
# Global skills (shared across all projects)
ls ~/.g3/skills/

# Workspace skills (project-specific)
ls .g3/skills/

# Repo skills (checked into git)
ls skills/
```

### Creating a New Skill

1. Create a skill directory:
   ```bash
   mkdir -p skills/my-skill
   ```

2. Create `SKILL.md` with frontmatter:
   ```markdown
   ---
   name: my-skill
   description: Brief description of what this skill does and when to use it.
   license: MIT
   compatibility: Any requirements (e.g., "Requires Python 3.8+")
   ---

   # My Skill

   Detailed instructions for the agent...
   ```

3. The skill is now available to g3.

## SKILL.md Format

### Frontmatter (Required)

The YAML frontmatter between `---` markers defines skill metadata:

```yaml
---
name: skill-name          # Required: 1-64 chars, lowercase + hyphens only
description: What it does # Required: 1-1024 chars, when to use this skill
license: Apache-2.0       # Optional: SPDX license identifier
compatibility: Requires X # Optional: Environment requirements (max 500 chars)
metadata:                 # Optional: Arbitrary key-value pairs
  author: your-org
  version: "1.0"
---
```

**Name validation rules**:
- 1-64 characters
- Lowercase letters, numbers, and hyphens only
- Must start with a letter
- No consecutive hyphens

### Body (Instructions)

After the frontmatter, write detailed instructions for the agent:

```markdown
# Skill Title

## Quick Start
How to use this skill in the simplest case.

## Detailed Usage
Step-by-step instructions, examples, edge cases.

## Troubleshooting
Common issues and solutions.
```

**Best practices**:
- Keep the description concise (it's shown in the skill summary)
- Put detailed instructions in the body (only loaded when needed)
- Include concrete examples
- Document error handling

## Discovery Priority

Skills are discovered from multiple locations. Higher priority sources override lower ones:

| Priority | Location | Use Case |
|----------|----------|----------|
| 1 (lowest) | Embedded | Core skills compiled into binary |
| 2 | `~/.g3/skills/` | Global user skills |
| 3 | Config `extra_paths` | Organization-wide skills |
| 4 | `.g3/skills/` | Workspace-local customizations |
| 5 (highest) | `skills/` | Repo skills (checked into git) |

**Override behavior**: If the same skill name exists in multiple locations, the highest priority version wins. This allows:
- Customizing embedded skills per-project
- Testing skill changes without modifying global installs
- Sharing skills across an organization via config paths

## Configuration

Skills can be configured in `~/.config/g3/config.toml` or `./g3.toml`:

```toml
[skills]
enabled = true                    # Default: true
extra_paths = [                   # Additional skill directories
  "/org/shared/skills",
  "~/my-skills"
]
```

To disable skills entirely:

```toml
[skills]
enabled = false
```

## Embedded Skills

Core skills are embedded into the g3 binary at compile time, ensuring they work anywhere without external files.

### Currently Embedded Skills

| Skill | Description |
|-------|-------------|
| `research` | Web-based research via scout agent with browser automation |

### How Embedding Works

Embedded skills use Rust's `include_str!` macro to compile SKILL.md and scripts into the binary:

```rust
// Currently empty - skills can be added here as needed
static EMBEDDED_SKILLS: &[EmbeddedSkill] = &[
    // Example of how to add an embedded skill:
    // EmbeddedSkill {
    //     name: "example-skill",
    //     skill_md: include_str!("../../../../skills/example-skill/SKILL.md"),
    // },
];
```

### Script Extraction

Embedded scripts are extracted to `.g3/bin/` on first use:

1. **First run**: Script is written to `.g3/bin/<script-name>`
2. **Permissions**: Made executable (chmod 755 on Unix)
3. **Version tracking**: Content hash stored in `.g3/bin/<script-name>.version`
4. **Updates**: Re-extracted automatically when embedded version changes

This ensures:
- Scripts are always available, even in fresh workspaces
- Updates propagate automatically when g3 is upgraded
- No manual installation required

## Creating Skills with Scripts

Skills can include executable scripts for complex operations:

### Script Location

Place scripts in the skill directory:

```
skills/my-skill/
├── SKILL.md
├── my-script.sh        # Bash script
├── helper.py           # Python script
└── scripts/
    └── complex-tool    # Subdirectory also works
```

### Referencing Scripts

In SKILL.md, reference scripts relative to the skill directory:

```markdown
## Usage

Run the helper script:
```bash
shell("skills/my-skill/my-script.sh arg1 arg2")
```
```

### Embedding Scripts

To embed scripts in the binary (for core skills), add them to `embedded.rs`:

```rust
EmbeddedSkill {
    name: "my-skill",
    skill_md: include_str!("../../../../skills/my-skill/SKILL.md"),
    scripts: &[
        ("my-script", include_str!("../../../../skills/my-skill/my-script.sh")),
    ],
},
```

## Context Budget

Each skill adds approximately 50-100 tokens to the system prompt (name + description + path). The full SKILL.md body is only loaded when the agent reads it.

**Recommendations**:
- Keep descriptions under 200 characters
- Put detailed instructions in the body, not the description
- Avoid creating many small skills; consolidate related functionality

## Troubleshooting

### Skill Not Discovered

1. Check the skill directory exists and contains `SKILL.md`
2. Verify the frontmatter is valid YAML
3. Ensure `name` and `description` fields are present
4. Check for syntax errors in the YAML (use a YAML validator)

### Skill Overridden Unexpectedly

Skills with the same name are overridden by higher-priority sources. Check:
- `skills/` (repo) overrides everything
- `.g3/skills/` (workspace) overrides global and embedded
- `~/.g3/skills/` (global) overrides embedded only

### Embedded Script Not Found

If `.g3/bin/<script>` doesn't exist:
1. The skill may not have been used yet (extraction is lazy)
2. Check permissions on `.g3/bin/` directory
3. Try deleting `.g3/bin/<script>.version` to force re-extraction

## Adding a New Embedded Skill

To add a new skill to the g3 binary:

1. Create the skill in `skills/<name>/`:
   ```
   skills/new-skill/
   ├── SKILL.md
   └── optional-script.sh
   ```

2. Add to `crates/g3-core/src/skills/embedded.rs`:
   ```rust
   EmbeddedSkill {
       name: "new-skill",
       skill_md: include_str!("../../../../skills/new-skill/SKILL.md"),
       scripts: &[
           ("optional-script", include_str!("../../../../skills/new-skill/optional-script.sh")),
       ],
   },
   ```

3. Rebuild g3:
   ```bash
   cargo build --release
   ```

## See Also

- [Agent Skills Specification](https://agentskills.io) - The open standard
- [Architecture: Skills System](architecture.md#skills-system-extensible-capabilities) - Internal implementation
- [README: Agent Skills](../README.md#agent-skills) - Quick overview
