use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use air_error::AppError;
use air_mihomo::subscriptions::SubscriptionSource;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct CommandId(pub u64);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AppCommand {
    // 只刷新本地或系统中的 mihomo 核心发现结果，不安装也不启动进程。
    DetectCore,
    // 执行运行前准备，例如核心检测、托管核心安装和基础诊断收集。
    PrepareCore,
    StartCore,
    StopCore,
    RestartCore,
    RefreshCoreService,
    InstallCoreService,
    UninstallCoreService,
    // 日志页只表达“开始读取 core.log”的意图；真实文件轮询由 app runtime 持有。
    StartLogMonitoring,
    // 离开日志页、隐藏到托盘或退出时停止日志轮询，避免不可见页面继续消费事件。
    StopLogMonitoring,
    // 连接页使用 mihomo `/connections` 的 WebSocket 推送；UI 只表达页面可见性。
    StartConnectionsMonitoring,
    StopConnectionsMonitoring,
    // 状态栏需要跨页面显示实时网速；`/traffic` 订阅只跟随核心运行状态。
    StartTrafficMonitoring,
    StopTrafficMonitoring,
    SetRuntimeMode {
        mode: String,
    },
    SaveConfig {
        profile: String,
    },
    SetOverrideScriptEnabled {
        enabled: bool,
    },
    SaveOverrideScript {
        script: String,
        enabled: bool,
    },
    DebugOverrideScript {
        script: String,
    },
    RefreshProxies,
    RefreshRules,
    SelectProxy {
        group: String,
        proxy: String,
    },
    TestProxyDelay {
        name: String,
        url: String,
        timeout_ms: u64,
    },
    TestProxyGroupDelay {
        name: String,
        url: String,
        timeout_ms: u64,
    },
    ClearProxyGroupFixed {
        name: String,
    },
    DisableRule {
        index: usize,
        disabled: bool,
    },
    UpdateRuleProvider {
        name: String,
    },
    UpdateSubscription {
        subscription_id: String,
    },
    // 后台定时器只表达“检查到期订阅”的意图；是否到期由 app/domain 根据持久化元数据判断。
    RefreshDueSubscriptions,
    // 订阅元数据保存必须穿过 app/storage 边界，避免 UI 表单直接写索引文件。
    SaveSubscriptionSource {
        source: SubscriptionSource,
    },
    // 编辑弹窗展示订阅缓存原文；UI 只显示只读内容，不反序列化再写回。
    LoadSubscriptionYaml {
        subscription_id: String,
    },
    ReorderSubscriptions {
        ordered_ids: Vec<String>,
    },
    // 选中订阅会落到仓储的 enabled 状态；运行配置合并只读取 enabled 缓存。
    SelectSubscription {
        subscription_id: String,
    },
    // URL 导入由 UI 表达意图，下载、校验和持久化在订阅服务/仓储中完成。
    ImportSubscriptionUrl {
        subscription_id: String,
        url: String,
    },
    // 原生文件选择只提供路径；实际读取、备份和缓存写入仍由 app/service 层接管。
    ImportSubscriptionFile {
        path: PathBuf,
    },
    DeleteSubscription {
        subscription_id: String,
    },
    CancelSubscriptionUpdate {
        subscription_id: String,
    },
    LoadProfile {
        path: PathBuf,
    },
    RefreshConnections,
    CloseConnection {
        id: String,
    },
    CloseAllConnections,
}

#[derive(Debug)]
pub struct CommandResult {
    pub id: CommandId,
    pub command: AppCommand,
    pub outcome: Result<(), AppError>,
}

impl CommandResult {
    // UI 只需要订阅统一结果，不需要知道命令由哪个服务执行。
    pub fn ok(id: CommandId, command: AppCommand) -> Self {
        Self {
            id,
            command,
            outcome: Ok(()),
        }
    }

    pub fn failed(id: CommandId, command: AppCommand, error: AppError) -> Self {
        Self {
            id,
            command,
            outcome: Err(error),
        }
    }
}

impl AppCommand {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::DetectCore => "DetectCore",
            Self::PrepareCore => "PrepareCore",
            Self::StartCore => "StartCore",
            Self::StopCore => "StopCore",
            Self::RestartCore => "RestartCore",
            Self::RefreshCoreService => "RefreshCoreService",
            Self::InstallCoreService => "InstallCoreService",
            Self::UninstallCoreService => "UninstallCoreService",
            Self::StartLogMonitoring => "StartLogMonitoring",
            Self::StopLogMonitoring => "StopLogMonitoring",
            Self::StartConnectionsMonitoring => "StartConnectionsMonitoring",
            Self::StopConnectionsMonitoring => "StopConnectionsMonitoring",
            Self::StartTrafficMonitoring => "StartTrafficMonitoring",
            Self::StopTrafficMonitoring => "StopTrafficMonitoring",
            Self::SetRuntimeMode { .. } => "SetRuntimeMode",
            Self::SaveConfig { .. } => "SaveConfig",
            Self::SetOverrideScriptEnabled { .. } => "SetOverrideScriptEnabled",
            Self::SaveOverrideScript { .. } => "SaveOverrideScript",
            Self::DebugOverrideScript { .. } => "DebugOverrideScript",
            Self::RefreshProxies => "RefreshProxies",
            Self::RefreshRules => "RefreshRules",
            Self::SelectProxy { .. } => "SelectProxy",
            Self::TestProxyDelay { .. } => "TestProxyDelay",
            Self::TestProxyGroupDelay { .. } => "TestProxyGroupDelay",
            Self::ClearProxyGroupFixed { .. } => "ClearProxyGroupFixed",
            Self::DisableRule { .. } => "DisableRule",
            Self::UpdateRuleProvider { .. } => "UpdateRuleProvider",
            Self::UpdateSubscription { .. } => "UpdateSubscription",
            Self::RefreshDueSubscriptions => "RefreshDueSubscriptions",
            Self::SaveSubscriptionSource { .. } => "SaveSubscriptionSource",
            Self::LoadSubscriptionYaml { .. } => "LoadSubscriptionYaml",
            Self::ReorderSubscriptions { .. } => "ReorderSubscriptions",
            Self::SelectSubscription { .. } => "SelectSubscription",
            Self::ImportSubscriptionUrl { .. } => "ImportSubscriptionUrl",
            Self::ImportSubscriptionFile { .. } => "ImportSubscriptionFile",
            Self::DeleteSubscription { .. } => "DeleteSubscription",
            Self::CancelSubscriptionUpdate { .. } => "CancelSubscriptionUpdate",
            Self::LoadProfile { .. } => "LoadProfile",
            Self::RefreshConnections => "RefreshConnections",
            Self::CloseConnection { .. } => "CloseConnection",
            Self::CloseAllConnections => "CloseAllConnections",
        }
    }

    pub fn log_payload(&self) -> String {
        // 命令日志必须保留 UI 提交给后台的完整参数，方便复盘跨层交互。
        // 序列化失败本身不应影响业务命令执行，因此这里只降级为 Debug 文本。
        serde_json::to_string(self).unwrap_or_else(|_| format!("{self:?}"))
    }
}
