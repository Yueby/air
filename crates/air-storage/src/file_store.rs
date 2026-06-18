use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use air_error::{AppResult, StorageError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoredFormat {
    Json,
    Yaml,
}

#[derive(Clone, Debug)]
pub struct FileStore {
    root: PathBuf,
    backups_dir: PathBuf,
}

impl FileStore {
    pub fn new(root: PathBuf, backups_dir: PathBuf) -> Self {
        Self { root, backups_dir }
    }

    pub fn read<T>(&self, path: &Path, format: StoredFormat) -> AppResult<T>
    where
        T: DeserializeOwned,
    {
        let target = self.resolve_path(path)?;
        tracing::debug!(
            path = %target.display(),
            format = ?format,
            "reading stored file"
        );
        let bytes = fs::read(target).map_err(StorageError::Io)?;
        match format {
            StoredFormat::Json => Ok(serde_json::from_slice(&bytes).map_err(StorageError::Json)?),
            StoredFormat::Yaml => Ok(serde_yaml::from_slice(&bytes).map_err(StorageError::Yaml)?),
        }
    }

    pub fn write<T>(&self, path: &Path, value: &T, format: StoredFormat) -> AppResult<()>
    where
        T: Serialize,
    {
        let bytes = match format {
            StoredFormat::Json => serde_json::to_vec_pretty(value).map_err(StorageError::Json)?,
            StoredFormat::Yaml => serde_yaml::to_string(value)
                .map_err(StorageError::Yaml)?
                .into_bytes(),
        };
        self.write_bytes(path, &bytes)
    }

    pub fn write_bytes(&self, path: &Path, bytes: &[u8]) -> AppResult<()> {
        let target = self.resolve_path(path)?;
        tracing::info!(
            path = %target.display(),
            bytes = bytes.len(),
            "writing stored file atomically"
        );
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(StorageError::Io)?;
        }
        fs::create_dir_all(&self.backups_dir).map_err(StorageError::Io)?;
        if target.exists() {
            let backup = self.backup_path(&target);
            if let Some(parent) = backup.parent() {
                fs::create_dir_all(parent).map_err(StorageError::Io)?;
            }
            fs::copy(&target, backup).map_err(StorageError::Io)?;
            tracing::debug!(path = %target.display(), "backed up existing file before overwrite");
        }

        // 使用同目录临时文件后 rename，确保异常中断时不会留下半截配置。
        let parent = target
            .parent()
            .ok_or_else(|| StorageError::UnsafePath(target.clone()))?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(StorageError::Io)?;
        temp.write_all(bytes).map_err(StorageError::Io)?;
        temp.flush().map_err(StorageError::Io)?;
        temp.persist(&target)
            .map_err(|error| StorageError::Io(error.error))?;
        tracing::info!(path = %target.display(), "stored file write completed");
        Ok(())
    }

    fn resolve_path(&self, path: &Path) -> AppResult<PathBuf> {
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(StorageError::UnsafePath(path.to_path_buf()).into());
        }
        if path.is_absolute() && !path.starts_with(&self.root) {
            return Err(StorageError::UnsafePath(path.to_path_buf()).into());
        }
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(self.root.join(path))
        }
    }

    fn backup_path(&self, target: &Path) -> PathBuf {
        let file_name = target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("backup");
        self.backups_dir.join(format!("{file_name}.bak"))
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct Sample {
        name: String,
    }

    #[test]
    fn writes_atomically_and_creates_backup() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("config");
        let backups = temp.path().join("backups");
        fs::create_dir_all(&root).unwrap();
        let store = FileStore::new(root.clone(), backups.clone());
        let target = root.join("settings.json");

        store
            .write(&target, &Sample { name: "old".into() }, StoredFormat::Json)
            .unwrap();
        store
            .write(&target, &Sample { name: "new".into() }, StoredFormat::Json)
            .unwrap();

        let loaded: Sample = store.read(&target, StoredFormat::Json).unwrap();
        assert_eq!(loaded.name, "new");
        assert!(backups.join("settings.json.bak").exists());
    }

    #[test]
    fn refuses_absolute_path_outside_root() {
        let temp = tempfile::tempdir().unwrap();
        let store = FileStore::new(temp.path().join("root"), temp.path().join("backups"));
        let other = temp.path().join("other.json");

        let error = store
            .write(&other, &Sample { name: "x".into() }, StoredFormat::Json)
            .unwrap_err();
        assert!(matches!(error, air_error::AppError::Storage(_)));
    }

    #[test]
    fn resolves_relative_paths_under_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let store = FileStore::new(root.clone(), temp.path().join("backups"));

        store
            .write(
                Path::new("profiles/default.yaml"),
                &Sample {
                    name: "profile".into(),
                },
                StoredFormat::Yaml,
            )
            .unwrap();

        assert!(root.join("profiles/default.yaml").exists());
    }

    #[test]
    fn refuses_parent_dir_escape() {
        let temp = tempfile::tempdir().unwrap();
        let store = FileStore::new(temp.path().join("root"), temp.path().join("backups"));

        let error = store
            .write(
                Path::new("../escape.json"),
                &Sample { name: "x".into() },
                StoredFormat::Json,
            )
            .unwrap_err();
        assert!(matches!(error, air_error::AppError::Storage(_)));
    }
}
