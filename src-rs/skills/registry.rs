use super::types::{Skill, SkillManifest};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    pub fn load_from_paths(paths: Vec<PathBuf>) -> Self {
        let mut registry = Self::new();
        // Load in order provided. Later entries override earlier ones.
        for path in paths {
            registry.scan_directory(&path);
        }
        registry
    }

    fn scan_directory(&mut self, root: &Path) {
        if !root.exists() {
            return;
        }
        log::info!("Scanning skills in {:?}", root);

        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() && entry.file_name() == "SKILL.md" {
                match Self::parse_skill(entry.path()) {
                    Ok(skill) => {
                        log::debug!("Loaded skill '{}' from {:?}", skill.manifest.name, entry.path());
                        self.skills.insert(skill.manifest.name.clone(), skill);
                    }
                    Err(e) => {
                        log::warn!("Failed to parse skill at {:?}: {}", entry.path(), e);
                    }
                }
            }
        }
    }

    fn parse_skill(path: &Path) -> Result<Skill> {
        let content = std::fs::read_to_string(path)?;
        
        // Regex to match YAML frontmatter: ^---\n(.*?)\n---\n(.*)$
        // Note: .*? matches non-greedy.
        // We use (?s) to enable dot matches newline for the content part.
        // But for frontmatter we want to capture between first two --- lines.
        
        // Simple manual parsing might be more robust than regex for --- separators
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        
        if parts.len() < 3 {
             // Maybe no frontmatter? But spec says required.
             // Try to see if it starts with ---
             if content.starts_with("---") {
                 // It might be malformed
                 return Err(anyhow::anyhow!("Malformed frontmatter"));
             } else {
                 // No frontmatter. Use directory name as name?
                 // Spec says "Every skill... must start with YAML frontmatter".
                 // But let's be lenient or strict? User said "fully compatible", Claude requires it.
                 return Err(anyhow::anyhow!("Missing YAML frontmatter"));
             }
        }

        // parts[0] is empty (before first ---)
        // parts[1] is yaml
        // parts[2] is markdown
        
        let yaml_str = parts[1];
        let markdown_body = parts[2].trim().to_string();

        let mut manifest: SkillManifest = serde_yaml::from_str(yaml_str)
            .context("Failed to parse YAML frontmatter")?;

        // If name is missing in manifest, use directory name
        if manifest.name.is_empty() {
             if let Some(parent) = path.parent() {
                 if let Some(dir_name) = parent.file_name() {
                     manifest.name = dir_name.to_string_lossy().to_string();
                 }
             }
        }

        Ok(Skill {
            manifest,
            instruction: markdown_body,
            path: path.to_path_buf(),
        })
    }
    
    pub fn list(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }
    
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    pub fn resolve_system_prompt_injection(&self, enabled_skills: &[String]) -> String {
        let mut injection = String::new();
        for name in enabled_skills {
            if let Some(skill) = self.get(name) {
                injection.push_str("\n\n<skill_instruction name=\"");
                injection.push_str(&skill.manifest.name);
                injection.push_str("\">\n");
                injection.push_str(&skill.instruction);
                injection.push_str("\n</skill_instruction>");
            }
        }
        injection
    }
}

pub fn load_standard_skills() -> SkillRegistry {
    let mut paths = Vec::new();
    
    // 1. User level: ~/.claude/skills
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".claude/skills"));
        paths.push(home.join(".carry/skills"));
    }
    
    // 2. Project level: .claude/skills, .carry/skills
    // We assume current working directory is the project root for CLI
    if let Ok(cwd) = std::env::current_dir() {
    paths.push(cwd.join(".claude/skills"));
    paths.push(cwd.join(".carry/skills"));
    }
    
    SkillRegistry::load_from_paths(paths)
}
