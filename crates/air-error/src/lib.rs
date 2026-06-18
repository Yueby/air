extern crate self as air_error;

use std::path::PathBuf;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("配置错误: {0}")]
    Config(#[from] ConfigError),
    #[error("mihomo 进程错误: {0}")]
    Process(#[from] ProcessError),
    #[error("mihomo API 错误: {0}")]
    Api(#[from] ApiError),
    #[error("存储错误: {0}")]
    Storage(#[from] StorageError),
    #[error("平台能力错误: {0}")]
    Platform(#[from] PlatformError),
    #[error("界面错误: {0}")]
    Gui(#[from] GuiError),
    #[error("后台任务错误: {0}")]
    Runtime(#[from] RuntimeError),
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("配置文档格式无效: {0}")]
    InvalidDocument(String),
    #[error("配置校验失败: {0}")]
    Validation(String),
    #[error("订阅源操作失败: {0}")]
    Subscription(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("核心路径不存在: {0}")]
    BinaryNotFound(PathBuf),
    #[error("核心进程状态不允许当前操作: {0}")]
    InvalidState(String),
    #[error("核心执行权限不足: {0}")]
    PermissionDenied(PathBuf),
    #[error("进程 IO 失败: {0}")]
    Io(#[from] std::io::Error),
    #[error("等待进程状态超时: {0}")]
    Timeout(String),
    #[error("核心安装失败: {0}")]
    Install(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("请求失败: {0}")]
    Request(String),
    #[error("HTTP 状态错误 {status}: {body}")]
    HttpStatus { status: u16, body: String },
    #[error("mihomo 返回业务错误: {0}")]
    Business(String),
    #[error("响应 JSON 解析失败: {0}")]
    Json(String),
    #[error("响应格式无效: {0}")]
    InvalidResponse(String),
    #[error("事件流已关闭")]
    StreamClosed,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("无法解析应用目录")]
    ProjectDirsUnavailable,
    #[error("路径越界，拒绝写入: {0}")]
    UnsafePath(PathBuf),
    #[error("文件系统错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON 序列化错误: {0}")]
    Json(#[from] serde_json::Error),
    #[error("YAML 序列化错误: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("TOML 序列化错误: {0}")]
    Toml(String),
}

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("当前平台不支持: {0}")]
    Unsupported(String),
    #[error("平台调用失败: {0}")]
    OperationFailed(String),
}

#[derive(Debug, thiserror::Error)]
pub enum GuiError {
    #[error("界面初始化失败: {0}")]
    Initialization(String),
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("后台运行时初始化失败: {0}")]
    Initialization(String),
    #[error("命令通道已关闭")]
    CommandChannelClosed,
    #[error("命令尚未接入: {0}")]
    CommandNotImplemented(String),
    #[error("命令不支持当前操作: {0}")]
    UnsupportedCommand(String),
    #[error("后台任务取消")]
    Canceled,
}
