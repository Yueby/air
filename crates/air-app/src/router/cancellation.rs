use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use air_app::command::AppCommand;
use air_app::runtime::CancellationToken;

#[derive(Clone, Default)]
pub(super) struct CommandCancellationRegistry {
    tokens: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl CommandCancellationRegistry {
    pub(super) fn insert(&self, key: String, token: CancellationToken) {
        if let Some(previous) = self
            .tokens
            .lock()
            .expect("command cancellation lock should not be poisoned")
            .insert(key, token)
        {
            previous.cancel();
        }
    }

    pub(super) fn cancel(&self, key: &str) -> Option<CancellationToken> {
        let token = self
            .tokens
            .lock()
            .expect("command cancellation lock should not be poisoned")
            .remove(key)?;
        token.cancel();
        Some(token)
    }

    pub(super) fn remove(&self, key: &str) {
        self.tokens
            .lock()
            .expect("command cancellation lock should not be poisoned")
            .remove(key);
    }

    pub(super) fn remove_if_same(&self, key: &str, token: &CancellationToken) {
        let mut tokens = self
            .tokens
            .lock()
            .expect("command cancellation lock should not be poisoned");
        // 后台任务可能被相同 key 的新任务替换；这里只清理本任务注册的令牌，
        // 防止旧任务晚结束时把新任务的取消入口从注册表里删掉。
        if tokens
            .get(key)
            .is_some_and(|registered| registered.ptr_eq(token))
        {
            tokens.remove(key);
        }
    }

    #[cfg(test)]
    pub(super) fn contains(&self, key: &str) -> bool {
        self.tokens
            .lock()
            .expect("command cancellation lock should not be poisoned")
            .contains_key(key)
    }
}

pub(super) fn long_task_key(command: &AppCommand) -> Option<String> {
    match command {
        AppCommand::UpdateSubscription { subscription_id } => {
            Some(format!("subscription:{subscription_id}"))
        }
        AppCommand::RefreshDueSubscriptions => Some("subscriptions-scheduler".into()),
        AppCommand::StartLogMonitoring | AppCommand::StopLogMonitoring => {
            Some("log-monitoring".into())
        }
        AppCommand::StartTrafficMonitoring | AppCommand::StopTrafficMonitoring => {
            Some("traffic-monitoring".into())
        }
        AppCommand::StartConnectionsMonitoring | AppCommand::StopConnectionsMonitoring => {
            Some("connections-monitoring".into())
        }
        AppCommand::StartCore | AppCommand::StopCore | AppCommand::RestartCore => {
            Some("core".into())
        }
        AppCommand::InstallCoreService | AppCommand::UninstallCoreService => {
            Some("core-service".into())
        }
        _ => None,
    }
}
