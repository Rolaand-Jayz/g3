//! Generate XML prompt section for available skills.
//!
//! Creates the `<available_skills>` XML block that gets injected into
//! the system prompt to inform the agent about available skills.

use super::parser::Skill;

/// Generate the XML section for available skills.
///
/// Returns an empty string if no skills are available.
/// The XML format follows the Agent Skills specification.
pub fn generate_skills_prompt(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    
    let mut xml = String::new();
    xml.push_str("# Available Skills\n\n");
    xml.push_str("You have access to the following skills. When a task matches a skill's description, \
                  read the full skill file using `read_file` to get detailed instructions.\n\n");
    xml.push_str("<available_skills>\n");
    
    for skill in skills {
        xml.push_str("  <skill>\n");
        xml.push_str(&format!("    <name>{}</name>\n", escape_xml(&skill.name)));
        xml.push_str(&format!("    <description>{}</description>\n", escape_xml(&skill.description)));
        // Don't escape location - it's a path the LLM needs to use with read_file
        // Embedded paths like <embedded:name>/SKILL.md must remain unescaped
        xml.push_str(&format!("    <location>{}</location>\n", &skill.path));
        
        // Include compatibility info if present
        if let Some(ref compat) = skill.compatibility {
            xml.push_str(&format!("    <compatibility>{}</compatibility>\n", escape_xml(compat)));
        }
        
        xml.push_str("  </skill>\n");
    }
    
    xml.push_str("</available_skills>\n");
    xml
}

/// Escape special XML characters.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn make_skill(name: &str, description: &str, path: &str) -> Skill {
        Skill {
            name: name.to_string(),
            description: description.to_string(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: None,
            body: String::new(),
            path: path.to_string(),
        }
    }
    
    #[test]
    fn test_empty_skills() {
        let result = generate_skills_prompt(&[]);
        assert!(result.is_empty());
    }
    
    #[test]
    fn test_single_skill() {
        let skills = vec![
            make_skill("pdf-processing", "Extract text from PDFs", "/home/user/.g3/skills/pdf-processing/SKILL.md"),
        ];
        
        let result = generate_skills_prompt(&skills);
        
        assert!(result.contains("<available_skills>"));
        assert!(result.contains("</available_skills>"));
        assert!(result.contains("<name>pdf-processing</name>"));
        assert!(result.contains("<description>Extract text from PDFs</description>"));
        assert!(result.contains("<location>/home/user/.g3/skills/pdf-processing/SKILL.md</location>"));
    }
    
    #[test]
    fn test_multiple_skills() {
        let skills = vec![
            make_skill("skill-a", "First skill", "/path/a/SKILL.md"),
            make_skill("skill-b", "Second skill", "/path/b/SKILL.md"),
        ];
        
        let result = generate_skills_prompt(&skills);
        
        assert!(result.contains("<name>skill-a</name>"));
        assert!(result.contains("<name>skill-b</name>"));
        // Should have exactly 2 skill blocks
        assert_eq!(result.matches("<skill>").count(), 2);
        assert_eq!(result.matches("</skill>").count(), 2);
    }
    
    #[test]
    fn test_xml_escaping() {
        let skills = vec![
            make_skill("test-skill", "Handle <special> & \"characters\"", "/path/SKILL.md"),
        ];
        
        let result = generate_skills_prompt(&skills);
        
        assert!(result.contains("&lt;special&gt;"));
        assert!(result.contains("&amp;"));
        assert!(result.contains("&quot;characters&quot;"));
        // Should not contain unescaped special chars in description
        assert!(!result.contains("<special>"));
    }
    
    #[test]
    fn test_with_compatibility() {
        let mut skill = make_skill("docker-skill", "Docker operations", "/path/SKILL.md");
        skill.compatibility = Some("Requires Docker 20.0+".to_string());
        
        let result = generate_skills_prompt(&[skill]);
        
        assert!(result.contains("<compatibility>Requires Docker 20.0+</compatibility>"));
    }
    
    #[test]
    fn test_header_text() {
        let skills = vec![
            make_skill("test", "Test skill", "/path/SKILL.md"),
        ];
        
        let result = generate_skills_prompt(&skills);
        
        assert!(result.contains("# Available Skills"));
        assert!(result.contains("read the full skill file using `read_file`"));
    }

    #[test]
    fn test_embedded_skill_path_not_escaped() {
        // Embedded skill paths use <embedded:name> syntax which must NOT be escaped
        // so the LLM can use them directly with read_file
        let skills = vec![
            make_skill("example-skill", "Example embedded skill", "<embedded:example-skill>/SKILL.md"),
        ];
        
        let result = generate_skills_prompt(&skills);
        
        // The path should appear unescaped
        assert!(result.contains("<location><embedded:example-skill>/SKILL.md</location>"));
        // Should NOT be escaped
        assert!(!result.contains("&lt;embedded:research&gt;"));
    }
}
