//! Skill discovery - scans directories for SKILL.md files.
//!
//! Discovers skills from (highest to lowest priority):
//! 1. Repo: `skills/` at repo root (checked into git, overrides all)
//! 2. Workspace: `.g3/skills/` (local customizations)
//! 3. Extra paths from config
//! 4. Global: `~/.g3/skills/`
//! 5. Embedded: compiled into binary (always available)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

use super::embedded::get_embedded_skills;
use super::parser::Skill;

/// Default global skills directory
const GLOBAL_SKILLS_DIR: &str = "~/.g3/skills";

/// Default workspace skills directory (relative to workspace root)
const WORKSPACE_SKILLS_DIR: &str = ".g3/skills";

/// Repo-local skills directory (relative to workspace root, checked into git)
const REPO_SKILLS_DIR: &str = "skills";

/// Discover all available skills from configured paths.
///
/// Skills are loaded in priority order (lowest to highest):
/// 1. Embedded skills (compiled into binary)
/// 2. Global directory (~/.g3/skills/)
/// 3. Extra paths from config
/// 4. Workspace directory (.g3/skills/)
/// 5. Repo directory (skills/) - highest priority
///
/// Higher priority skills override lower priority skills with the same name.
pub fn discover_skills(
    workspace_dir: Option<&Path>,
    extra_paths: &[PathBuf],
) -> Vec<Skill> {
    let mut skills_by_name: HashMap<String, Skill> = HashMap::new();
    
    // 1. Load embedded skills first (lowest priority)
    load_embedded_skills(&mut skills_by_name);
    
    // 2. Load global skills
    let global_dir = expand_tilde(GLOBAL_SKILLS_DIR);
    if global_dir.exists() {
        debug!("Scanning global skills directory: {}", global_dir.display());
        load_skills_from_dir(&global_dir, &mut skills_by_name);
    }
    
    // 3. Load from extra paths
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
    
    // 4. Load workspace skills (.g3/skills/)
    if let Some(workspace) = workspace_dir {
        let workspace_skills = workspace.join(WORKSPACE_SKILLS_DIR);
        if workspace_skills.exists() {
            debug!("Scanning workspace skills directory: {}", workspace_skills.display());
            load_skills_from_dir(&workspace_skills, &mut skills_by_name);
        }
    }
    
    // 5. Load repo skills (skills/) - highest priority
    if let Some(workspace) = workspace_dir {
        let repo_skills = workspace.join(REPO_SKILLS_DIR);
        if repo_skills.exists() {
            debug!("Scanning repo skills directory: {}", repo_skills.display());
            load_skills_from_dir(&repo_skills, &mut skills_by_name);
        }
    }
    
    // Convert to sorted vector for deterministic ordering
    let mut skills: Vec<Skill> = skills_by_name.into_values().collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    
    debug!("Discovered {} skills", skills.len());
    skills
}

/// Load embedded skills into the map.
fn load_embedded_skills(skills: &mut HashMap<String, Skill>) {
    for embedded in get_embedded_skills() {
        match Skill::parse(embedded.skill_md, Path::new("<embedded>")) {
            Ok(mut skill) => {
                // Mark as embedded in the path
                skill.path = format!("<embedded:{}>/{}", embedded.name, "SKILL.md");
                debug!("Loaded embedded skill: {}", skill.name);
                skills.insert(skill.name.clone(), skill);
            }
            Err(e) => {
                warn!("Failed to parse embedded skill '{}': {}", embedded.name, e);
            }
        }
    }
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
    fn test_discover_embedded_skills() {
        // With no directories and no embedded skills, should return empty
        let skills = discover_skills(None, &[]);
        
        // No embedded skills currently (research was moved to first-class tool)
        assert!(skills.is_empty(), "Should have no skills when no directories provided");
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
        
        // Should have workspace skills
        assert!(skills.iter().any(|s| s.name == "test-skill"));
        assert!(skills.iter().any(|s| s.name == "another-skill"));
    }
    
    #[test]
    fn test_discover_from_repo_skills() {
        let temp = TempDir::new().unwrap();
        let workspace = temp.path();
        
        // Create repo skills directory (skills/)
        let skills_dir = workspace.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        
        create_skill_dir(&skills_dir, "repo-skill", "A repo skill");
        
        let skills = discover_skills(Some(workspace), &[]);
        
        assert!(skills.iter().any(|s| s.name == "repo-skill"));
    }
    
    #[test]
    fn test_repo_skill_discovery() {
        let temp = TempDir::new().unwrap();
        let workspace = temp.path();
        
        // Create repo skills directory
        let skills_dir = workspace.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        
        create_skill_dir(&skills_dir, "custom-skill", "Custom skill");
        
        let skills = discover_skills(Some(workspace), &[]);
        
        let custom = skills.iter().find(|s| s.name == "custom-skill").unwrap();
        assert_eq!(custom.description, "Custom skill");
    }
    
    #[test]
    fn test_repo_overrides_workspace() {
        let temp = TempDir::new().unwrap();
        let workspace = temp.path();
        
        // Create workspace skill
        let workspace_skills = workspace.join(".g3/skills");
        fs::create_dir_all(&workspace_skills).unwrap();
        create_skill_dir(&workspace_skills, "shared-skill", "Workspace version");
        
        // Create repo skill with same name (should override)
        let repo_skills = workspace.join("skills");
        fs::create_dir_all(&repo_skills).unwrap();
        create_skill_dir(&repo_skills, "shared-skill", "Repo version");
        
        let skills = discover_skills(Some(workspace), &[]);
        
        let shared = skills.iter().find(|s| s.name == "shared-skill").unwrap();
        assert_eq!(shared.description, "Repo version");
    }
    
    #[test]
    fn test_discover_from_extra_paths() {
        let temp = TempDir::new().unwrap();
        let extra_dir = temp.path().join("extra-skills");
        fs::create_dir_all(&extra_dir).unwrap();
        
        create_skill_dir(&extra_dir, "extra-skill", "An extra skill");
        
        let skills = discover_skills(None, &[extra_dir]);
        
        assert!(skills.iter().any(|s| s.name == "extra-skill"));
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
        
        let shared = skills.iter().find(|s| s.name == "shared-skill").unwrap();
        assert_eq!(shared.description, "Workspace version");
    }
    
    #[test]
    fn test_nonexistent_directory() {
        let skills = discover_skills(Some(Path::new("/nonexistent/path")), &[]);
        // No embedded skills, so should be empty
        assert!(skills.is_empty());
    }
    
    #[test]
    fn test_empty_directory() {
        let temp = TempDir::new().unwrap();
        let skills_dir = temp.path().join(".g3/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        
        let skills = discover_skills(Some(temp.path()), &[]);
        // No embedded skills and empty directory, so should be empty
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
        
        // Valid skill should be loaded, invalid should be skipped
        assert!(skills.iter().any(|s| s.name == "valid-skill"));
        assert!(!skills.iter().any(|s| s.name == "invalid-skill"));
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
        
        assert!(skills.iter().any(|s| s.name == "lowercase-skill"));
    }
    
    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/test/path");
        assert!(!expanded.to_string_lossy().starts_with('~'));
        
        let no_tilde = expand_tilde("/absolute/path");
        assert_eq!(no_tilde, PathBuf::from("/absolute/path"));
    }
}
