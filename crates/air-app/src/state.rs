use std::sync::{Arc, RwLock};

use air_app::events::{AppEvent, AppSnapshot, RuntimeStatus};
use air_app::runtime::AppRuntime;
use air_mihomo::MihomoRuntimeInfo;
use air_platform::core_service::CoreServiceSnapshot;
use air_telemetry::redaction::redact_log_value;

#[derive(Clone)]
pub struct AppStateStore {
    // AppSnapshot 是面向 UI 的只读投影；这里是 app 层唯一写入口，避免服务和页面各自维护影子状态。
    inner: Arc<RwLock<AppSnapshot>>,
    runtime: Arc<AppRuntime>,
}

impl AppStateStore {
    pub fn new(runtime: Arc<AppRuntime>, initial: AppSnapshot) -> Self {
        Self {
            inner: Arc::new(RwLock::new(initial)),
            runtime,
        }
    }

    pub fn snapshot(&self) -> AppSnapshot {
        match self.inner.read() {
            Ok(snapshot) => snapshot.clone(),
            Err(poisoned) => {
                // 锁中毒不代表快照不可恢复，后台晚到事件也不能让 GUI 崩溃，因此降级使用已恢复的值。
                tracing::warn!("app snapshot lock poisoned while reading; using recovered value");
                poisoned.into_inner().clone()
            }
        }
    }

    pub fn replace_snapshot(&self, snapshot: AppSnapshot) -> bool {
        self.update_snapshot(|current| *current = snapshot)
    }

    pub fn set_runtime_status(&self, status: RuntimeStatus) -> bool {
        let (changed, runtime_changed, snapshot) = self.update_snapshot_inner(|current| {
            let runtime_changed = current.runtime != status;
            current.runtime = status.clone();
            runtime_changed
        });
        if changed {
            if runtime_changed {
                self.runtime.emit(AppEvent::RuntimeStatusChanged(status));
            }
            self.runtime.emit(AppEvent::SnapshotChanged(snapshot));
        }
        changed
    }

    pub fn set_runtime_projection(
        &self,
        status: RuntimeStatus,
        runtime_info: Option<MihomoRuntimeInfo>,
        last_error: Option<String>,
    ) -> bool {
        let redacted_error = last_error.map(|error| redact_log_value(&error));
        let (changed, runtime_changed, snapshot) = self.update_snapshot_inner(|current| {
            let runtime_changed = current.runtime != status;
            current.runtime = status.clone();
            current.runtime_info = runtime_info;
            current.last_error = redacted_error;
            runtime_changed
        });
        if changed {
            if runtime_changed {
                self.runtime.emit(AppEvent::RuntimeStatusChanged(status));
            }
            self.runtime.emit(AppEvent::SnapshotChanged(snapshot));
        }
        changed
    }

    pub fn set_runtime_info(&self, runtime_info: Option<MihomoRuntimeInfo>) -> bool {
        self.update_snapshot(|current| current.runtime_info = runtime_info)
    }

    pub fn set_controller_addr(&self, controller_addr: Option<String>) -> bool {
        self.update_snapshot(|current| current.controller_addr = controller_addr)
    }

    pub fn set_active_profile(&self, active_profile: Option<String>) -> bool {
        self.update_snapshot(|current| current.active_profile = active_profile)
    }

    pub fn set_core_service(&self, service: CoreServiceSnapshot) -> bool {
        self.update_snapshot(|current| current.core_service = service)
    }

    pub fn set_last_error(&self, message: impl AsRef<str>) -> bool {
        let redacted = redact_log_value(message.as_ref());
        self.update_snapshot(|current| current.last_error = Some(redacted))
    }

    pub fn clear_last_error(&self) -> bool {
        self.update_snapshot(|current| current.last_error = None)
    }

    fn update_snapshot(&self, updater: impl FnOnce(&mut AppSnapshot)) -> bool {
        let (changed, (), snapshot) = self.update_snapshot_inner(|current| updater(current));
        if changed {
            // SnapshotChanged 只在内容实际变化时发送；高频流事件不写入 snapshot，避免 GPUI 过度刷新。
            self.runtime.emit(AppEvent::SnapshotChanged(snapshot));
        }
        changed
    }

    fn update_snapshot_inner<T>(
        &self,
        updater: impl FnOnce(&mut AppSnapshot) -> T,
    ) -> (bool, T, AppSnapshot) {
        let mut guard = match self.inner.write() {
            Ok(snapshot) => snapshot,
            Err(poisoned) => {
                // 继续写入恢复后的 guard，保证乱序命令完成或窗口关闭后的晚到事件不会因为锁状态 panic。
                tracing::warn!("app snapshot lock poisoned while writing; using recovered value");
                poisoned.into_inner()
            }
        };
        let previous = guard.clone();
        let value = updater(&mut guard);
        let changed = *guard != previous;
        let snapshot = guard.clone();
        (changed, value, snapshot)
    }
}

impl Default for AppStateStore {
    fn default() -> Self {
        let runtime = Arc::new(AppRuntime::new().expect("app runtime should initialize in tests"));
        Self::new(runtime, AppSnapshot::default())
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::broadcast::error::TryRecvError;

    use super::*;

    #[test]
    fn duplicate_snapshot_does_not_emit_change_event() {
        let runtime = Arc::new(AppRuntime::new().unwrap());
        let store = AppStateStore::new(Arc::clone(&runtime), AppSnapshot::default());
        let mut events = runtime.subscribe();

        assert!(!store.replace_snapshot(AppSnapshot::default()));

        assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn runtime_update_emits_runtime_and_snapshot_events_once() {
        let runtime = Arc::new(AppRuntime::new().unwrap());
        let store = AppStateStore::new(Arc::clone(&runtime), AppSnapshot::default());
        let mut events = runtime.subscribe();

        assert!(store.set_runtime_status(RuntimeStatus::Running));

        assert!(matches!(
            events.try_recv(),
            Ok(AppEvent::RuntimeStatusChanged(RuntimeStatus::Running))
        ));
        assert!(matches!(
            events.try_recv(),
            Ok(AppEvent::SnapshotChanged(snapshot)) if snapshot.runtime == RuntimeStatus::Running
        ));
        assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn user_visible_error_is_redacted_before_snapshot() {
        let runtime = Arc::new(AppRuntime::new().unwrap());
        let store = AppStateStore::new(runtime, AppSnapshot::default());

        store.set_last_error("fetch https://example.test/sub?token=abc secret=def");

        let error = store.snapshot().last_error.unwrap();
        assert!(error.contains("token=***"));
        assert!(error.contains("secret=***"));
        assert!(!error.contains("abc"));
        assert!(!error.contains("def"));
    }
}
