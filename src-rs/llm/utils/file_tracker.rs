use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::SystemTime;

pub static FILE_READ_TRACKER: LazyLock<Mutex<FileReadTracker>> =
    LazyLock::new(|| Mutex::new(FileReadTracker::new()));

pub static FILE_HISTORY_TRACKER: LazyLock<Mutex<FileHistoryTracker>> =
    LazyLock::new(|| Mutex::new(FileHistoryTracker::new()));

#[derive(Debug, Clone)]
pub struct FileVersion {
    pub content: String,
}

#[derive(Debug)]
pub struct FileHistoryTracker {
    histories: HashMap<String, Vec<FileVersion>>,
}

impl FileHistoryTracker {
    pub fn new() -> Self {
        Self {
            histories: HashMap::new(),
        }
    }

    pub fn record_version(&mut self, path: &str, content: String) {
        let canonical_path = fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
        let path_str = canonical_path.to_string_lossy().to_string();

        let version = FileVersion { content };

        self.histories
            .entry(path_str)
            .or_insert_with(Vec::new)
            .push(version);
    }

    pub fn get_versions(&self, path: &str) -> Option<&Vec<FileVersion>> {
        let canonical_path = match fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => return None,
        };
        let path_str = canonical_path.to_string_lossy().to_string();
        self.histories.get(&path_str)
    }

    pub fn get_latest_version(&self, path: &str) -> Option<&FileVersion> {
        self.get_versions(path)?.last()
    }
}

#[derive(Debug)]
pub struct FileReadTracker {
    reads: HashMap<String, SystemTime>,
}

impl FileReadTracker {
    pub fn new() -> Self {
        Self {
            reads: HashMap::new(),
        }
    }

    pub fn record_read(&mut self, path: &str) {
        let canonical_path = fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
        let path_str = canonical_path.to_string_lossy().to_string();
        self.reads.insert(path_str, SystemTime::now());
    }

    pub fn has_been_read(&self, path: &str) -> bool {
        let canonical_path = match fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let path_str = canonical_path.to_string_lossy().to_string();
        self.reads.contains_key(&path_str)
    }

    pub fn get_last_read_time(&self, path: &str) -> Option<SystemTime> {
        let canonical_path = match fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => return None,
        };
        let path_str = canonical_path.to_string_lossy().to_string();
        self.reads.get(&path_str).copied()
    }
}

pub struct PathSecurity;

impl PathSecurity {
    pub fn to_absolute_path<P: AsRef<Path>>(path: P) -> Result<String> {
        let path = path.as_ref();

        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .context("Failed to get current directory")?
                .join(path)
        };

        Ok(absolute_path.to_string_lossy().to_string())
    }

    pub fn get_modification_time<P: AsRef<Path>>(path: P) -> Result<SystemTime> {
        let path = path.as_ref();
        let metadata = fs::metadata(path)
            .with_context(|| format!("Failed to get file metadata: {}", path.display()))?;

        metadata
            .modified()
            .with_context(|| format!("Failed to get modification time: {}", path.display()))
    }
}

