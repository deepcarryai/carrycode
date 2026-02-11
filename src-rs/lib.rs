#![deny(clippy::all)]

pub mod llm;
mod lsp;
pub mod cons;
pub mod config;
pub mod ffi;
pub mod session;
pub mod policy;
pub mod skills;

use napi::bindgen_prelude::Result;
use napi_derive::napi;
use std::fs;
use std::sync::Once;

#[global_allocator]
static ALLOC: std::alloc::System = std::alloc::System;

static INIT: Once = Once::new();

pub fn init_logger() {
    INIT.call_once(|| {
        // Set panic hook to log panics
        std::panic::set_hook(Box::new(|info| {
            let msg = match info.payload().downcast_ref::<&'static str>() {
                Some(s) => *s,
                None => match info.payload().downcast_ref::<String>() {
                    Some(s) => &s[..],
                    None => "Box<Any>",
                },
            };
            let location = info.location().map(|l| format!("{}:{}", l.file(), l.line())).unwrap_or_else(|| "unknown".to_string());
            log::error!("CRASH: thread panicked at '{}', {}", msg, location);
        }));

        use log::LevelFilter;
        use log4rs::append::console::ConsoleAppender;
        use log4rs::append::file::FileAppender;
        use log4rs::config::{Appender, Config, Root, Logger};
        use log4rs::encode::pattern::PatternEncoder;

        // Try to load log4rs configuration from file first
        let mut candidates = vec![];
        if let Ok(p) = std::env::var("LOG4RS_CONFIG") {
            candidates.push(std::path::PathBuf::from(p));
        }
        candidates.push(std::path::PathBuf::from("log4rs.yaml"));
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                candidates.push(exe_dir.join("log4rs.yaml"));
            }
        }

        let mut config_loaded = false;
        for path in candidates {
            if path.exists() {
                if let Ok(_) = log4rs::init_file(&path, Default::default()) {
                    log::info!("Logger initialized from {:?}", path);
                    config_loaded = true;
                    break;
                }
            }
        }

        if config_loaded {
            return;
        } else {
            // println!("[INIT] No external log4rs.yaml found, using embedded configuration");
        }

        let pattern = "{d(%Y-%m-%d %H:%M:%S)} [{l}] {t} - {m}\n";
        let session_pattern = "{m}\n";

        let console = ConsoleAppender::builder()
            .encoder(Box::new(PatternEncoder::new(pattern)))
            .build();

        let log_dir = if let Some(home_dir) = dirs::home_dir() {
            home_dir.join(".carry").join("logs")
        } else {
            std::path::PathBuf::from("logs")
        };
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            println!("[INIT] Failed to create log directory: {}", e);
        }

        let logfile = match FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new(pattern)))
            .build(log_dir.join("carry.log")) {
            Ok(f) => f,
            Err(e) => {
                println!("[INIT] Failed to create log file: {}", e);
                return;
            }
        };

        let sessionfile = match FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new(session_pattern)))
            .build(log_dir.join("carry-session.log")) {
            Ok(f) => f,
            Err(e) => {
                println!("[INIT] Failed to create session log file: {}", e);
                return;
            }
        };

        let config = match Config::builder()
            .appender(Appender::builder().build("console", Box::new(console)))
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
            .appender(Appender::builder().build("sessionfile", Box::new(sessionfile)))
            .logger(Logger::builder()
                .appender("sessionfile")
                .additive(false)
                .build("carry_session", LevelFilter::Info))
            .build(Root::builder()
                .appender("logfile")
                .build(LevelFilter::Debug)) {
            Ok(c) => c,
            Err(e) => {
                println!("[INIT] Failed to build config: {}", e);
                return;
            }
        };

        match log4rs::init_config(config) {
            Ok(_) => log::info!("Logger initialized from embedded config"),
            Err(e) => eprintln!("[INIT] Failed to initialize logger: {}", e),
        }
    });
}

#[napi]
pub fn get_log_dir() -> String {
    if let Some(home_dir) = dirs::home_dir() {
        home_dir.join(".carry").join("logs").to_string_lossy().to_string()
    } else {
        "logs".to_string()
    }
}

#[napi]
pub fn get_app_config() -> String {
    let config = match config::AppConfig::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {:?}", e);
            return "{}".to_string();
        }
    };
    serde_json::to_string(&config.to_public()).unwrap_or("{}".to_string())
}

#[napi(object)]
pub struct CoreAvailableModel {
    pub provider: String,
    pub model: String,
}

#[napi]
pub fn list_available_models() -> Result<Vec<CoreAvailableModel>> {
    init_logger();
    let cfg = config::AppConfig::load()
        .map_err(|e| napi::Error::from_reason(format!("Failed to load config: {}", e)))?;
    let mut out = Vec::new();
    for p in cfg.providers {
        for m in p.models {
            out.push(CoreAvailableModel {
                provider: p.name.clone(),
                model: m,
            });
        }
    }
    Ok(out)
}

#[napi]
pub fn get_default_model() -> Result<Option<String>> {
    init_logger();
    let cfg = config::AppConfig::load()
        .map_err(|e| napi::Error::from_reason(format!("Failed to load config: {}", e)))?;
    let Some(raw) = cfg.default_model else {
        return Ok(None);
    };
    let raw = raw.trim().to_string();
    if raw.is_empty() {
        return Ok(None);
    }
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() == 2 {
        return Ok(Some(raw));
    }
    if let Some(p) = cfg.providers.iter().find(|p| p.models.contains(&raw)) {
        return Ok(Some(format!("{}:{}", p.name, raw)));
    }
    Ok(Some(raw))
}

#[napi(object)]
pub struct CoreProviderPreset {
    pub provider_id: String,
    pub provider_brand: String,
    pub base_url: String,
    pub api_key: String,
    pub model_name: String,
    pub provider_desc: String,
}

#[napi]
pub fn list_provider_presets() -> Result<Vec<CoreProviderPreset>> {
    init_logger();
    let cfg = config::AppConfig::load()
        .map_err(|e| napi::Error::from_reason(format!("Failed to load config: {}", e)))?;
    Ok(cfg
        .provider_presets
        .into_iter()
        .map(|p| CoreProviderPreset {
            provider_id: p.provider_id,
            provider_brand: p.provider_brand,
            base_url: p.base_url,
            api_key: p.api_key,
            model_name: p.model_name,
            provider_desc: p.provider_desc,
        })
        .collect())
}

#[napi(object)]
pub struct CoreConfigBootstrapState {
    pub needs_welcome_wizard: bool,
    pub runtime_language: Option<String>,
}

#[napi]
pub fn get_config_bootstrap_state() -> CoreConfigBootstrapState {
    let mut user_empty = true;
    let mut runtime_empty = true;
    let mut wizard_done = false;
    let mut runtime_language: Option<String> = None;

      if let Some(home) = dirs::home_dir() {
          let config_dir = home.join(".carry");
          let user_path = config_dir.join("carrycode.json");
          if user_path.exists() {
              if let Ok(content) = fs::read_to_string(&user_path) {
                  if let Ok(patch) = serde_json::from_str::<config::UserOverrideConfig>(&content) {
                      user_empty = patch.providers.as_ref().map(|p| p.is_empty()).unwrap_or(true);
                  }
              }
          }

          let runtime_path = config_dir.join("carrycode-runtime.json");
        if runtime_path.exists() {
            if let Ok(content) = fs::read_to_string(&runtime_path) {
                match serde_json::from_str::<config::RuntimeConfig>(&content) {
                    Ok(runtime) => {
                        runtime_language = runtime.language.as_ref().and_then(|v| {
                            let v = v.trim().to_string();
                            if v.is_empty() { None } else { Some(v) }
                        });
                        wizard_done = runtime.is_welcome_wizard_done.unwrap_or(false);
                        let has_default_model = runtime
                            .default_model
                            .as_ref()
                            .map(|v| !v.trim().is_empty())
                            .unwrap_or(false);
                        let has_sessions = !session::store::list_saved_sessions().unwrap_or_default().is_empty();
                        let has_language = runtime_language.is_some();
                        runtime_empty = !(has_default_model || has_sessions || has_language);
                    }
                    Err(e) => {
                        log::warn!("Failed to parse runtime config for bootstrap: {}", e);
                    }
                }
            }
        } else {
             // If runtime file doesn't exist, we still check if sessions exist
             let has_sessions = !session::store::list_saved_sessions().unwrap_or_default().is_empty();
             if has_sessions {
                 runtime_empty = false;
             }
        }
    }

    CoreConfigBootstrapState {
        needs_welcome_wizard: (!wizard_done) || (user_empty && runtime_empty),
        runtime_language,
    }
}

#[napi]
pub fn set_language(language: String) -> Result<()> {
    init_logger();
    let language = language.trim().to_string();
    if language.is_empty() {
        return Ok(());
    }
    let mut cfg = config::AppConfig::load()
        .map_err(|e| napi::Error::from_reason(format!("Failed to load config: {}", e)))?;
    cfg.runtime.language = Some(language);
    cfg.save_runtime()
        .map_err(|e| napi::Error::from_reason(format!("Failed to save runtime config: {}", e)))?;
    Ok(())
}

#[napi]
pub fn set_welcome_wizard_done(done: bool) -> Result<()> {
    init_logger();
    let mut cfg = config::AppConfig::load()
        .map_err(|e| napi::Error::from_reason(format!("Failed to load config: {}", e)))?;
    cfg.runtime.is_welcome_wizard_done = Some(done);
    cfg.save_runtime()
        .map_err(|e| napi::Error::from_reason(format!("Failed to save runtime config: {}", e)))?;
    Ok(())
}

#[napi(object)]
pub struct CoreUserProviderConfig {
    pub provider_brand: String,
    pub provider_id: String,
    pub model_name: String,
    pub base_url: String,
    pub api_key: String,
}

#[napi]
pub fn save_user_providers(providers: Vec<CoreUserProviderConfig>) -> Result<()> {
    init_logger();
    let Some(home) = dirs::home_dir() else {
        return Err(napi::Error::from_reason("Failed to resolve home directory"));
    };

      let config_dir = home.join(".carry");
      let user_path = config_dir.join("carrycode.json");

    let mut root: serde_json::Value = if user_path.exists() {
        fs::read_to_string(&user_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if !root.is_object() {
        root = serde_json::json!({});
    }

    let root_obj = root.as_object_mut().expect("just ensured object");

    let existing = root_obj
        .get("providers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut ordered_keys: Vec<String> = Vec::new();
    let mut by_key: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();
    let mut passthrough: Vec<serde_json::Value> = Vec::new();

    for item in existing {
        let provider_id = item
            .get("provider_id")
            .or_else(|| item.get("provider_name"))
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        let model_name = item.get("model_name").and_then(|v| v.as_str()).map(|v| v.to_string());
        match (provider_id, model_name) {
            (Some(pn), Some(mn)) if !pn.trim().is_empty() && !mn.trim().is_empty() => {
                let key = format!("{}:{}", pn, mn);
                if !by_key.contains_key(&key) {
                    ordered_keys.push(key.clone());
                }
                by_key.insert(key, item);
            }
            _ => passthrough.push(item),
        }
    }

    for p in providers {
        let provider_id = p.provider_id.trim().to_string();
        let model_name = p.model_name.trim().to_string();
        if provider_id.is_empty() || model_name.is_empty() {
            continue;
        }
        let key = format!("{}:{}", provider_id, model_name);
        if !by_key.contains_key(&key) {
            ordered_keys.push(key.clone());
        }
        by_key.insert(
            key,
            serde_json::json!({
                "provider_brand": p.provider_brand,
                "provider_id": provider_id,
                "model_name": model_name,
                "base_url": p.base_url,
                "api_key": p.api_key,
            }),
        );
    }

    let mut merged: Vec<serde_json::Value> = Vec::new();
    for k in ordered_keys {
        if let Some(v) = by_key.remove(&k) {
            merged.push(v);
        }
    }
    merged.extend(by_key.into_values());
    merged.extend(passthrough);

    root_obj.insert("providers".to_string(), serde_json::Value::Array(merged));

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| napi::Error::from_reason(format!("Failed to create config dir: {}", e)))?;
    }
    let content = serde_json::to_string_pretty(&root)
        .map_err(|e| napi::Error::from_reason(format!("Failed to serialize user config: {}", e)))?;
    fs::write(&user_path, content)
        .map_err(|e| napi::Error::from_reason(format!("Failed to write user config: {}", e)))?;
    Ok(())
}

// Re-export FFI functions and types
pub use ffi::*;

// Keep legacy function for compatibility
#[napi]
pub fn generate_random_lines() -> Vec<String> {
    init_logger();
    vec!["Legacy function kept for compatibility".to_string()]
}

use crate::ffi::session_util::{list_skills, enable_skill, disable_skill, get_skill_content};
use crate::skills::types::SkillManifest;
use crate::session::SESSION_MANAGER;
use std::sync::Arc;

#[napi(object)]
pub struct CoreSkillManifest {
    pub name: String,
    pub description: Option<String>,
    pub argument_hint: Option<String>,
    pub disable_model_invocation: Option<bool>,
    pub user_invocable: Option<bool>,
    pub allowed_tools: Option<Vec<String>>,
    pub context: Option<String>,
}

impl From<SkillManifest> for CoreSkillManifest {
    fn from(m: SkillManifest) -> Self {
        Self {
            name: m.name,
            description: m.description,
            argument_hint: m.argument_hint,
            disable_model_invocation: m.disable_model_invocation,
            user_invocable: m.user_invocable,
            allowed_tools: m.allowed_tools,
            context: m.context,
        }
    }
}

#[napi]
pub fn list_available_skills(session_id: String) -> Result<Vec<CoreSkillManifest>> {
    list_skills(&session_id)
        .map(|skills| skills.into_iter().map(CoreSkillManifest::from).collect())
        .map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub fn get_skill_markdown(session_id: String, skill_name: String) -> Result<String> {
    get_skill_content(&session_id, &skill_name).map_err(|e| napi::Error::from_reason(e.to_string()))
}

#[napi]
pub async fn enable_skill_for_session(session_id: String, skill_name: String) -> Result<()> {
    let inner = if let Ok(manager) = SESSION_MANAGER.lock() {
        if let Some(ctx) = manager.get(&session_id) {
            Arc::clone(&ctx.inner)
        } else {
            return Err(napi::Error::from_reason("Session not found"));
        }
    } else {
        return Err(napi::Error::from_reason("Failed to lock session manager"));
    };
    
    enable_skill(&session_id, &inner, &skill_name).await
}

#[napi]
pub async fn disable_skill_for_session(session_id: String, skill_name: String) -> Result<()> {
    let inner = if let Ok(manager) = SESSION_MANAGER.lock() {
        if let Some(ctx) = manager.get(&session_id) {
            Arc::clone(&ctx.inner)
        } else {
            return Err(napi::Error::from_reason("Session not found"));
        }
    } else {
        return Err(napi::Error::from_reason("Failed to lock session manager"));
    };

    disable_skill(&session_id, &inner, &skill_name).await
}

#[cfg(test)]
mod tests;
