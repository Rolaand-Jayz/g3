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
    /// Scripts bundled with the skill: (filename, content)
    pub scripts: &'static [(&'static str, &'static str)],
}

/// All embedded skills, compiled into the binary.
///
/// To add a new embedded skill:
/// 1. Create `skills/<name>/SKILL.md` in the repo
/// 2. Add an entry here with `include_str!`
static EMBEDDED_SKILLS: &[EmbeddedSkill] = &[
    EmbeddedSkill {
        name: "research",
        skill_md: include_str!("../../../../skills/research/SKILL.md"),
        scripts: &[
            ("g3-research", include_str!("../../../../skills/research/g3-research")),
        ],
    },
];

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
    fn test_embedded_skills_exist() {
        let skills = get_embedded_skills();
        assert!(!skills.is_empty(), "Should have at least one embedded skill");
    }

    #[test]
    fn test_research_skill_embedded() {
        let skill = get_embedded_skill("research");
        assert!(skill.is_some(), "Research skill should be embedded");
        
        let skill = skill.unwrap();
        assert!(skill.skill_md.contains("name: research"), "SKILL.md should have name field");
        assert!(!skill.scripts.is_empty(), "Research skill should have scripts");
    }

    #[test]
    fn test_get_by_name() {
        assert!(get_embedded_skill("research").is_some());
        assert!(get_embedded_skill("nonexistent").is_none());
    }
}
