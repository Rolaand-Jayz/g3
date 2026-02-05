//! Script extraction for embedded skills.
//!
//! Extracts embedded scripts to `.g3/bin/` on first use.
//! Scripts are re-extracted if the embedded version changes.

use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

use super::embedded::get_embedded_skill;

/// Directory where extracted scripts are placed (relative to workspace)
const BIN_DIR: &str = ".g3/bin";

/// Version file to track when scripts need re-extraction
const VERSION_FILE: &str = ".version";

/// Extract a script from an embedded skill to the bin directory.
///
/// Returns the path to the extracted script.
///
/// # Arguments
/// * `skill_name` - Name of the skill containing the script
/// * `script_name` - Name of the script file to extract
/// * `workspace_dir` - Workspace root directory
///
/// # Returns
/// Path to the extracted script, ready to execute.
pub fn extract_script(
    skill_name: &str,
    script_name: &str,
    workspace_dir: &Path,
) -> Result<PathBuf> {
    let skill = get_embedded_skill(skill_name)
        .with_context(|| format!("Embedded skill '{}' not found", skill_name))?;
    
    let script_content = skill
        .scripts
        .iter()
        .find(|(name, _)| *name == script_name)
        .map(|(_, content)| *content)
        .with_context(|| format!("Script '{}' not found in skill '{}'", script_name, skill_name))?;
    
    let bin_dir = workspace_dir.join(BIN_DIR);
    fs::create_dir_all(&bin_dir)
        .with_context(|| format!("Failed to create bin directory: {}", bin_dir.display()))?;
    
    let script_path = bin_dir.join(script_name);
    let version_path = bin_dir.join(format!("{}{}", script_name, VERSION_FILE));
    
    // Check if we need to extract (script missing or version changed)
    let needs_extraction = if !script_path.exists() {
        debug!("Script {} does not exist, extracting", script_path.display());
        true
    } else if needs_update(&version_path, script_content)? {
        debug!("Script {} is outdated, re-extracting", script_path.display());
        true
    } else {
        debug!("Script {} is up to date", script_path.display());
        false
    };
    
    if needs_extraction {
        // Write the script
        fs::write(&script_path, script_content)
            .with_context(|| format!("Failed to write script: {}", script_path.display()))?;
        
        // Make it executable (Unix only)
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&script_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms)?;
        }
        
        // Write version file (content hash)
        let hash = compute_hash(script_content);
        fs::write(&version_path, hash)
            .with_context(|| format!("Failed to write version file: {}", version_path.display()))?;
        
        info!("Extracted {} to {}", script_name, script_path.display());
    }
    
    Ok(script_path)
}

/// Extract all scripts from an embedded skill.
///
/// Returns a vector of (script_name, script_path) pairs.
pub fn extract_all_scripts(
    skill_name: &str,
    workspace_dir: &Path,
) -> Result<Vec<(String, PathBuf)>> {
    let skill = get_embedded_skill(skill_name)
        .with_context(|| format!("Embedded skill '{}' not found", skill_name))?;
    
    let mut extracted = Vec::new();
    
    for (script_name, _) in skill.scripts {
        let path = extract_script(skill_name, script_name, workspace_dir)?;
        extracted.push((script_name.to_string(), path));
    }
    
    Ok(extracted)
}

/// Check if a script needs to be updated based on version file.
fn needs_update(version_path: &Path, current_content: &str) -> Result<bool> {
    if !version_path.exists() {
        return Ok(true);
    }
    
    let stored_hash = fs::read_to_string(version_path)
        .with_context(|| format!("Failed to read version file: {}", version_path.display()))?;
    
    let current_hash = compute_hash(current_content);
    
    Ok(stored_hash.trim() != current_hash)
}

/// Compute a simple hash of content for version tracking.
/// Uses a fast non-cryptographic hash.
fn compute_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Get the path where a script would be extracted.
/// Does not actually extract the script.
pub fn get_script_path(script_name: &str, workspace_dir: &Path) -> PathBuf {
    workspace_dir.join(BIN_DIR).join(script_name)
}

/// Check if a script has been extracted.
pub fn is_script_extracted(script_name: &str, workspace_dir: &Path) -> bool {
    get_script_path(script_name, workspace_dir).exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_extract_research_script() {
        let temp = TempDir::new().unwrap();
        
        let result = extract_script("research", "g3-research", temp.path());
        assert!(result.is_ok(), "Should extract research script: {:?}", result.err());
        
        let script_path = result.unwrap();
        assert!(script_path.exists(), "Script should exist after extraction");
        
        // Check it's executable
        #[cfg(unix)]
        {
            let metadata = fs::metadata(&script_path).unwrap();
            let mode = metadata.permissions().mode();
            assert!(mode & 0o111 != 0, "Script should be executable");
        }
        
        // Check content
        let content = fs::read_to_string(&script_path).unwrap();
        assert!(content.starts_with("#!/bin/bash"), "Should be a bash script");
    }

    #[test]
    fn test_extract_idempotent() {
        let temp = TempDir::new().unwrap();
        
        // Extract twice
        let path1 = extract_script("research", "g3-research", temp.path()).unwrap();
        let path2 = extract_script("research", "g3-research", temp.path()).unwrap();
        
        assert_eq!(path1, path2, "Should return same path");
    }

    #[test]
    fn test_version_tracking() {
        let temp = TempDir::new().unwrap();
        
        // Extract
        extract_script("research", "g3-research", temp.path()).unwrap();
        
        // Version file should exist
        let version_path = temp.path().join(".g3/bin/g3-research.version");
        assert!(version_path.exists(), "Version file should exist");
        
        let hash = fs::read_to_string(&version_path).unwrap();
        assert!(!hash.is_empty(), "Version file should contain hash");
    }

    #[test]
    fn test_nonexistent_skill() {
        let temp = TempDir::new().unwrap();
        
        let result = extract_script("nonexistent", "script", temp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_nonexistent_script() {
        let temp = TempDir::new().unwrap();
        
        let result = extract_script("research", "nonexistent", temp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_get_script_path() {
        let workspace = Path::new("/workspace");
        let path = get_script_path("g3-research", workspace);
        assert_eq!(path, PathBuf::from("/workspace/.g3/bin/g3-research"));
    }

    #[test]
    fn test_compute_hash() {
        let hash1 = compute_hash("hello world");
        let hash2 = compute_hash("hello world");
        let hash3 = compute_hash("different content");
        
        assert_eq!(hash1, hash2, "Same content should produce same hash");
        assert_ne!(hash1, hash3, "Different content should produce different hash");
        assert_eq!(hash1.len(), 16, "Hash should be 16 hex chars");
    }
}
