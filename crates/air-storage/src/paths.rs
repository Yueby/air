use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use air_error::{AppResult, StorageError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub subscription_cache_dir: PathBuf,
    pub cores_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub backups_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> AppResult<Self> {
        let dirs =
            ProjectDirs::from("org.air", "", "Air").ok_or(StorageError::ProjectDirsUnavailable)?;
        let paths = Self::from_base_dirs(dirs.config_dir(), dirs.data_dir(), dirs.cache_dir());
        tracing::info!(
            config_dir = %paths.config_dir.display(),
            data_dir = %paths.data_dir.display(),
            cache_dir = %paths.cache_dir.display(),
            "resolved application paths"
        );
        Ok(paths)
    }

    pub fn from_base_dirs(config_dir: &Path, data_dir: &Path, cache_dir: &Path) -> Self {
        // Windows/macOS/Linux 的系统目录不同，但业务层只依赖这些语义化子目录。
        Self {
            config_dir: config_dir.to_path_buf(),
            data_dir: data_dir.to_path_buf(),
            cache_dir: cache_dir.to_path_buf(),
            subscription_cache_dir: config_dir.join("subscriptions"),
            cores_dir: cache_dir.join("core"),
            logs_dir: data_dir.join("logs"),
            backups_dir: data_dir.join("backups"),
        }
    }

    pub fn init(&self) -> AppResult<()> {
        for dir in [
            &self.config_dir,
            &self.data_dir,
            &self.cache_dir,
            &self.subscription_cache_dir,
            &self.cores_dir,
            &self.logs_dir,
            &self.backups_dir,
        ] {
            std::fs::create_dir_all(dir).map_err(StorageError::Io)?;
            tracing::debug!(path = %dir.display(), "ensured application directory exists");
        }
        tracing::info!("initialized application directory layout");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_semantic_subdirectories_from_platform_roots() {
        let paths = AppPaths::from_base_dirs(
            Path::new("/config/air"),
            Path::new("/data/air"),
            Path::new("/cache/air"),
        );

        assert_eq!(
            paths.subscription_cache_dir,
            PathBuf::from("/config/air/subscriptions")
        );
        assert_eq!(paths.cores_dir, PathBuf::from("/cache/air/core"));
        assert_eq!(paths.backups_dir, PathBuf::from("/data/air/backups"));
    }

    #[test]
    fn keeps_same_layout_for_common_platform_roots() {
        // 目录库会按平台返回不同根目录；这里验证业务子目录在三类根目录下保持一致。
        for (config, data, cache) in [
            (
                r"C:\Users\Alice\AppData\Roaming\dev\air\air\config",
                r"C:\Users\Alice\AppData\Roaming\dev\air\air\data",
                r"C:\Users\Alice\AppData\Local\dev\air\air\cache",
            ),
            (
                "/Users/alice/Library/Application Support/dev.air.air",
                "/Users/alice/Library/Application Support/dev.air.air",
                "/Users/alice/Library/Caches/dev.air.air",
            ),
            (
                "/home/alice/.config/air",
                "/home/alice/.local/share/air",
                "/home/alice/.cache/air",
            ),
        ] {
            let paths =
                AppPaths::from_base_dirs(Path::new(config), Path::new(data), Path::new(cache));

            assert!(paths.subscription_cache_dir.ends_with("subscriptions"));
            assert!(paths.cores_dir.ends_with("core"));
            assert!(paths.logs_dir.ends_with("logs"));
            assert!(paths.backups_dir.ends_with("backups"));
        }
    }
}
