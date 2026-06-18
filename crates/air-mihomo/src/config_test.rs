use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use air_error::{AppResult, ConfigError, ProcessError};
use air_telemetry::redaction::redact_log_value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MihomoConfigTestOptions {
    pub binary_path: PathBuf,
    pub working_dir: PathBuf,
    pub safe_paths: Vec<PathBuf>,
    pub timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MihomoConfigTestPreview {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub current_dir: PathBuf,
}

impl MihomoConfigTestOptions {
    pub fn new(binary_path: PathBuf, working_dir: PathBuf, safe_paths: Vec<PathBuf>) -> Self {
        Self {
            binary_path,
            working_dir,
            safe_paths,
            timeout: Duration::from_secs(15),
        }
    }
}

pub fn build_mihomo_config_test_preview(
    options: &MihomoConfigTestOptions,
    config_path: PathBuf,
) -> MihomoConfigTestPreview {
    let mut env = BTreeMap::new();
    if !options.safe_paths.is_empty() {
        env.insert(
            "SAFE_PATHS".to_string(),
            join_safe_paths(&options.safe_paths),
        );
    }

    MihomoConfigTestPreview {
        program: options.binary_path.clone(),
        args: vec![
            "-d".to_string(),
            options.working_dir.to_string_lossy().to_string(),
            "-t".to_string(),
            "-f".to_string(),
            config_path.to_string_lossy().to_string(),
        ],
        env,
        current_dir: options.working_dir.clone(),
    }
}

pub async fn test_mihomo_config(options: MihomoConfigTestOptions, yaml: &str) -> AppResult<()> {
    std::fs::create_dir_all(&options.working_dir).map_err(ProcessError::Io)?;
    let temp_config = write_temp_config_for_test(&options, yaml)?;
    // 统一使用临时 YAML 文件校验，避免 Windows 命令行长度限制，也让小配置和大配置走同一条路径。
    let preview = build_mihomo_config_test_preview(&options, temp_config.path().to_path_buf());
    tracing::info!(
        binary = %preview.program.display(),
        working_dir = %preview.current_dir.display(),
        config_bytes = yaml.len(),
        "testing mihomo configuration before write"
    );

    let mut command = Command::new(&preview.program);
    command
        .args(&preview.args)
        .current_dir(&preview.current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    air_platform::process::hide_tokio_subprocess_window(&mut command);
    for (key, value) in &preview.env {
        command.env(key, value);
    }

    let child = command.spawn().map_err(ProcessError::Io)?;
    let output = tokio::time::timeout(options.timeout, child.wait_with_output())
        .await
        .map_err(|_| ProcessError::Timeout("mihomo 配置校验超时".into()))?
        .map_err(ProcessError::Io)?;

    if output.status.success() {
        tracing::info!("mihomo configuration test completed");
        return Ok(());
    }

    // mihomo 的校验输出可能包含订阅 URL、secret 或本地路径；进入错误和日志前统一脱敏。
    let message = summarize_mihomo_output(&output.stdout, &output.stderr);
    tracing::warn!(
        status = output.status.code(),
        message = %message,
        "mihomo configuration test failed"
    );
    Err(ConfigError::Validation(format!("mihomo -t 校验失败: {message}")).into())
}

fn write_temp_config_for_test(
    options: &MihomoConfigTestOptions,
    yaml: &str,
) -> AppResult<tempfile::NamedTempFile> {
    let mut file = tempfile::Builder::new()
        .prefix("air-config-test-")
        .suffix(".yaml")
        .tempfile_in(&options.working_dir)
        .map_err(ProcessError::Io)?;
    file.write_all(yaml.as_bytes()).map_err(ProcessError::Io)?;
    file.flush().map_err(ProcessError::Io)?;
    Ok(file)
}

fn summarize_mihomo_output(stdout: &[u8], stderr: &[u8]) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let combined = format!("{stdout}\n{stderr}");
    let redacted = redact_log_value(combined.trim());
    if redacted.is_empty() {
        return "mihomo 未返回诊断输出".to_string();
    }

    const MAX_LEN: usize = 4000;
    if redacted.len() <= MAX_LEN {
        redacted
    } else {
        format!("{}...", &redacted[..MAX_LEN])
    }
}

fn join_safe_paths(paths: &[PathBuf]) -> String {
    let separator = if cfg!(target_os = "windows") {
        ";"
    } else {
        ":"
    };
    paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(separator)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_test_preview_uses_temp_config_path() {
        let options = MihomoConfigTestOptions::new(
            PathBuf::from("mihomo"),
            PathBuf::from("core-work"),
            vec![PathBuf::from("config"), PathBuf::from("cache")],
        );

        let preview =
            build_mihomo_config_test_preview(&options, PathBuf::from("core-work/test.yaml"));

        assert_eq!(preview.args[0], "-d");
        assert_eq!(preview.args[2], "-t");
        assert_eq!(preview.args[3], "-f");
        assert!(preview.args[4].ends_with("test.yaml"));
        assert!(preview.env.contains_key("SAFE_PATHS"));
    }

    #[test]
    fn config_test_output_is_redacted() {
        let message = summarize_mihomo_output(
            b"",
            b"load https://example.test/sub.yaml?token=abc failed secret=def",
        );

        assert!(message.contains("token=***"));
        assert!(message.contains("secret=***"));
        assert!(!message.contains("abc"));
        assert!(!message.contains("def"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn nonzero_config_test_exit_becomes_validation_error() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("mihomo");
        std::fs::write(
            &binary,
            "#!/bin/sh\necho 'bad config secret=abc' >&2\nexit 1\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&binary, permissions).unwrap();

        let error = test_mihomo_config(
            MihomoConfigTestOptions::new(binary, temp.path().to_path_buf(), Vec::new()),
            "mixed-port: 7890\n",
        )
        .await
        .unwrap_err();

        assert!(matches!(error, air_error::AppError::Config(_)));
        assert!(error.to_string().contains("secret=***"));
        assert!(!error.to_string().contains("abc"));
    }
}
