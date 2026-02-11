use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LLMProvider {
    OpenAI,
    Claude,
    Gemini,
    Codex,
    ZhipuAI,
    DeepSeek,
    Qwen,
}

impl LLMProvider {
    /// Returns the unique organization identifier used in configuration (e.g., "openai", "claude")
    pub fn provider_name(&self) -> &'static str {
        match self {
            LLMProvider::OpenAI => "openai",
            LLMProvider::Claude => "claude",
            LLMProvider::Gemini => "gemini",
            LLMProvider::Codex => "codex",
            LLMProvider::ZhipuAI => "zhipuai",
            LLMProvider::DeepSeek => "deepseek",
            LLMProvider::Qwen => "qwen",
        }
    }

    /// Helper to parse from a string (handles aliases)
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "openai" => Some(LLMProvider::OpenAI),
            "claude" | "anthropic" => Some(LLMProvider::Claude),
            "gemini" => Some(LLMProvider::Gemini),
            "codex" => Some(LLMProvider::Codex),
            "zhipuai" => Some(LLMProvider::ZhipuAI),
            "deepseek" => Some(LLMProvider::DeepSeek),
            "qwen" => Some(LLMProvider::Qwen),
            _ => None,
        }
    }
}

// Ensure Display trait matches provider_name for convenience
impl std::fmt::Display for LLMProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.provider_name())
    }
}
