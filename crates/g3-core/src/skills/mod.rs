//! Agent Skills support for G3.
//!
//! Implements the Agent Skills specification (https://agentskills.io)
//! for discovering and using portable skill packages.
//!
//! # Overview
//!
//! Skills are packages of instructions that give the agent new capabilities.
//! Each skill is a directory containing a `SKILL.md` file with:
//! - YAML frontmatter (name, description, metadata)
//! - Markdown body with detailed instructions
//!
//! # Directory Structure
//!
//! ```text
//! skill-name/
//! ├── SKILL.md          # Required: instructions + metadata
//! ├── scripts/          # Optional: executable code
//! ├── references/       # Optional: additional documentation  
//! └── assets/           # Optional: templates, data files
//! ```
//!
//! # Discovery
//!
//! Skills are discovered from (highest to lowest priority):
//! 1. Repo: `skills/` at repo root (checked into git, overrides all)
//! 2. Workspace: `.g3/skills/` (local customizations)
//! 3. Extra paths from config
//! 4. Global: `~/.g3/skills/`
//! 5. Embedded: compiled into binary (always available)
//!
//! # Usage
//!
//! At startup, g3 scans skill directories and injects a summary into the
//! system prompt. When the agent needs a skill, it reads the full SKILL.md
//! using the `read_file` tool.

mod parser;
mod discovery;
mod prompt;
mod embedded;

pub use parser::Skill;
pub use discovery::discover_skills;
pub use prompt::generate_skills_prompt;
pub use embedded::{get_embedded_skills, get_embedded_skill, EmbeddedSkill};
