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
//! Skills are discovered from:
//! 1. Global: `~/.g3/skills/` (lowest priority)
//! 2. Extra paths from config (medium priority)
//! 3. Workspace: `.g3/skills/` (highest priority, overrides others)
//!
//! # Usage
//!
//! At startup, g3 scans skill directories and injects a summary into the
//! system prompt. When the agent needs a skill, it reads the full SKILL.md
//! using the `read_file` tool.

mod parser;
mod discovery;
mod prompt;

pub use parser::Skill;
pub use discovery::discover_skills;
pub use prompt::generate_skills_prompt;
