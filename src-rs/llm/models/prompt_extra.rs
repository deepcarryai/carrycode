use crate::cons::provider_cons::LLMProvider;

/// Extra prompt fragments for specific LLM providers to improve their behavior.
pub const ZHIPU_PROMPT_EXTRA: &str = "\n\n[STRICT FORMAT RULE]\n- Always use the provided function-calling tools for any external actions.\n- DO NOT encapsulate tool calls in XML-like tags such as <tool_call>, <arg_key>, or <arg_value>.\n- Provide tool arguments only in the required JSON format through the standard API mechanism.";

/// Returns the extra prompt text for a specific provider brand if defined.
pub fn get_extra_prompt_for_provider(brand: &str) -> Option<&'static str> {
    match LLMProvider::from_name(brand) {
        Some(LLMProvider::ZhipuAI) => Some(ZHIPU_PROMPT_EXTRA),
        _ => None,
    }
}
