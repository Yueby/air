use std::sync::Arc;

use air_app::runtime::CancellationToken;
use air_app::services::AppServices;

use super::cancellation::CommandCancellationRegistry;

#[derive(Clone)]
pub(super) struct CommandExecutionContext {
    pub(super) services: Arc<AppServices>,
    pub(super) cancellations: CommandCancellationRegistry,
    pub(super) token: CancellationToken,
}

impl CommandExecutionContext {
    pub(super) fn new(
        services: Arc<AppServices>,
        cancellations: CommandCancellationRegistry,
        token: CancellationToken,
    ) -> Self {
        // 命令 handler 共享同一份服务集合和取消令牌；具体领域模块只编排业务，不重新持有 UI 状态。
        Self {
            services,
            cancellations,
            token,
        }
    }
}
