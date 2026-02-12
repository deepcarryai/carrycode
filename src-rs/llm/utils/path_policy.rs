use anyhow::{bail, Context, Result};
use std::path::{Component, Path, PathBuf};
use crate::llm::utils::tool_access::is_full_access;

#[derive(Debug, Clone)]
pub struct PathPolicy {
    root: PathBuf,
    root_depth: usize,
}

impl PathPolicy {
    pub fn new() -> Result<Self> {
        if is_full_access() {
            let root = PathBuf::from("/");
            return Ok(Self { root, root_depth: 1 });
        }
        let root = std::fs::canonicalize(std::env::current_dir()?).context("Failed to determine workspace root")?;
        let root_depth = root.components().count();
        Ok(Self { root, root_depth })
    }

    pub fn resolve(&self, input: &str) -> Result<PathBuf> {
        let requested = Path::new(input);

        let mut components: Vec<Component> = if requested.is_absolute() {
            if let Ok(stripped) = requested.strip_prefix(&self.root) {
                stripped.components().collect()
            } else {
                bail!(
                    "Path '{}' is outside the workspace '{}'",
                    requested.display(),
                    self.root.display()
                );
            }
        } else {
            requested.components().collect()
        };

        let mut normalized = self.root.clone();

        for comp in components.drain(..) {
            match comp {
                Component::CurDir => {}
                Component::ParentDir => {
                    if normalized.components().count() > self.root_depth {
                        normalized.pop();
                    }
                }
                Component::Normal(c) => normalized.push(c),
                _ => {}
            }
        }

        Ok(normalized)
    }
}
