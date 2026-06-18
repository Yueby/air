use std::collections::BTreeSet;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use air_error::{AppResult, ProcessError};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RuntimeDiagnosticKind {
    CoreNotFound,
    VersionUnparseable,
    PermissionDenied,
    ControllerUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeDiagnostic {
    pub kind: RuntimeDiagnosticKind,
    pub message: String,
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MihomoRuntimeInfo {
    pub binary_path: Option<PathBuf>,
    pub version: Option<String>,
    pub executable: bool,
    pub controller_reachable: Option<bool>,
    pub diagnostics: Vec<RuntimeDiagnostic>,
}

#[derive(Clone, Debug)]
pub struct RuntimeDetectionOptions {
    pub user_config_dir: Option<PathBuf>,
    pub managed_core_dir: PathBuf,
    pub controller_addr: Option<String>,
    pub version_timeout: Duration,
    pub connect_timeout: Duration,
}

impl RuntimeDetectionOptions {
    pub fn new(
        user_config_dir: Option<PathBuf>,
        managed_core_dir: PathBuf,
        controller_addr: Option<String>,
    ) -> Self {
        Self {
            user_config_dir,
            managed_core_dir,
            controller_addr,
            version_timeout: Duration::from_secs(3),
            connect_timeout: Duration::from_millis(500),
        }
    }
}

#[async_trait]
pub trait RuntimeDetector: Send + Sync {
    async fn detect_runtime(
        &self,
        options: RuntimeDetectionOptions,
    ) -> AppResult<MihomoRuntimeInfo>;
}

#[derive(Clone, Debug, Default)]
pub struct MihomoRuntimeDetector;

#[async_trait]
impl RuntimeDetector for MihomoRuntimeDetector {
    async fn detect_runtime(
        &self,
        options: RuntimeDetectionOptions,
    ) -> AppResult<MihomoRuntimeInfo> {
        let candidates = find_mihomo_candidates(&options);
        let mut diagnostics = Vec::new();
        let Some(binary_path) = candidates.into_iter().next() else {
            diagnostics.push(RuntimeDiagnostic {
                kind: RuntimeDiagnosticKind::CoreNotFound,
                message: "未在用户配置目录、托管目录或 PATH 中找到 mihomo 可执行文件".into(),
                path: None,
            });
            return Ok(MihomoRuntimeInfo {
                binary_path: None,
                version: None,
                executable: false,
                controller_reachable: None,
                diagnostics,
            });
        };

        let executable = is_executable(&binary_path);
        if !executable {
            diagnostics.push(RuntimeDiagnostic {
                kind: RuntimeDiagnosticKind::PermissionDenied,
                message: "mihomo 文件存在但当前用户没有执行权限".into(),
                path: Some(binary_path.clone()),
            });
        }

        let version = if executable {
            match read_mihomo_version(&binary_path, options.version_timeout).await {
                Ok(Some(version)) => Some(version),
                Ok(None) => {
                    diagnostics.push(RuntimeDiagnostic {
                        kind: RuntimeDiagnosticKind::VersionUnparseable,
                        message: "mihomo -v 输出无法解析为版本号".into(),
                        path: Some(binary_path.clone()),
                    });
                    None
                }
                Err(error) => {
                    diagnostics.push(RuntimeDiagnostic {
                        kind: RuntimeDiagnosticKind::VersionUnparseable,
                        message: error.to_string(),
                        path: Some(binary_path.clone()),
                    });
                    None
                }
            }
        } else {
            None
        };

        let controller_reachable = options.controller_addr.as_ref().map(|addr| {
            let reachable = controller_reachable(addr, options.connect_timeout);
            if !reachable {
                diagnostics.push(RuntimeDiagnostic {
                    kind: RuntimeDiagnosticKind::ControllerUnavailable,
                    message: format!("external-controller 不可达: {addr}"),
                    path: None,
                });
            }
            reachable
        });

        Ok(MihomoRuntimeInfo {
            binary_path: Some(binary_path),
            version,
            executable,
            controller_reachable,
            diagnostics,
        })
    }
}

pub fn find_mihomo_candidates(options: &RuntimeDetectionOptions) -> Vec<PathBuf> {
    let mut candidates = BTreeSet::new();
    if let Some(dir) = &options.user_config_dir {
        push_candidate_names(&mut candidates, dir);
    }
    push_candidate_names(&mut candidates, &options.managed_core_dir);
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            push_candidate_names(&mut candidates, &dir);
        }
    }
    candidates
        .into_iter()
        .filter(|path| path.is_file())
        .collect()
}

pub fn parse_mihomo_version(output: &str) -> Option<String> {
    output
        .split(|ch: char| ch.is_whitespace() || ch == '(' || ch == ')')
        .find_map(|part| {
            let normalized = part.trim_start_matches('v');
            if looks_like_semver(normalized) {
                Some(normalized.to_owned())
            } else {
                None
            }
        })
}

async fn read_mihomo_version(path: &Path, timeout: Duration) -> AppResult<Option<String>> {
    let mut command = Command::new(path);
    command
        .arg("-v")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    air_platform::process::hide_tokio_subprocess_window(&mut command);
    let child = command.spawn().map_err(ProcessError::Io)?;
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| ProcessError::Timeout("读取 mihomo 版本超时".into()))?
        .map_err(ProcessError::Io)?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(parse_mihomo_version(&text))
}

fn controller_reachable(addr: &str, timeout: Duration) -> bool {
    let target = addr
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    target
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .and_then(|addr| TcpStream::connect_timeout(&addr, timeout).ok())
        .is_some()
}

fn push_candidate_names(candidates: &mut BTreeSet<PathBuf>, dir: &Path) {
    // Windows 通常带 .exe；Unix 可执行文件没有扩展名，调用方只处理语义候选路径。
    candidates.insert(dir.join(executable_file_name()));
    candidates.insert(dir.join("mihomo"));
}

fn executable_file_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "mihomo.exe"
    } else {
        "mihomo"
    }
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    platform_is_executable(path)
}

#[cfg(unix)]
fn platform_is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn platform_is_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
}

#[cfg(not(any(unix, windows)))]
fn platform_is_executable(path: &Path) -> bool {
    path.is_file()
}

fn looks_like_semver(part: &str) -> bool {
    let mut segments = part.split('.');
    matches!(
        (segments.next(), segments.next(), segments.next()),
        (Some(major), Some(minor), Some(patch))
            if major.chars().all(|ch| ch.is_ascii_digit())
                && minor.chars().all(|ch| ch.is_ascii_digit())
                && patch
                    .chars()
                    .take_while(|ch| ch.is_ascii_digit())
                    .count()
                    > 0
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mihomo_version_from_common_outputs() {
        assert_eq!(
            parse_mihomo_version(
                "Mihomo Meta alpha-123-gabc linux amd64 with go1.22, version v1.19.3"
            ),
            Some("1.19.3".into())
        );
        assert_eq!(
            parse_mihomo_version("mihomo version 1.18.8 premium"),
            Some("1.18.8".into())
        );
    }

    #[test]
    fn reports_unparseable_version() {
        assert_eq!(parse_mihomo_version("mihomo custom build"), None);
    }

    #[test]
    fn builds_platform_candidate_names() {
        let temp = tempfile::tempdir().unwrap();
        let options = RuntimeDetectionOptions {
            user_config_dir: Some(temp.path().join("config")),
            managed_core_dir: temp.path().join("cores"),
            controller_addr: None,
            version_timeout: Duration::from_secs(1),
            connect_timeout: Duration::from_secs(1),
        };
        let mut candidates = BTreeSet::new();
        push_candidate_names(&mut candidates, &options.managed_core_dir);

        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.ends_with(executable_file_name()))
        );
    }
}
