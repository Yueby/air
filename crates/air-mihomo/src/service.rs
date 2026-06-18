use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use air_error::{AppResult, ProcessError};
use air_mihomo::client::MihomoHealthCheck;
use air_mihomo::detect::{
    MihomoRuntimeInfo, RuntimeDetectionOptions, RuntimeDetector, RuntimeDiagnosticKind,
};
use air_mihomo::process::{ProcessControl, ProcessLaunchConfig};
use air_mihomo::{CoreProcessStatus, RuntimeDiagnostic};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MihomoServicePhase {
    Idle,
    Preparing,
    Ready,
    Starting,
    Running,
    Stopping,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MihomoServiceStatus {
    pub phase: MihomoServicePhase,
    pub process: CoreProcessStatus,
    pub runtime: Option<MihomoRuntimeInfo>,
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub last_error: Option<String>,
}

impl Default for MihomoServiceStatus {
    fn default() -> Self {
        Self {
            phase: MihomoServicePhase::Idle,
            process: CoreProcessStatus::Stopped,
            runtime: None,
            diagnostics: Vec::new(),
            last_error: None,
        }
    }
}

pub struct MihomoService<D, P, H> {
    detector: Arc<D>,
    process: Arc<P>,
    health: Arc<H>,
    status: Arc<RwLock<MihomoServiceStatus>>,
    health_timeout: Duration,
    health_initial_delay: Duration,
}

impl<D, P, H> MihomoService<D, P, H>
where
    D: RuntimeDetector + 'static,
    P: ProcessControl + 'static,
    H: MihomoHealthCheck + 'static,
{
    pub fn new(detector: Arc<D>, process: Arc<P>, health: Arc<H>) -> Self {
        Self {
            detector,
            process,
            health,
            status: Arc::new(RwLock::new(MihomoServiceStatus::default())),
            health_timeout: Duration::from_secs(8),
            health_initial_delay: Duration::from_millis(350),
        }
    }

    pub async fn prepare(
        &self,
        options: RuntimeDetectionOptions,
    ) -> AppResult<MihomoServiceStatus> {
        tracing::info!("mihomo runtime prepare started");
        self.set_phase(MihomoServicePhase::Preparing).await;
        match self.detector.detect_runtime(options).await {
            Ok(runtime) => {
                let phase = if runtime.binary_path.is_some() && runtime.executable {
                    MihomoServicePhase::Ready
                } else {
                    MihomoServicePhase::Failed
                };
                let mut status = self.status.write().await;
                status.phase = phase;
                status.diagnostics = runtime.diagnostics.clone();
                status.runtime = Some(runtime);
                status.last_error = None;
                tracing::info!(phase = ?status.phase, "mihomo runtime prepare completed");
                Ok(status.clone())
            }
            Err(error) => {
                tracing::warn!(error = %error, "mihomo runtime prepare failed");
                self.fail(error.to_string()).await;
                Err(error)
            }
        }
    }

    pub async fn start(&self, config: ProcessLaunchConfig) -> AppResult<MihomoServiceStatus> {
        tracing::info!(
            binary = %config.binary_path.display(),
            config = %config.config_path.display(),
            "mihomo service start requested"
        );
        validate_launch_config(&config)?;
        self.set_phase(MihomoServicePhase::Starting).await;
        match self.process.start_process(config).await {
            Ok(process_status) => {
                {
                    let mut status = self.status.write().await;
                    status.process = process_status;
                }
                // 启动后等待 /version 可用；失败时保留进程状态和诊断，便于 UI 提供重试。
                if let Err(error) = self.wait_health().await {
                    tracing::warn!(error = %error, "mihomo health check failed after start");
                    let process_status = match self.process.stop_process().await {
                        Ok(status) => status,
                        Err(stop_error) => CoreProcessStatus::Failed {
                            message: stop_error.to_string(),
                        },
                    };
                    let mut status = self.status.write().await;
                    status.phase = MihomoServicePhase::Failed;
                    status.process = process_status;
                    status.last_error = Some(error.to_string());
                    status.diagnostics.push(RuntimeDiagnostic {
                        kind: RuntimeDiagnosticKind::ControllerUnavailable,
                        message: "mihomo 已启动但 external-controller 未在超时内可用".into(),
                        path: None,
                    });
                    return Err(error);
                }
                let mut status = self.status.write().await;
                status.phase = MihomoServicePhase::Running;
                status.last_error = None;
                mark_controller_available(&mut status);
                tracing::info!("mihomo service started and controller is available");
                Ok(status.clone())
            }
            Err(error) => {
                tracing::warn!(error = %error, "mihomo process start failed");
                self.fail(error.to_string()).await;
                Err(error)
            }
        }
    }

    pub async fn stop(&self) -> AppResult<MihomoServiceStatus> {
        tracing::info!("mihomo service stop requested");
        self.set_phase(MihomoServicePhase::Stopping).await;
        match self.process.stop_process().await {
            Ok(process_status) => {
                let mut status = self.status.write().await;
                status.phase = MihomoServicePhase::Ready;
                status.process = process_status;
                tracing::info!("mihomo service stopped");
                Ok(status.clone())
            }
            Err(error) => {
                tracing::warn!(error = %error, "mihomo service stop failed");
                self.fail(error.to_string()).await;
                Err(error)
            }
        }
    }

    pub async fn restart(&self, config: ProcessLaunchConfig) -> AppResult<MihomoServiceStatus> {
        // reload/restart 失败时不清空旧 runtime 快照，UI 仍能显示上一次可用状态和错误诊断。
        tracing::info!("mihomo service restart requested");
        let _ = self.stop().await?;
        self.start(config).await
    }

    pub async fn status(&self) -> AppResult<MihomoServiceStatus> {
        let mut status = self.status.write().await;
        status.process = self.process.process_status().await?;
        Ok(status.clone())
    }

    async fn wait_health(&self) -> AppResult<()> {
        if !self.health_initial_delay.is_zero() {
            // mihomo 进程创建成功不代表 external-controller 已经完成监听；
            // 首次探测前给核心一点初始化时间，避免把正常启动窗口误报成 controller 不可用。
            tokio::time::sleep(self.health_initial_delay).await;
        }
        let deadline = Instant::now() + self.health_timeout;
        let mut last_error = None;
        while Instant::now() < deadline {
            match self.health.health_version().await {
                Ok(_) => {
                    tracing::debug!("mihomo controller health check succeeded");
                    return Ok(());
                }
                Err(error) => {
                    tracing::debug!(error = %error, "mihomo controller health check not ready");
                    last_error = Some(error.to_string());
                    // 配置解析 fatal 等场景下 mihomo 会在 controller 可用前直接退出；
                    // 这里主动同步进程状态，避免 UI 等完整健康检查超时后才知道启动失败。
                    match self.process.process_status().await? {
                        CoreProcessStatus::Failed { message } => {
                            return Err(ProcessError::InvalidState(message).into());
                        }
                        CoreProcessStatus::Stopped => {
                            return Err(
                                ProcessError::InvalidState("mihomo 启动后已退出".into()).into()
                            );
                        }
                        CoreProcessStatus::Starting
                        | CoreProcessStatus::Running { .. }
                        | CoreProcessStatus::Stopping => {}
                    }
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
        Err(ProcessError::Timeout(
            last_error.unwrap_or_else(|| "external-controller 不可用".into()),
        )
        .into())
    }

    async fn set_phase(&self, phase: MihomoServicePhase) {
        self.status.write().await.phase = phase;
    }

    async fn fail(&self, message: String) {
        let mut status = self.status.write().await;
        status.phase = MihomoServicePhase::Failed;
        status.last_error = Some(message);
    }
}

fn validate_launch_config(config: &ProcessLaunchConfig) -> AppResult<()> {
    if !config.binary_path.is_file() {
        return Err(ProcessError::BinaryNotFound(config.binary_path.clone()).into());
    }
    if !config.config_path.is_file() {
        return Err(ProcessError::InvalidState(format!(
            "配置文件不存在或无效: {}",
            config.config_path.display()
        ))
        .into());
    }
    Ok(())
}

fn mark_controller_available(status: &mut MihomoServiceStatus) {
    remove_controller_unavailable(&mut status.diagnostics);
    if let Some(runtime) = status.runtime.as_mut() {
        runtime.controller_reachable = Some(true);
        remove_controller_unavailable(&mut runtime.diagnostics);
    }
}

fn remove_controller_unavailable(diagnostics: &mut Vec<RuntimeDiagnostic>) {
    diagnostics
        .retain(|diagnostic| diagnostic.kind != RuntimeDiagnosticKind::ControllerUnavailable);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use air_error::ApiError;
    use async_trait::async_trait;

    use super::*;

    struct MockDetector;

    #[async_trait]
    impl RuntimeDetector for MockDetector {
        async fn detect_runtime(
            &self,
            _options: RuntimeDetectionOptions,
        ) -> AppResult<MihomoRuntimeInfo> {
            Ok(MihomoRuntimeInfo {
                binary_path: Some(PathBuf::from("mihomo")),
                version: Some("1.19.0".into()),
                executable: true,
                controller_reachable: Some(true),
                diagnostics: Vec::new(),
            })
        }
    }

    struct MockProcess;

    #[async_trait]
    impl ProcessControl for MockProcess {
        async fn start_process(
            &self,
            _config: ProcessLaunchConfig,
        ) -> AppResult<CoreProcessStatus> {
            Ok(CoreProcessStatus::Running { pid: 42 })
        }

        async fn stop_process(&self) -> AppResult<CoreProcessStatus> {
            Ok(CoreProcessStatus::Stopped)
        }

        async fn restart_process(
            &self,
            _config: ProcessLaunchConfig,
        ) -> AppResult<CoreProcessStatus> {
            Ok(CoreProcessStatus::Running { pid: 43 })
        }

        async fn process_status(&self) -> AppResult<CoreProcessStatus> {
            Ok(CoreProcessStatus::Stopped)
        }
    }

    struct MockHealth {
        failures_before_success: AtomicUsize,
    }

    #[async_trait]
    impl MihomoHealthCheck for MockHealth {
        async fn health_version(&self) -> AppResult<String> {
            let left = self.failures_before_success.load(Ordering::SeqCst);
            if left == 0 {
                Ok("1.19.0".into())
            } else {
                self.failures_before_success.fetch_sub(1, Ordering::SeqCst);
                Err(ApiError::Request("not ready".into()).into())
            }
        }
    }

    #[tokio::test]
    async fn prepare_exposes_stable_snapshot() {
        let service = MihomoService::new(
            Arc::new(MockDetector),
            Arc::new(MockProcess),
            Arc::new(MockHealth {
                failures_before_success: AtomicUsize::new(0),
            }),
        );
        let temp = tempfile::tempdir().unwrap();

        let status = service
            .prepare(RuntimeDetectionOptions {
                user_config_dir: None,
                managed_core_dir: temp.path().to_path_buf(),
                controller_addr: None,
                version_timeout: Duration::from_secs(1),
                connect_timeout: Duration::from_secs(1),
            })
            .await
            .unwrap();

        assert_eq!(status.phase, MihomoServicePhase::Ready);
    }

    #[tokio::test]
    async fn start_reports_invalid_config_before_spawning() {
        let service = MihomoService::new(
            Arc::new(MockDetector),
            Arc::new(MockProcess),
            Arc::new(MockHealth {
                failures_before_success: AtomicUsize::new(0),
            }),
        );
        let config = ProcessLaunchConfig {
            binary_path: PathBuf::from("missing"),
            config_path: PathBuf::from("missing.yaml"),
            working_dir: PathBuf::from("."),
            console_log_path: None,
            env: BTreeMap::new(),
            safe_paths: Vec::new(),
            requires_admin: false,
        };

        let error = service.start(config).await.unwrap_err();

        assert!(matches!(error, air_error::AppError::Process(_)));
    }

    struct StartThenStopProcess {
        stop_called: AtomicBool,
    }

    #[async_trait]
    impl ProcessControl for StartThenStopProcess {
        async fn start_process(
            &self,
            _config: ProcessLaunchConfig,
        ) -> AppResult<CoreProcessStatus> {
            Ok(CoreProcessStatus::Running { pid: 42 })
        }

        async fn stop_process(&self) -> AppResult<CoreProcessStatus> {
            self.stop_called.store(true, Ordering::SeqCst);
            Ok(CoreProcessStatus::Stopped)
        }

        async fn restart_process(
            &self,
            _config: ProcessLaunchConfig,
        ) -> AppResult<CoreProcessStatus> {
            Ok(CoreProcessStatus::Running { pid: 43 })
        }

        async fn process_status(&self) -> AppResult<CoreProcessStatus> {
            Ok(CoreProcessStatus::Stopped)
        }
    }

    #[tokio::test]
    async fn start_health_failure_stops_process_and_exposes_failed_status() {
        let process = Arc::new(StartThenStopProcess {
            stop_called: AtomicBool::new(false),
        });
        let mut service = MihomoService::new(
            Arc::new(MockDetector),
            Arc::clone(&process),
            Arc::new(MockHealth {
                failures_before_success: AtomicUsize::new(usize::MAX),
            }),
        );
        service.health_timeout = Duration::from_millis(1);
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("mihomo");
        let config_path = temp.path().join("config.yaml");
        std::fs::write(&binary, b"fake").unwrap();
        std::fs::write(&config_path, b"mixed-port: 7890\n").unwrap();
        let config = ProcessLaunchConfig {
            binary_path: binary,
            config_path,
            working_dir: temp.path().to_path_buf(),
            console_log_path: None,
            env: BTreeMap::new(),
            safe_paths: Vec::new(),
            requires_admin: false,
        };

        let error = service.start(config).await.unwrap_err();
        let status = service.status().await.unwrap();

        assert!(matches!(error, air_error::AppError::Process(_)));
        assert!(process.stop_called.load(Ordering::SeqCst));
        assert_eq!(status.phase, MihomoServicePhase::Failed);
        assert_eq!(status.process, CoreProcessStatus::Stopped);
    }

    #[tokio::test]
    async fn successful_start_clears_stale_controller_diagnostics() {
        let mut service = MihomoService::new(
            Arc::new(MockDetector),
            Arc::new(MockProcess),
            Arc::new(MockHealth {
                failures_before_success: AtomicUsize::new(0),
            }),
        );
        service.health_initial_delay = Duration::ZERO;
        {
            let mut status = service.status.write().await;
            status.runtime = Some(MihomoRuntimeInfo {
                binary_path: Some(PathBuf::from("mihomo")),
                version: Some("1.19.0".into()),
                executable: true,
                controller_reachable: Some(false),
                diagnostics: vec![RuntimeDiagnostic {
                    kind: RuntimeDiagnosticKind::ControllerUnavailable,
                    message: "external-controller 不可达".into(),
                    path: None,
                }],
            });
            status.diagnostics = vec![RuntimeDiagnostic {
                kind: RuntimeDiagnosticKind::ControllerUnavailable,
                message: "external-controller 不可达".into(),
                path: None,
            }];
        }
        let temp = tempfile::tempdir().unwrap();
        let binary = temp.path().join("mihomo");
        let config_path = temp.path().join("config.yaml");
        std::fs::write(&binary, b"fake").unwrap();
        std::fs::write(&config_path, b"mixed-port: 7890\n").unwrap();

        let status = service
            .start(ProcessLaunchConfig {
                binary_path: binary,
                config_path,
                working_dir: temp.path().to_path_buf(),
                console_log_path: None,
                env: BTreeMap::new(),
                safe_paths: Vec::new(),
                requires_admin: false,
            })
            .await
            .unwrap();

        let runtime = status.runtime.expect("runtime should remain available");
        assert_eq!(runtime.controller_reachable, Some(true));
        assert!(runtime.diagnostics.is_empty());
        assert!(status.diagnostics.is_empty());
        assert_eq!(status.last_error, None);
    }
}
