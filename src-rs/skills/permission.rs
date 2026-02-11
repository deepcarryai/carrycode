use super::registry::SkillRegistry;
use std::collections::HashSet;

/// Checks if a tool is allowed to be used given the set of enabled skills.
/// 
/// Rule:
/// 1. If no enabled skills specify `allowed-tools`, then all tools are allowed (returns true).
/// 2. If any enabled skill specifies `allowed-tools`, then the tool MUST be present in the
///    union of all allowed tools from all enabled skills.
pub fn check_tool_permission(enabled_skills: &[String], registry: &SkillRegistry, tool_name: &str) -> bool {
    let mut restricted_mode = false;
    let mut allowed_set: HashSet<String> = HashSet::new();

    for skill_name in enabled_skills {
        if let Some(skill) = registry.get(skill_name) {
            if let Some(allowed_list) = &skill.manifest.allowed_tools {
                restricted_mode = true;
                for tool in allowed_list {
                    allowed_set.insert(tool.to_lowercase());
                }
            }
        }
    }

    if !restricted_mode {
        // No skills restrict tools, so allow everything (subject to global approval policy)
        return true;
    }

    // Check if tool_name is in the allowed set (case-insensitive)
    allowed_set.contains(&tool_name.to_lowercase())
}
