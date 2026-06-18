use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use air_app::services::AppServices;
use air_error::AppResult;
use air_mihomo::streams::{StreamEvent, StreamKind, StreamOptions};
use air_telemetry::redaction::redact_log_value;

use super::context::CommandExecutionContext;
use super::shared::{runtime_api_available, wait_for_cancellation};

pub(super) async fn handle_start_log_monitoring(
    context: &CommandExecutionContext,
) -> AppResult<()> {
    run_log_monitoring(Arc::clone(&context.services), context.token.clone()).await?;
    context.cancellations.remove("log-monitoring");
    Ok(())
}

pub(super) async fn handle_stop_log_monitoring(context: &CommandExecutionContext) -> AppResult<()> {
    context.cancellations.cancel("log-monitoring");
    Ok(())
}

pub(super) async fn handle_start_traffic_monitoring(
    context: &CommandExecutionContext,
) -> AppResult<()> {
    run_traffic_monitoring(Arc::clone(&context.services), context.token.clone()).await?;
    context.cancellations.remove("traffic-monitoring");
    Ok(())
}

pub(super) async fn handle_stop_traffic_monitoring(
    context: &CommandExecutionContext,
) -> AppResult<()> {
    context.cancellations.cancel("traffic-monitoring");
    Ok(())
}

pub(super) async fn handle_start_connections_monitoring(
    context: &CommandExecutionContext,
) -> AppResult<()> {
    run_connections_monitoring(Arc::clone(&context.services), context.token.clone()).await?;
    context.cancellations.remove("connections-monitoring");
    Ok(())
}

pub(super) async fn handle_stop_connections_monitoring(
    context: &CommandExecutionContext,
) -> AppResult<()> {
    context.cancellations.cancel("connections-monitoring");
    Ok(())
}

async fn run_log_monitoring(
    services: Arc<AppServices>,
    token: air_app::runtime::CancellationToken,
) -> AppResult<()> {
    // 日志页只表达开始/停止读取日志文件的意图；实际文件轮询和 AppEvent 回填集中在 app 层，
    // 避免页面组件直接持有本地路径和异步轮询任务。
    let mut core_log_tail = CoreLogTail::from_start(services.paths.logs_dir.join("core.log"));
    let mut core_log_tick = tokio::time::interval(Duration::from_secs(1));
    core_log_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = wait_for_cancellation(token.clone()) => break,
            _ = core_log_tick.tick() => {
                for event in core_log_tail.poll_new_events().await {
                    services.runtime.emit(air_app::AppEvent::MihomoStreamEvent(event));
                }
            }
        }
    }

    Ok(())
}

async fn run_traffic_monitoring(
    services: Arc<AppServices>,
    token: air_app::runtime::CancellationToken,
) -> AppResult<()> {
    if !runtime_api_available(&services) {
        return Ok(());
    }

    // 右下角网速是全局状态，不属于某个页面；这里单独维护一条 `/traffic`
    // WebSocket，供状态栏和需要流量点的页面共享，避免重复订阅和切页断流。
    let client = services.mihomo_clients.stream_client()?;
    let mut traffic = client.subscribe(StreamKind::Traffic, StreamOptions::default());
    let mut traffic_active = true;

    loop {
        tokio::select! {
            _ = wait_for_cancellation(token.clone()) => break,
            event = traffic.events.recv(), if traffic_active => {
                traffic_active = forward_monitoring_event(&services, event);
            }
        }
    }

    traffic.cancel();
    Ok(())
}

async fn run_connections_monitoring(
    services: Arc<AppServices>,
    token: air_app::runtime::CancellationToken,
) -> AppResult<()> {
    if !runtime_api_available(&services) {
        return Ok(());
    }

    // 连接页需要 500ms 粒度的活动连接快照，单独订阅 `/connections`
    // WebSocket，避免和日志页、状态栏流量订阅产生页面生命周期耦合。
    let client = services.mihomo_clients.stream_client()?;
    let options = StreamOptions {
        connection_interval_ms: Some(500),
        ..StreamOptions::default()
    };
    let mut connections = client.subscribe(StreamKind::Connections, options);
    let mut connections_active = true;

    loop {
        tokio::select! {
            _ = wait_for_cancellation(token.clone()) => break,
            event = connections.events.recv(), if connections_active => {
                connections_active = forward_monitoring_event(&services, event);
            }
        }
    }

    connections.cancel();
    Ok(())
}

fn forward_monitoring_event(
    services: &AppServices,
    event: Result<air_mihomo::StreamEvent, tokio::sync::broadcast::error::RecvError>,
) -> bool {
    match event {
        Ok(event) => {
            services
                .runtime
                .emit(air_app::AppEvent::MihomoStreamEvent(event));
            true
        }
        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
            tracing::warn!(skipped, "monitoring stream receiver lagged");
            true
        }
        Err(tokio::sync::broadcast::error::RecvError::Closed) => false,
    }
}

struct CoreLogTail {
    path: PathBuf,
    offset: u64,
    pending: String,
}

impl CoreLogTail {
    fn from_start(path: PathBuf) -> Self {
        Self {
            path,
            offset: 0,
            pending: String::new(),
        }
    }

    async fn poll_new_events(&mut self) -> Vec<StreamEvent> {
        match self.read_new_text().await {
            Ok(text) => self.consume_text(&text),
            Err(error) => {
                tracing::debug!(
                    error = %redact_log_value(&error.to_string()),
                    path = %self.path.display(),
                    "core log tail read skipped"
                );
                Vec::new()
            }
        }
    }

    async fn read_new_text(&mut self) -> std::io::Result<String> {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};

        let metadata = tokio::fs::metadata(&self.path).await?;
        if metadata.len() < self.offset {
            // core.log 可能被用户清空或日志轮转；检测到文件变短时从头重新接续，不复用旧偏移。
            self.offset = 0;
            self.pending.clear();
        }
        if metadata.len() == self.offset {
            return Ok(String::new());
        }

        let mut file = tokio::fs::File::open(&self.path).await?;
        file.seek(std::io::SeekFrom::Start(self.offset)).await?;
        let mut text = String::new();
        let bytes_read = file.read_to_string(&mut text).await?;
        self.offset = self.offset.saturating_add(bytes_read as u64);
        Ok(text)
    }

    fn consume_text(&mut self, text: &str) -> Vec<StreamEvent> {
        if text.is_empty() {
            return Vec::new();
        }
        self.pending.push_str(text);
        let mut complete = std::mem::take(&mut self.pending);
        if !complete.ends_with('\n') {
            if let Some(index) = complete.rfind('\n') {
                self.pending = complete.split_off(index + 1);
            } else {
                self.pending = complete;
                return Vec::new();
            }
        }
        complete
            .lines()
            .filter_map(core_log_line_to_stream_event)
            .collect()
    }
}

pub(super) fn core_log_line_to_stream_event(line: &str) -> Option<StreamEvent> {
    let line = line.trim_end_matches('\r').trim();
    if line.is_empty() {
        return None;
    }
    let line = strip_core_log_timestamp(line);
    let (level, message) = if let Some(message) = line.strip_prefix("[stderr]") {
        ("error", message.trim())
    } else if let Some(message) = line.strip_prefix("[stdout]") {
        ("info", message.trim())
    } else {
        ("info", line)
    };
    Some(StreamEvent::Log {
        level: level.to_string(),
        // core.log 是跨权限边界的落盘通道，进入 UI 事件前仍然统一脱敏。
        message: redact_log_value(message),
    })
}

fn strip_core_log_timestamp(line: &str) -> &str {
    let Some(rest) = line.strip_prefix('[') else {
        return line;
    };
    let Some(index) = rest.find(']') else {
        return line;
    };
    let candidate = &rest[..index];
    if looks_like_rfc3339_timestamp(candidate) {
        rest[index + 1..].trim_start()
    } else {
        line
    }
}

fn looks_like_rfc3339_timestamp(value: &str) -> bool {
    value.len() >= 20
        && value.as_bytes().get(4) == Some(&b'-')
        && value.as_bytes().get(7) == Some(&b'-')
        && value.as_bytes().get(10) == Some(&b'T')
}
