extern crate self as air_mihomo;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use air_error::AppResult;

pub mod acquire;
pub mod client;
pub mod config_test;
pub mod detect;
pub mod dto;
pub mod embedded;
pub mod global_config;
pub mod groups;
pub mod process;
pub mod proxies;
pub mod release;
pub mod rules;
pub mod service;
pub mod streams;
pub mod subscriptions;

pub use acquire::{CoreAcquisitionService, CoreInstallEvent, CoreInstallMetadata};
pub use client::{MihomoHealthCheck, MihomoHttpClient};
pub use config_test::{
    MihomoConfigTestOptions, MihomoConfigTestPreview, build_mihomo_config_test_preview,
    test_mihomo_config,
};
pub use detect::{
    MihomoRuntimeDetector, MihomoRuntimeInfo, RuntimeDetectionOptions, RuntimeDetector,
    RuntimeDiagnostic, RuntimeDiagnosticKind,
};
pub use dto::{ConnectionsResponse, ProxiesResponse, RulesResponse, VersionResponse};
pub use process::{MihomoProcessManager, ProcessControl, ProcessEvent, ProcessLaunchConfig};
pub use release::{CoreAsset, CoreRelease, CoreReleaseProvider, HostPlatform};
pub use service::{MihomoService, MihomoServiceStatus};
pub use streams::{MihomoStreamClient, StreamEvent, StreamKind};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CoreBinary {
    pub path: PathBuf,
    pub version: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CoreLaunchOptions {
    pub binary: CoreBinary,
    pub config_path: PathBuf,
    pub working_dir: PathBuf,
    pub external_controller: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CoreProcessStatus {
    Stopped,
    Starting,
    Running { pid: u32 },
    Stopping,
    Failed { message: String },
}

#[async_trait]
pub trait CoreRuntime: Send + Sync {
    // 进程管理是异步边界：调用方只能观察状态和请求动作，不能直接持有子进程句柄。
    async fn detect(&self, search_dir: &Path) -> AppResult<Option<CoreBinary>>;
    async fn start(&self, options: CoreLaunchOptions) -> AppResult<CoreProcessStatus>;
    async fn stop(&self) -> AppResult<CoreProcessStatus>;
    async fn restart(&self, options: CoreLaunchOptions) -> AppResult<CoreProcessStatus>;
    async fn status(&self) -> AppResult<CoreProcessStatus>;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MihomoEndpoint {
    pub base_url: String,
    pub secret: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub version: Option<String>,
    pub uptime_seconds: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProxySummary {
    pub name: String,
    pub kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MihomoEvent {
    Log { level: String, message: String },
    Traffic { upload: u64, download: u64 },
}

#[async_trait]
pub trait MihomoApiClient: Send + Sync {
    // API trait 保持传输无关，后续 HTTP/WS 实现可以替换为 mock 或 reqwest 客户端。
    async fn runtime_info(&self, endpoint: &MihomoEndpoint) -> AppResult<RuntimeInfo>;
    async fn proxies(&self, endpoint: &MihomoEndpoint) -> AppResult<Vec<ProxySummary>>;
    async fn select_proxy(
        &self,
        endpoint: &MihomoEndpoint,
        group: &str,
        proxy: &str,
    ) -> AppResult<()>;
}
