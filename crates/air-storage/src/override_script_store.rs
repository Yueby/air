use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use air_config::DEFAULT_OVERRIDE_SCRIPT;
use air_error::{AppResult, StorageError};

use super::{AppPaths, FileStore};

pub const OVERRIDE_SCRIPT_PATH: &str = "override.js";

#[derive(Clone, Debug)]
pub struct OverrideScriptStore {
    paths: AppPaths,
    files: FileStore,
}

impl OverrideScriptStore {
    pub fn new(paths: AppPaths) -> Self {
        let files = FileStore::new(paths.data_dir.clone(), paths.backups_dir.clone());
        Self { paths, files }
    }

    pub fn load_or_default(&self) -> AppResult<String> {
        let target = self.script_path();
        tracing::info!(path = %target.display(), "loading override script");
        match fs::read_to_string(&target) {
            Ok(source) => {
                tracing::info!(path = %target.display(), bytes = source.len(), "loaded override script");
                Ok(source)
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                tracing::info!(path = %target.display(), "override script missing; using built-in default");
                Ok(DEFAULT_OVERRIDE_SCRIPT.to_string())
            }
            Err(error) => Err(StorageError::Io(error).into()),
        }
    }

    pub fn save(&self, script: &str) -> AppResult<()> {
        // 覆写脚本是用户数据，不跟随 config 目录迁移；仍通过 FileStore 原子写和备份保护编辑结果。
        self.files
            .write_bytes(Path::new(OVERRIDE_SCRIPT_PATH), script.as_bytes())
    }

    pub fn script_path(&self) -> PathBuf {
        self.paths.data_dir.join(OVERRIDE_SCRIPT_PATH)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_in_temp() -> (tempfile::TempDir, OverrideScriptStore) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_base_dirs(
            &temp.path().join("config"),
            &temp.path().join("data"),
            &temp.path().join("cache"),
        );
        paths.init().unwrap();
        (temp, OverrideScriptStore::new(paths))
    }

    #[test]
    fn missing_script_uses_default_function() {
        let (_temp, store) = store_in_temp();

        let source = store.load_or_default().unwrap();

        assert!(source.contains("function override"));
    }

    #[test]
    fn saves_script_in_data_dir_with_backup() {
        let (temp, store) = store_in_temp();

        store.save("function override() { return {}; }\n").unwrap();
        store
            .save("function override(_, config) { return config; }\n")
            .unwrap();

        let source = fs::read_to_string(temp.path().join("data/override.js")).unwrap();
        assert!(source.contains("config"));
        assert!(temp.path().join("data/backups/override.js.bak").exists());
    }
}
