use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::llm::models::provider_handle::Message;

pub const SESSION_SNAPSHOT_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub version: u16,
    pub session_id: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub agent_mode: String,
    pub approval_mode: String,
    #[serde(default)]
    pub enabled_skills: Option<Vec<String>>,
    #[serde(default)]
    pub title: Option<String>,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub version: u16,
    pub session_id: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub message_count: usize,
    #[serde(default)]
    pub agent_mode: String,
    #[serde(default)]
    pub approval_mode: String,
    #[serde(default)]
    pub enabled_skills: Option<Vec<String>>,
    #[serde(default)]
    pub title: Option<String>,
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn validate_session_id(session_id: &str) -> Result<()> {
    if session_id.is_empty() {
        anyhow::bail!("session_id is empty");
    }
    if session_id.len() > 128 {
        anyhow::bail!("session_id too long");
    }
    let ok = session_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !ok {
        anyhow::bail!("invalid session_id");
    }
    Ok(())
}

fn sessions_root_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".carry").join("sessions"))
}

fn session_dir(session_id: &str) -> Result<PathBuf> {
    validate_session_id(session_id)?;
    let root = sessions_root_dir().context("failed to determine home directory")?;
    Ok(root.join(session_id))
}

fn snapshot_path(session_id: &str) -> Result<PathBuf> {
    Ok(session_dir(session_id)?.join("snapshot.json"))
}

fn meta_path(session_id: &str) -> Result<PathBuf> {
    Ok(session_dir(session_id)?.join("meta.json"))
}

fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let parent = path
        .parent()
        .context("missing parent directory for atomic write")?;
    if !parent.exists() {
        fs::create_dir_all(parent).context("failed to create session directory")?;
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let tmp_path = parent.join(format!("{file_name}.tmp.{}", now_ms()));

    fs::write(&tmp_path, content).context("failed to write tmp file")?;
    fs::rename(&tmp_path, path).context("failed to rename tmp file")?;
    Ok(())
}

pub fn load_snapshot(session_id: &str) -> Result<Option<SessionSnapshot>> {
    let path = snapshot_path(session_id)?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).context("failed to read snapshot file")?;
    let snapshot: SessionSnapshot =
        serde_json::from_str(&content).context("failed to parse snapshot file")?;
    if snapshot.version != SESSION_SNAPSHOT_VERSION {
        return Ok(None);
    }
    Ok(Some(snapshot))
}

pub fn save_snapshot(mut snapshot: SessionSnapshot) -> Result<()> {
    let existing = load_meta(&snapshot.session_id).ok().flatten();
    if let Some(meta) = existing {
        snapshot.created_at_ms = meta.created_at_ms;
    } else if snapshot.created_at_ms <= 0 {
        snapshot.created_at_ms = now_ms();
    }
    snapshot.updated_at_ms = now_ms();
    snapshot.version = SESSION_SNAPSHOT_VERSION;

    let snapshot_json =
        serde_json::to_string_pretty(&snapshot).context("failed to serialize snapshot")?;
    atomic_write(&snapshot_path(&snapshot.session_id)?, &snapshot_json)?;

    let meta = SessionMeta {
        version: SESSION_SNAPSHOT_VERSION,
        session_id: snapshot.session_id.clone(),
        created_at_ms: snapshot.created_at_ms,
        updated_at_ms: snapshot.updated_at_ms,
        message_count: snapshot.messages.len(),
        agent_mode: snapshot.agent_mode.clone(),
        approval_mode: snapshot.approval_mode.clone(),
        enabled_skills: snapshot.enabled_skills.clone(),
        title: snapshot.title.clone(),
    };
    let meta_json = serde_json::to_string_pretty(&meta).context("failed to serialize meta")?;
    atomic_write(&meta_path(&meta.session_id)?, &meta_json)?;
    Ok(())
}

pub fn load_meta(session_id: &str) -> Result<Option<SessionMeta>> {
    let path = meta_path(session_id)?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).context("failed to read meta file")?;
    let meta: SessionMeta = serde_json::from_str(&content).context("failed to parse meta file")?;
    if meta.version != SESSION_SNAPSHOT_VERSION {
        return Ok(None);
    }
    Ok(Some(meta))
}

pub fn list_saved_sessions() -> Result<Vec<SessionMeta>> {
    let root = sessions_root_dir().context("failed to determine home directory")?;
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut metas: Vec<SessionMeta> = Vec::new();
    for entry in fs::read_dir(&root).context("failed to read sessions directory")? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let session_id = match path.file_name().and_then(|n| n.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        if validate_session_id(&session_id).is_err() {
            continue;
        }

        match load_meta(&session_id) {
            Ok(Some(meta)) => metas.push(meta),
            Ok(None) => {
                if let Ok(Some(snapshot)) = load_snapshot(&session_id) {
                    metas.push(SessionMeta {
                        version: SESSION_SNAPSHOT_VERSION,
                        session_id: snapshot.session_id,
                        created_at_ms: snapshot.created_at_ms,
                        updated_at_ms: snapshot.updated_at_ms,
                        message_count: snapshot.messages.len(),
                        agent_mode: snapshot.agent_mode,
                        approval_mode: snapshot.approval_mode,
                        enabled_skills: snapshot.enabled_skills,
                        title: snapshot.title,
                    });
                }
            }
            Err(_) => {}
        }
    }

    metas.sort_by(|a, b| b.session_id.cmp(&a.session_id));
    Ok(metas)
}

