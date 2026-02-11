#[cfg(test)]
pub mod config;

#[cfg(test)]
pub mod session {
    pub mod confirm;
    pub mod store;
}

#[cfg(test)]
pub mod ffi {
    pub mod session_util;
}

#[cfg(test)]
pub mod llm {
    pub mod models {
        pub mod gemini;
        pub mod claude;
        pub mod openai;
        pub mod codex;
        pub mod provider_handle;
    }
    pub mod agents {
        pub mod agent;
    }
    pub mod tools {
        pub mod builtin {
            pub mod core_edit;
            pub mod core_bash;
            pub mod core_tool_base;
            pub mod core_write;
        }
    }
}

#[cfg(test)]
pub mod policy {
    pub mod policy_text;
}

#[cfg(test)]
pub mod skills;