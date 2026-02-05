//! Skill discovery - scans directories for SKILL.md files.
//!
//! Discovers skills from:
//! - Global: ~/.g3/skills/
//! - Workspace: .g3/skills/
//!
//! Workspace skills override global skills with the same name.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use super::parser::Skill;

/// Default global skills directory
const GLOBAL_SKILLS_DIR: &str = "~/.g3/skills";

/// Default workspace skills directory (relative to workspace root)
const WORKSPACE_SKILLS_DIR: &str = ".g3/skills";

/// Discover all available skills from configured paths.
///
/// Skills are loaded from:
/// 1. Global directory (~/.g3/skills/)
/// 2. Workspace directory (.g3/skills/)
///
/// Workspace skills override global skills with the same name.
/// Additional paths can be provided via `extra_paths`.
pub fn discover_skills(
    workspace_dir: Option<&Path>,
    extra_paths: &[PathBuf],
) -> Vec<Skill> {
    let mut skills_by_name: HashMap<String, Skill> = HashMap::new();
    
    // 1. Load global skills first (lowest priority)
    let global_dir = expand_tilde(GLOBAL_SKILLS_DIR);
    if global_dir.exists() {
        debug!("Scanning global skills directory: {}", global_dir.display());
        load_skills_from_dir(&global_dir, &mut skills_by_name);
    }
    
    // 2. Load from extra paths (medium priority)
    for path in extra_paths {
        let expanded = if path.starts_with("~") {
            expand_tilde(&path.to_string_lossy())
        } else {
            path.clone()
        };
        if expanded.exists() {
            debug!("Scanning extra skills directory: {}", expanded.display());
            load_skills_from_dir(&expanded, &mut skills_by_name);
        }
    }
    
    // 3. Load workspace skills last (highest priority - overrides others)
    if let Some(workspace) = workspace_dir {
        let workspace_skills = workspace.join(WORKSPACE_SKILLS_DIR);
        if workspace_skills.exists() {
            debug!("Scanning workspace skills directory: {}", workspace_skills.display());
            load_skills_from_dir(&workspace_skills, &mut skills_by_name);
        }
    }
    
    // Convert to sorted vector for deterministic ordering
    let mut skills: Vec<Skill> = skills_by_name.into_values().collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    
    debug!("Discovered {} skills", skills.len());
    skills
}

/// Load skills from a directory into the map.
/// Each subdirectory should contain a SKILL.md file.
fn load_skills_from_dir(dir: &Path, skills: &mut HashMap<String, Skill>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            warn!("Failed to read skills directory {}: {}", dir.display(), e);
            return;
        }
    };
    
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        
        // Skip non-directories
        if !path.is_dir() {
            continue;
        }
        
        // Look for SKILL.md in this directory
        let skill_file = path.join("SKILL.md");
        if !skill_file.exists() {
            // Also check for lowercase variant
            let skill_file_lower = path.join("skill.md");
            if skill_file_lower.exists() {
                load_skill_file(&skill_file_lower, skills);
            }
            continue;
        }
        
        load_skill_file(&skill_file, skills);
    }
}

/// Load a single skill file and add to the map.
fn load_skill_file(path: &Path, skills: &mut HashMap<String, Skill>) {
    match Skill::from_file(path) {
        Ok(skill) => {
            let name = skill.name.clone();
            if skills.contains_key(&name) {
                debug!("Skill '{}' overridden by {}", name, path.display());
            }
            skills.insert(name, skill);
        }
        Err(e) => {
            warn!("Failed to parse skill {}: {}", path.display(), e);
        }
    }
}

/// Expand tilde in path to home directory.
fn expand_tilde(path: &str) -> PathBuf {
    let expanded = shellexpand::tilde(path);
    PathBuf::from(expanded.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    
    fn create_skill_dir(parent: &Path, name: &str, description: &str) -> PathBuf {
        let skill_dir = parent.join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        
        let content = format!(
            "---\nname: {}\ndescription: {}\n---\n\n# {}\n\nSkill body.",
            name, description, name
        );
        fs::write(skill_dir.join("SKILL.md"), content).unwrap();
        
        skill_dir
    }
    
    #[test]
    fn test_discover_from_workspace() {
        let temp = TempDir::new().unwrap();
        let workspace = temp.path();
        
        // Create workspace skills directory
        let skills_dir = workspace.join(".g3/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        
        create_skill_dir(&skills_dir, "test-skill", "A test skill");
        create_skill_dir(&skills_dir, "another-skill", "Another skill");
        
        let skills = discover_skills(Some(workspace), &[]);
        
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].name, "another-skill"); // Sorted alphabetically
        assert_eq!(skills[1].name, "test-skill");
    }
    
    #[test]
    fn test_discover_from_extra_paths() {
        let temp = TempDir::new().unwrap();
        let extra_dir = temp.path().join("extra-skills");
        fs::create_dir_all(&extra_dir).unwrap();
        
        create_skill_dir(&extra_dir, "extra-skill", "An extra skill");
        
        let skills = discover_skills(None, &[extra_dir]);
        
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "extra-skill");
    }
    
    #[test]
    fn test_workspace_overrides_extra() {
        let temp = TempDir::new().unwrap();
        let workspace = temp.path();
        
        // Create extra skills directory
        let extra_dir = temp.path().join("extra");
        fs::create_dir_all(&extra_dir).unwrap();
        create_skill_dir(&extra_dir, "shared-skill", "Extra version");
        
        // Create workspace skills directory with same skill name
        let workspace_skills = workspace.join(".g3/skills");
        fs::create_dir_all(&workspace_skills).unwrap();
        create_skill_dir(&workspace_skills, "shared-skill", "Workspace version");
        
        let skills = discover_skills(Some(workspace), &[extra_dir]);
        
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "shared-skill");
        assert_eq!(skills[0].description, "Workspace version");
    }
    
    #[test]
    fn test_nonexistent_directory() {
        let skills = discover_skills(Some(Path::new("/nonexistent/path")), &[]);
        assert!(skills.is_empty());
    }
    
    #[test]
    fn test_empty_directory() {
        let temp = TempDir::new().unwrap();
        let skills_dir = temp.path().join(".g3/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        
        let skills = discover_skills(Some(temp.path()), &[]);
        assert!(skills.is_empty());
    }
    
    #[test]
    fn test_invalid_skill_skipped() {
        let temp = TempDir::new().unwrap();
        let skills_dir = temp.path().join(".g3/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        
        // Create valid skill
        create_skill_dir(&skills_dir, "valid-skill", "Valid");
        
        // Create invalid skill (missing description)
        let invalid_dir = skills_dir.join("invalid-skill");
        fs::create_dir_all(&invalid_dir).unwrap();
        fs::write(
            invalid_dir.join("SKILL.md"),
            "---\nname: invalid-skill\n---\n\nNo description."
        ).unwrap();
        
        let skills = discover_skills(Some(temp.path()), &[]);
        
        // Only valid skill should be loaded
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "valid-skill");
    }
    
    #[test]
    fn test_lowercase_skill_md() {
        let temp = TempDir::new().unwrap();
        let skills_dir = temp.path().join(".g3/skills");
        let skill_dir = skills_dir.join("lowercase-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        
        // Use lowercase skill.md
        fs::write(
            skill_dir.join("skill.md"),
            "---\nname: lowercase-skill\ndescription: Uses lowercase filename\n---\n\nBody."
        ).unwrap();
        
        let skills = discover_skills(Some(temp.path()), &[]);
        
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "lowercase-skill");
    }
    
    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/test/path");
        assert!(!expanded.to_string_lossy().starts_with('~'));
        
        let no_tilde = expand_tilde("/absolute/path");
        assert_eq!(no_tilde, PathBuf::from("/absolute/path"));
    }
}
