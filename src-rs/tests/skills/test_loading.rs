use crate::skills::registry::SkillRegistry;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_load_skill() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("demo-skill");
    fs::create_dir(&skill_dir).unwrap();
    
    let content = r#"---
name: demo-skill
description: A demo skill
allowed-tools:
  - read
---
This is a demo skill instruction.
"#;
    fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    
    let registry = SkillRegistry::load_from_paths(vec![temp.path().to_path_buf()]);
    let skills = registry.list();
    
    assert_eq!(skills.len(), 1);
    let skill = skills[0];
    assert_eq!(skill.manifest.name, "demo-skill");
    assert_eq!(skill.manifest.description.as_deref(), Some("A demo skill"));
    assert_eq!(skill.manifest.allowed_tools.as_deref(), Some(&vec!["read".to_string()][..]));
    assert_eq!(skill.instruction, "This is a demo skill instruction.");
}

#[test]
fn test_load_skill_override() {
    let temp_user = TempDir::new().unwrap();
    let temp_proj = TempDir::new().unwrap();
    
    // User skill
    let user_skill_dir = temp_user.path().join("foo");
    fs::create_dir(&user_skill_dir).unwrap();
    fs::write(user_skill_dir.join("SKILL.md"), "---\nname: foo\ndescription: user\n---\nUser body").unwrap();
    
    // Project skill (same name)
    let proj_skill_dir = temp_proj.path().join("foo");
    fs::create_dir(&proj_skill_dir).unwrap();
    fs::write(proj_skill_dir.join("SKILL.md"), "---\nname: foo\ndescription: project\n---\nProject body").unwrap();
    
    let registry = SkillRegistry::load_from_paths(vec![
        temp_user.path().to_path_buf(),
        temp_proj.path().to_path_buf()
    ]);
    
    let skill = registry.get("foo").unwrap();
    assert_eq!(skill.manifest.description.as_deref(), Some("project"));
    assert_eq!(skill.instruction, "Project body");
}
