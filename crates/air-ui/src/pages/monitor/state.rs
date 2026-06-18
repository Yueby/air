use std::collections::VecDeque;

use air_mihomo::streams::StreamEvent;
use air_telemetry::redaction::redact_log_value;

use super::format::{format_bytes, visible_log_text};
pub const MAX_LOG_ENTRIES: usize = 200;
pub const MAX_METRIC_POINTS: usize = 1;

#[derive(Clone, Debug)]
pub struct MonitorPageState {
    pub(crate) logs: VecDeque<LogEntry>,
    pub(crate) traffic_points: VecDeque<TrafficPoint>,
    connection: StreamConnectionState,
    next_log_sequence: u64,
    active_filter: LogLevelFilter,
    search_query: String,
}

impl Default for MonitorPageState {
    fn default() -> Self {
        Self {
            logs: VecDeque::new(),
            traffic_points: VecDeque::new(),
            connection: StreamConnectionState::Stopped,
            next_log_sequence: 1,
            active_filter: LogLevelFilter::All,
            search_query: String::new(),
        }
    }
}

impl MonitorPageState {
    pub fn apply_stream_event(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::Log { level, message } => {
                let entry = LogEntry {
                    sequence: self.next_log_sequence,
                    level: LogLevel::from_mihomo(&level),
                    // core.log 和 websocket 日志都可能跨服务边界进入 UI，展示前再次脱敏，避免订阅 URL/secret 外泄。
                    message: redact_log_value(&message),
                };
                self.next_log_sequence += 1;
                self.logs.push_back(entry);
                trim_front(&mut self.logs, MAX_LOG_ENTRIES);
                self.connection = StreamConnectionState::Streaming;
            }
            StreamEvent::Traffic { upload, download } => {
                // 状态栏只展示最新上下行速度；不再为不可见的历史曲线保留采样窗口，降低运行期常驻内存。
                self.traffic_points
                    .push_back(TrafficPoint { upload, download });
                trim_front(&mut self.traffic_points, MAX_METRIC_POINTS);
                self.connection = StreamConnectionState::Streaming;
            }
            StreamEvent::Memory { .. } => {
                // 内存曲线已经不再渲染；保留事件消费只用于维持流连接状态一致。
                self.connection = StreamConnectionState::Streaming;
            }
            StreamEvent::Disconnected { .. } => {}
            StreamEvent::Connections(_) => {}
        }
    }

    pub fn stop_streams(&mut self) {
        self.connection = StreamConnectionState::Stopped;
    }

    pub fn release_transient_stream_state(&mut self) {
        // traffic 流在离开页面或隐藏到托盘后可以完全丢弃，恢复页面时再由实时流重新建立。
        self.connection = StreamConnectionState::Stopped;
        self.traffic_points.clear();
    }

    pub fn clear_logs(&mut self) {
        self.logs.clear();
    }

    pub fn set_filter(&mut self, filter: LogLevelFilter) {
        self.active_filter = filter;
    }

    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
    }

    pub fn upload_text(&self) -> String {
        self.traffic_points
            .back()
            .map(|point| format!("{}/s", format_bytes(point.upload)))
            .unwrap_or_else(|| "0 B/s".to_string())
    }

    pub fn download_text(&self) -> String {
        self.traffic_points
            .back()
            .map(|point| format!("{}/s", format_bytes(point.download)))
            .unwrap_or_else(|| "0 B/s".to_string())
    }

    pub fn view_model(&self) -> MonitorViewModel {
        let query = self.search_query.trim().to_ascii_lowercase();
        let filtered = self
            .logs
            .iter()
            .filter(|entry| self.active_filter.matches(entry.level))
            .filter(|entry| {
                query.is_empty()
                    || entry.message.to_ascii_lowercase().contains(&query)
                    || entry.level.label().to_ascii_lowercase().contains(&query)
            })
            .collect::<Vec<_>>();
        let filtered_count = filtered.len();
        let visible_logs = filtered
            .into_iter()
            .map(|entry| LogEntryView {
                sequence_label: format!("#{:04}", entry.sequence),
                level: entry.level,
                level_label: entry.level.label().to_string(),
                message: entry.message.clone(),
            })
            .collect::<Vec<_>>();

        MonitorViewModel {
            connection: self.connection.clone(),
            active_filter: self.active_filter,
            search_query: self.search_query.clone(),
            log_count: self.logs.len(),
            filtered_count,
            rendered_count: visible_logs.len(),
            visible_logs,
        }
    }

    pub fn filtered_log_text(&self) -> String {
        visible_log_text(&self.view_model())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamConnectionState {
    Streaming,
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogLevelFilter {
    All,
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevelFilter {
    pub const ALL: [Self; 5] = [
        Self::All,
        Self::Debug,
        Self::Info,
        Self::Warning,
        Self::Error,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::All => "全部",
            Self::Debug => "Debug",
            Self::Info => "Info",
            Self::Warning => "Warn",
            Self::Error => "Error",
        }
    }

    fn matches(self, level: LogLevel) -> bool {
        match self {
            Self::All => true,
            Self::Debug => level == LogLevel::Debug,
            Self::Info => level == LogLevel::Info,
            Self::Warning => level == LogLevel::Warning,
            Self::Error => level == LogLevel::Error,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Unknown,
}

impl LogLevel {
    fn from_mihomo(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "debug" => Self::Debug,
            "info" | "information" => Self::Info,
            "warn" | "warning" => Self::Warning,
            "error" | "fatal" | "panic" => Self::Error,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warning => "WARN",
            Self::Error => "ERROR",
            Self::Unknown => "LOG",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LogEntry {
    sequence: u64,
    level: LogLevel,
    message: String,
}

#[derive(Clone, Debug)]
pub(crate) struct TrafficPoint {
    upload: u64,
    download: u64,
}

#[derive(Clone, Debug)]
pub struct MonitorViewModel {
    pub connection: StreamConnectionState,
    pub active_filter: LogLevelFilter,
    pub search_query: String,
    pub log_count: usize,
    pub filtered_count: usize,
    pub rendered_count: usize,
    pub visible_logs: Vec<LogEntryView>,
}

#[derive(Clone, Debug)]
pub struct LogEntryView {
    pub sequence_label: String,
    pub level: LogLevel,
    pub level_label: String,
    pub message: String,
}

fn trim_front<T>(items: &mut VecDeque<T>, capacity: usize) {
    while items.len() > capacity {
        items.pop_front();
    }
}
