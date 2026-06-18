use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use air_mihomo::MihomoRuntimeInfo;
use air_mihomo::groups::ProxyGroupRuntimeProjection;
use air_mihomo::streams::StreamEvent;
use air_mihomo::{ConnectionsResponse, RulesResponse};
use air_platform::core_service::CoreServiceSnapshot;

use super::command::CommandId;
use super::subscription_controller::SubscriptionStateProjection;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub runtime: RuntimeStatus,
    pub active_profile: Option<String>,
    // 运行时快照来自 MihomoService 和命令状态；UI 只读展示，不在回调里直接修改。
    #[serde(default)]
    pub runtime_info: Option<MihomoRuntimeInfo>,
    // controller 地址由当前 profile 或运行时装配注入，避免 UI 硬编码 mihomo API 入口。
    #[serde(default)]
    pub controller_addr: Option<String>,
    // 当前快照不再缓存旧的本地配置校验摘要，只保留核心服务等顶层运行态。
    #[serde(default)]
    pub core_service: CoreServiceSnapshot,
    // 最近错误用于首页和状态栏展示；写入前必须完成敏感信息脱敏。
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum RuntimeStatus {
    #[default]
    Idle,
    Starting,
    Running,
    Stopping,
    Failed {
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AppEvent {
    SnapshotChanged(AppSnapshot),
    // mihomo 日志、流量、内存和连接流统一由 app 层转发给 UI；真实订阅和重连策略仍由后台服务负责。
    MihomoStreamEvent(StreamEvent),
    OverridePreviewGenerated {
        contents: String,
    },
    // /connections 的一次性 HTTP 刷新结果独立成事件，避免命令路由拿到响应后被丢弃。
    ConnectionsStateChanged(ConnectionsResponse),
    // `/rules` 返回的是 mihomo 当前运行态规则链；禁用状态同样只属于内核运行期，
    // UI 接到事件后刷新列表，不把该状态写回用户 YAML。
    RulesStateChanged(RulesResponse),
    // 代理组配置来自本地 YAML；运行态选择和 API 展开的成员由命令路由刷新后回填。
    ProxyGroupStateChanged(ProxyGroupRuntimeProjection),
    // 测速结果只携带被测目标和延迟值，页面按自己的成员索引映射到可见卡片。
    ProxyDelayMeasured {
        name: String,
        delay_ms: u64,
    },
    ProxyGroupDelayMeasured {
        name: String,
        member_delays: BTreeMap<String, u64>,
    },
    SubscriptionStateChanged(SubscriptionStateProjection),
    SubscriptionYamlLoaded {
        subscription_id: String,
        contents: String,
    },
    SubscriptionUpdateCanceled {
        subscription_id: String,
    },
    CommandStarted {
        id: CommandId,
    },
    CommandFinished {
        id: CommandId,
    },
    RuntimeStatusChanged(RuntimeStatus),
    UserVisibleError {
        message: String,
    },
    UserNotification {
        level: AppNotificationLevel,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AppNotificationLevel {
    Info,
    Success,
    Warning,
}

impl AppEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SnapshotChanged(_) => "SnapshotChanged",
            Self::MihomoStreamEvent(_) => "MihomoStreamEvent",
            Self::OverridePreviewGenerated { .. } => "OverridePreviewGenerated",
            Self::ConnectionsStateChanged(_) => "ConnectionsStateChanged",
            Self::RulesStateChanged(_) => "RulesStateChanged",
            Self::ProxyGroupStateChanged(_) => "ProxyGroupStateChanged",
            Self::ProxyDelayMeasured { .. } => "ProxyDelayMeasured",
            Self::ProxyGroupDelayMeasured { .. } => "ProxyGroupDelayMeasured",
            Self::SubscriptionStateChanged(_) => "SubscriptionStateChanged",
            Self::SubscriptionYamlLoaded { .. } => "SubscriptionYamlLoaded",
            Self::SubscriptionUpdateCanceled { .. } => "SubscriptionUpdateCanceled",
            Self::CommandStarted { .. } => "CommandStarted",
            Self::CommandFinished { .. } => "CommandFinished",
            Self::RuntimeStatusChanged(_) => "RuntimeStatusChanged",
            Self::UserVisibleError { .. } => "UserVisibleError",
            Self::UserNotification { .. } => "UserNotification",
        }
    }

    pub fn log_payload(&self) -> String {
        // 事件日志记录后台推送给 UI 的完整事件体，便于对齐 UI reducer 的输入。
        // 如果后续新增不可序列化字段，降级 Debug 文本也不能阻断事件广播。
        serde_json::to_string(self).unwrap_or_else(|_| format!("{self:?}"))
    }
}
