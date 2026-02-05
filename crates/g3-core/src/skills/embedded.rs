//! Embedded skills - compiled into the binary for portability.
//!
//! Core skills are embedded at compile time using `include_str!`.
//! This ensures g3 works anywhere without needing external skill files.
//!
//! Priority order (highest to lowest):
//! 1. Repo `skills/` directory (on disk, checked into git)
//! 2. Workspace `.g3/skills/` directory
//! 3. Config extra_paths
//! 4. Global `~/.g3/skills/` directory
//! 5. Embedded skills (this module - always available)

/// An embedded skill with its SKILL.md content and optional scripts.
#[derive(Debug, Clone)]
pub struct EmbeddedSkill {
    /// Skill name (must match the name in SKILL.md frontmatter)
    pub name: &'static str,
    /// Content of SKILL.md
    pub skill_md: &'static str,
}

/// All embedded skills, compiled into the binary.
///
/// To add a new embedded skill:
/// 1. Create `skills/<name>/SKILL.md` in the repo
/// 2. Add an entry here with `include_str!`
static EMBEDDED_SKILLS: &[EmbeddedSkill] = &[];

/// Get all embedded skills.
pub fn get_embedded_skills() -> &'static [EmbeddedSkill] {
    EMBEDDED_SKILLS
}

/// Get an embedded skill by name.
pub fn get_embedded_skill(name: &str) -> Option<&'static EmbeddedSkill> {
    EMBEDDED_SKILLS.iter().find(|s| s.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_skills_empty() {
        // Currently no embedded skills - research was moved to first-class tool
        let skills = get_embedded_skills();
        assert!(skills.is_empty(), "No embedded skills expected");
    }

    #[test]
    fn test_get_nonexistent_skill() {
        assert!(get_embedded_skill("nonexistent").is_none());
    }
}
