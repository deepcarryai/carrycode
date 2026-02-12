#![deny(clippy::all)]

mod llm;
mod lsp;
pub mod config;
mod ffi;
pub mod session;

use napi::bindgen_prelude::Result;
use napi_derive::napi;
use std::sync::Once;

static INIT: Once = Once::new();

pub fn init_logger() {
    INIT.call_once(|| {
        use log::LevelFilter;
        use log4rs::append::file::FileAppender;
        use log4rs::config::{Appender, Config, Root};
        use log4rs::encode::pattern::PatternEncoder;

        // Try to load log4rs configuration from file first
        let config_path = std::env::var("LOG4RS_CONFIG").unwrap_or_else(|_| "log4rs.yaml".to_string());
        let _ = std::fs::create_dir_all("logs");
        if let Ok(_) = log4rs::init_file(config_path.clone(), Default::default()) {
            println!("[INIT] Logger initialized from {}", config_path);
            return;
        } else {
            println!("[INIT] Failed to load {}, falling back to default config", config_path);
        }

        let pattern = "{d(%Y-%m-%d %H:%M:%S)} [{l}] {t} - {m}\n";

        let logfile = match FileAppender::builder()
            .encoder(Box::new(PatternEncoder::new(pattern)))
            .build("logs/carrycode.log") {
            Ok(f) => f,
            Err(e) => {
                println!("[INIT] Failed to create log file: {}", e);
                return;
            }
        };

        let config = match Config::builder()
            .appender(Appender::builder().build("logfile", Box::new(logfile)))
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
            Ok(_) => println!("[INIT] Logger initialized successfully"),
            Err(e) => println!("[INIT] Failed to initialize logger: {}", e),
        }
    });
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
    serde_json::to_string(&config).unwrap_or("{}".to_string())
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

// Re-export FFI functions and types
pub use ffi::*;

// Keep legacy function for compatibility
#[napi]
pub fn generate_random_lines() -> Vec<String> {
    init_logger();
    vec!["Legacy function kept for compatibility".to_string()]
}
