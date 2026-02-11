use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SkillManifest {
    /// Display name for the skill. If omitted (in file), usually uses directory name, but here we expect it parsed.
    pub name: String,
    /// What the skill does and when to use it.
    pub description: Option<String>,
    /// Hint shown during autocomplete.
    pub argument_hint: Option<String>,
    /// Set to true to prevent Claude from automatically loading this skill.
    pub disable_model_invocation: Option<bool>,
    /// Whether user can invoke it manually (defaults to true usually).
    pub user_invocable: Option<bool>,
    /// Whitelist of tools this skill is allowed to use.
    pub allowed_tools: Option<Vec<String>>,
    /// Execution context ("fork" or "none").
    pub context: Option<String>,
    
    /// Catch-all for other fields to ensure forward compatibility
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub manifest: SkillManifest,
    pub instruction: String,
    pub path: PathBuf,
}
