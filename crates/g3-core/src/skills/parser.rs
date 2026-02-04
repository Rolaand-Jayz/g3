//! SKILL.md parser for Agent Skills specification.
//!
//! Parses YAML frontmatter and markdown body from SKILL.md files.
//! See: https://agentskills.io/specification

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A parsed Agent Skill from a SKILL.md file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Skill name (1-64 chars, lowercase alphanumeric + hyphens)
    pub name: String,
    /// Description of what the skill does and when to use it (1-1024 chars)
    pub description: String,
    /// Optional license
    pub license: Option<String>,
    /// Optional compatibility requirements (max 500 chars)
    pub compatibility: Option<String>,
    /// Optional arbitrary metadata
    pub metadata: Option<HashMap<String, String>>,
    /// Optional allowed tools (experimental)
    pub allowed_tools: Option<String>,
    /// The full markdown body (after frontmatter)
    pub body: String,
    /// Absolute path to the SKILL.md file
    pub path: String,
}

/// Raw frontmatter structure for deserialization
#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    license: Option<String>,
    compatibility: Option<String>,
    metadata: Option<HashMap<String, String>>,
    #[serde(rename = "allowed-tools")]
    allowed_tools: Option<String>,
}

impl Skill {
    /// Parse a SKILL.md file from its content and path.
    ///
    /// The file must have YAML frontmatter delimited by `---` lines,
    /// with at least `name` and `description` fields.
    pub fn parse(content: &str, path: &Path) -> Result<Self> {
        let (frontmatter, body) = split_frontmatter(content)?;
        
        let fm: SkillFrontmatter = serde_yaml::from_str(&frontmatter)
            .map_err(|e| anyhow!("Invalid YAML frontmatter in {}: {}", path.display(), e))?;
        
        // Validate required fields
        let name = fm.name.ok_or_else(|| {
            anyhow!("Missing required 'name' field in {}", path.display())
        })?;
        
        let description = fm.description.ok_or_else(|| {
            anyhow!("Missing required 'description' field in {}", path.display())
        })?;
        
        // Validate name format: 1-64 chars, lowercase alphanumeric + hyphens
        validate_name(&name, path)?;
        
        // Validate description length: 1-1024 chars
        if description.is_empty() {
            return Err(anyhow!("Description cannot be empty in {}", path.display()));
        }
        if description.len() > 1024 {
            return Err(anyhow!(
                "Description exceeds 1024 characters ({} chars) in {}",
                description.len(),
                path.display()
            ));
        }
        
        // Validate compatibility length if present
        if let Some(ref compat) = fm.compatibility {
            if compat.len() > 500 {
                return Err(anyhow!(
                    "Compatibility exceeds 500 characters ({} chars) in {}",
                    compat.len(),
                    path.display()
                ));
            }
        }
        
        // Get absolute path
        let abs_path = path.canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_string();
        
        Ok(Skill {
            name,
            description,
            license: fm.license,
            compatibility: fm.compatibility,
            metadata: fm.metadata,
            allowed_tools: fm.allowed_tools,
            body: body.to_string(),
            path: abs_path,
        })
    }
    
    /// Parse a SKILL.md file from disk.
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read {}: {}", path.display(), e))?;
        Self::parse(&content, path)
    }
}

/// Split content into frontmatter and body.
/// Frontmatter must be delimited by `---` lines at the start.
fn split_frontmatter(content: &str) -> Result<(String, String)> {
    let content = content.trim_start();
    
    // Must start with ---
    if !content.starts_with("---") {
        return Err(anyhow!("SKILL.md must start with YAML frontmatter (---)"));
    }
    
    // Find the closing ---
    let after_first = &content[3..];
    let closing_pos = after_first.find("\n---")
        .ok_or_else(|| anyhow!("SKILL.md frontmatter not closed (missing ---)"))?;
    
    let frontmatter = after_first[..closing_pos].trim();
    let body = after_first[closing_pos + 4..].trim_start(); // Skip "\n---"
    
    if frontmatter.is_empty() {
        return Err(anyhow!("SKILL.md frontmatter is empty"));
    }
    
    Ok((frontmatter.to_string(), body.to_string()))
}

/// Validate skill name format.
/// Must be 1-64 chars, lowercase alphanumeric + hyphens,
/// no leading/trailing/consecutive hyphens.
fn validate_name(name: &str, path: &Path) -> Result<()> {
    if name.is_empty() {
        return Err(anyhow!("Skill name cannot be empty in {}", path.display()));
    }
    
    if name.len() > 64 {
        return Err(anyhow!(
            "Skill name exceeds 64 characters ({} chars) in {}",
            name.len(),
            path.display()
        ));
    }
    
    // Check for valid characters
    for c in name.chars() {
        if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-' {
            return Err(anyhow!(
                "Skill name contains invalid character '{}' in {} (must be lowercase alphanumeric or hyphen)",
                c,
                path.display()
            ));
        }
    }
    
    // No leading/trailing hyphens
    if name.starts_with('-') || name.ends_with('-') {
        return Err(anyhow!(
            "Skill name cannot start or end with hyphen in {}",
            path.display()
        ));
    }
    
    // No consecutive hyphens
    if name.contains("--") {
        return Err(anyhow!(
            "Skill name cannot contain consecutive hyphens in {}",
            path.display()
        ));
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    
    fn test_path() -> PathBuf {
        PathBuf::from("/test/skill/SKILL.md")
    }
    
    #[test]
    fn test_parse_valid_skill() {
        let content = r#"---
name: pdf-processing
description: Extract text and tables from PDF files using pdfplumber
license: Apache-2.0
compatibility: Requires Python 3.8+
metadata:
  author: example-org
  version: "1.0"
---

# PDF Processing

Use this skill when working with PDF files.
"#;
        
        let skill = Skill::parse(content, &test_path()).unwrap();
        assert_eq!(skill.name, "pdf-processing");
        assert_eq!(skill.description, "Extract text and tables from PDF files using pdfplumber");
        assert_eq!(skill.license, Some("Apache-2.0".to_string()));
        assert_eq!(skill.compatibility, Some("Requires Python 3.8+".to_string()));
        assert!(skill.body.contains("# PDF Processing"));
        
        let metadata = skill.metadata.unwrap();
        assert_eq!(metadata.get("author"), Some(&"example-org".to_string()));
        assert_eq!(metadata.get("version"), Some(&"1.0".to_string()));
    }
    
    #[test]
    fn test_parse_minimal_skill() {
        let content = r#"---
name: simple-skill
description: A simple skill
---

Body content here.
"#;
        
        let skill = Skill::parse(content, &test_path()).unwrap();
        assert_eq!(skill.name, "simple-skill");
        assert_eq!(skill.description, "A simple skill");
        assert!(skill.license.is_none());
        assert!(skill.compatibility.is_none());
        assert!(skill.metadata.is_none());
    }
    
    #[test]
    fn test_missing_name() {
        let content = r#"---
description: A skill without a name
---

Body.
"#;
        
        let err = Skill::parse(content, &test_path()).unwrap_err();
        assert!(err.to_string().contains("Missing required 'name'"));
    }
    
    #[test]
    fn test_missing_description() {
        let content = r#"---
name: no-description
---

Body.
"#;
        
        let err = Skill::parse(content, &test_path()).unwrap_err();
        assert!(err.to_string().contains("Missing required 'description'"));
    }
    
    #[test]
    fn test_invalid_name_uppercase() {
        let content = r#"---
name: Invalid-Name
description: Has uppercase
---

Body.
"#;
        
        let err = Skill::parse(content, &test_path()).unwrap_err();
        assert!(err.to_string().contains("invalid character"));
    }
    
    #[test]
    fn test_invalid_name_leading_hyphen() {
        let content = r#"---
name: -leading-hyphen
description: Bad name
---

Body.
"#;
        
        let err = Skill::parse(content, &test_path()).unwrap_err();
        assert!(err.to_string().contains("cannot start or end with hyphen"));
    }
    
    #[test]
    fn test_invalid_name_consecutive_hyphens() {
        let content = r#"---
name: double--hyphen
description: Bad name
---

Body.
"#;
        
        let err = Skill::parse(content, &test_path()).unwrap_err();
        assert!(err.to_string().contains("consecutive hyphens"));
    }
    
    #[test]
    fn test_missing_frontmatter() {
        let content = "# Just markdown\n\nNo frontmatter here.";
        
        let err = Skill::parse(content, &test_path()).unwrap_err();
        assert!(err.to_string().contains("must start with YAML frontmatter"));
    }
    
    #[test]
    fn test_unclosed_frontmatter() {
        let content = r#"---
name: unclosed
description: Missing closing delimiter

Body without closing ---
"#;
        
        let err = Skill::parse(content, &test_path()).unwrap_err();
        assert!(err.to_string().contains("not closed"));
    }
    
    #[test]
    fn test_empty_frontmatter() {
        let content = "---\n---\n\nBody.";
        
        let err = Skill::parse(content, &test_path()).unwrap_err();
        assert!(err.to_string().contains("frontmatter is empty"));
    }
    
    #[test]
    fn test_description_too_long() {
        let long_desc = "x".repeat(1025);
        let content = format!("---\nname: long-desc\ndescription: {}\n---\n\nBody.", long_desc);
        
        let err = Skill::parse(&content, &test_path()).unwrap_err();
        assert!(err.to_string().contains("exceeds 1024 characters"));
    }
    
    #[test]
    fn test_allowed_tools_field() {
        let content = r#"---
name: git-skill
description: Git operations
allowed-tools: Bash(git:*) Read
---

Body.
"#;
        
        let skill = Skill::parse(content, &test_path()).unwrap();
        assert_eq!(skill.allowed_tools, Some("Bash(git:*) Read".to_string()));
    }
}
