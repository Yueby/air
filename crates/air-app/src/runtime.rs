use std::future::Future;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use tokio::runtime::{Builder, Runtime};
use tokio::sync::broadcast;

use air_app::command::{AppCommand, CommandId, CommandResult};
use air_app::events::AppEvent;
use air_error::{AppResult, RuntimeError};
use air_telemetry::redaction::redact_log_value;

pub struct AppRuntime {
    runtime: Runtime,
    events: broadcast::Sender<AppEvent>,
    next_command_id: AtomicU64,
}

impl AppRuntime {
    pub fn new() -> AppResult<Self> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .thread_name("air-bg")
            .build()
            .map_err(|error| RuntimeError::Initialization(error.to_string()))?;
        let (events, _) = broadcast::channel(128);
        Ok(Self {
            runtime,
            events,
            next_command_id: AtomicU64::new(1),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.events.subscribe()
    }

    pub fn emit(&self, event: AppEvent) {
        // 没有订阅者时不应让后台任务失败，GUI 页面稍后订阅即可收到新事件。
        let _ = self.events.send(event);
    }

    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        // GUI 退出回调本身不是 async，核心进程又必须在应用退出前收尾；
        // 因此只在明确的同步边界暴露 block_on，避免业务代码临时创建 Tokio runtime。
        self.runtime.block_on(future)
    }

    pub fn next_command_id(&self) -> CommandId {
        CommandId(self.next_command_id.fetch_add(1, Ordering::Relaxed))
    }

    pub fn spawn_command<F>(&self, id: CommandId, command: AppCommand, future: F) -> BackgroundTask
    where
        F: Future<Output = CommandResult> + Send + 'static,
    {
        let cancellation = CancellationToken::new();
        self.spawn_command_with_token(id, command, cancellation, future)
    }

    pub fn spawn_command_with_token<F>(
        &self,
        id: CommandId,
        command: AppCommand,
        cancellation: CancellationToken,
        future: F,
    ) -> BackgroundTask
    where
        F: Future<Output = CommandResult> + Send + 'static,
    {
        let events = self.events.clone();
        let kind = command.kind();
        let command_payload = command.log_payload();
        let handle = self.runtime.spawn(async move {
            tracing::info!(
                command_id = id.0,
                command_kind = kind,
                command_payload = %command_payload,
                "command task started"
            );
            let started_event = AppEvent::CommandStarted { id };
            tracing::info!(
                event_kind = started_event.kind(),
                event_payload = %started_event.log_payload(),
                "backend event emitted to ui"
            );
            let _ = events.send(started_event);
            let result = future.await;
            if let Err(error) = &result.outcome {
                tracing::warn!(
                    command_id = id.0,
                    command_kind = kind,
                    command_payload = %result.command.log_payload(),
                    error = %redact_log_value(&error.to_string()),
                    "command task failed"
                );
                let error_event = AppEvent::UserVisibleError {
                    message: redact_log_value(&error.to_string()),
                };
                tracing::info!(
                    event_kind = error_event.kind(),
                    event_payload = %error_event.log_payload(),
                    "backend event emitted to ui"
                );
                let _ = events.send(error_event);
            } else {
                tracing::info!(
                    command_id = id.0,
                    command_kind = kind,
                    command_payload = %result.command.log_payload(),
                    "command task succeeded"
                );
            }
            let finished_event = AppEvent::CommandFinished { id };
            tracing::info!(
                event_kind = finished_event.kind(),
                event_payload = %finished_event.log_payload(),
                "backend event emitted to ui"
            );
            let _ = events.send(finished_event);
            result
        });
        BackgroundTask {
            id,
            command,
            cancellation,
            handle,
        }
    }
}

pub struct BackgroundTask {
    pub id: CommandId,
    pub command: AppCommand,
    cancellation: CancellationToken,
    handle: tokio::task::JoinHandle<CommandResult>,
}

impl BackgroundTask {
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    pub async fn wait(self) -> AppResult<CommandResult> {
        self.handle
            .await
            .map_err(|error| RuntimeError::Initialization(error.to_string()).into())
    }
}

#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    canceled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::SeqCst);
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::SeqCst)
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        // 取消令牌会被克隆到路由、后台任务和执行分支中；按 Arc 身份比较可以区分
        // “同一个任务的多个句柄”和“相同 key 后来注册的新任务”，避免收尾时误删新任务。
        Arc::ptr_eq(&self.canceled, &other.canceled)
    }
}
