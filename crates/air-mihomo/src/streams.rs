use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use futures::StreamExt;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;

use air_error::{ApiError, AppResult};
use air_mihomo::MihomoEndpoint;
use air_mihomo::client::build_url;
use air_telemetry::redaction::redact_log_value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamKind {
    Logs,
    Traffic,
    Memory,
    Connections,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamOptions {
    pub log_level: Option<String>,
    pub connection_interval_ms: Option<u64>,
    pub reconnect: ReconnectPolicy,
}

impl Default for StreamOptions {
    fn default() -> Self {
        Self {
            log_level: None,
            connection_interval_ms: Some(500),
            reconnect: ReconnectPolicy::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconnectPolicy {
    pub max_attempts: usize,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(5),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum StreamEvent {
    Log { level: String, message: String },
    Traffic { upload: u64, download: u64 },
    Memory { in_use: u64, os_limit: Option<u64> },
    Connections(Value),
    Disconnected { attempt: usize, next_delay_ms: u64 },
}

#[derive(Clone, Debug)]
pub struct StreamCancellation {
    canceled: Arc<AtomicBool>,
}

impl StreamCancellation {
    pub fn new() -> Self {
        Self {
            canceled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.canceled.store(true, Ordering::SeqCst);
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::SeqCst)
    }
}

impl Default for StreamCancellation {
    fn default() -> Self {
        Self::new()
    }
}

pub struct StreamSubscription {
    pub events: broadcast::Receiver<StreamEvent>,
    cancellation: StreamCancellation,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl StreamSubscription {
    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub async fn wait(mut self) -> AppResult<()> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };
        handle
            .await
            .map_err(|error| ApiError::Request(error.to_string()).into())
    }
}

impl Drop for StreamSubscription {
    fn drop(&mut self) {
        // GUI 路由切换或监控命令取消时，不能只依赖 HTTP 流自然结束；
        // 主动 abort 后台任务，避免已废弃的订阅继续占用 controller 连接。
        self.cancellation.cancel();
        if let Some(handle) = &self.handle {
            handle.abort();
        }
    }
}

#[derive(Clone, Debug)]
pub struct MihomoStreamClient {
    http: reqwest::Client,
    endpoint: MihomoEndpoint,
}

impl MihomoStreamClient {
    pub fn new(endpoint: MihomoEndpoint) -> Self {
        Self {
            http: controller_stream_http_client(),
            endpoint,
        }
    }

    pub fn subscribe(&self, kind: StreamKind, options: StreamOptions) -> StreamSubscription {
        tracing::info!(?kind, ?options, base_url = %self.endpoint.base_url, "subscribing to mihomo stream");
        let (events, _) = broadcast::channel(512);
        let cancellation = StreamCancellation::new();
        let runner = StreamRunner {
            http: self.http.clone(),
            endpoint: self.endpoint.clone(),
            kind,
            options,
            events: events.clone(),
            cancellation: cancellation.clone(),
        };
        let handle = tokio::spawn(async move {
            runner.run().await;
        });
        StreamSubscription {
            events: events.subscribe(),
            cancellation,
            handle: Some(handle),
        }
    }
}

fn controller_stream_http_client() -> reqwest::Client {
    // 流式监控同样只访问 mihomo external-controller，必须绕过环境代理，避免日志/流量
    // HTTP 流被系统代理或 TUN 链路劫持，导致本地控制面连接异常。
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("mihomo stream controller client options should be valid")
}

struct StreamRunner {
    http: reqwest::Client,
    endpoint: MihomoEndpoint,
    kind: StreamKind,
    options: StreamOptions,
    events: broadcast::Sender<StreamEvent>,
    cancellation: StreamCancellation,
}

impl StreamRunner {
    async fn run(self) {
        let mut attempt = 0;
        tracing::info!(
            kind = ?self.kind,
            max_attempts = self.options.reconnect.max_attempts,
            base_url = %self.endpoint.base_url,
            "mihomo stream runner started"
        );
        loop {
            if self.cancellation.is_canceled() || attempt >= self.options.reconnect.max_attempts {
                tracing::info!(
                    kind = ?self.kind,
                    canceled = self.cancellation.is_canceled(),
                    attempts = attempt,
                    "mihomo stream runner stopping"
                );
                break;
            }
            match self.connect_once().await {
                Ok(()) => {
                    tracing::info!(kind = ?self.kind, "mihomo stream connection completed cleanly");
                    attempt = 0;
                }
                Err(error) => {
                    tracing::warn!(kind = ?self.kind, attempt = attempt + 1, error = %error, "mihomo stream connection failed");
                    attempt += 1;
                    let delay = backoff_delay(&self.options.reconnect, attempt);
                    let _ = self.events.send(StreamEvent::Disconnected {
                        attempt,
                        next_delay_ms: delay.as_millis() as u64,
                    });
                    tracing::info!(
                        kind = ?self.kind,
                        attempt,
                        delay_ms = delay.as_millis() as u64,
                        "mihomo stream scheduled reconnect"
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    async fn connect_once(&self) -> AppResult<()> {
        if self.kind == StreamKind::Connections {
            return self.connect_websocket_once().await;
        }

        let (segments, query) = stream_path_and_query(self.kind, &self.options);
        let url = build_url(&self.endpoint.base_url, &segments, &query)?;
        tracing::info!(kind = ?self.kind, url = %url, "connecting mihomo http stream");
        let mut request = self.http.request(Method::GET, url);
        if let Some(secret) = &self.endpoint.secret {
            request = request.bearer_auth(secret);
        }
        let response = request
            .send()
            .await
            .map_err(|error| ApiError::Request(error.to_string()))?;
        tracing::info!(kind = ?self.kind, status = response.status().as_u16(), "mihomo http stream connected");
        if !response.status().is_success() {
            return Err(ApiError::HttpStatus {
                status: response.status().as_u16(),
                body: response.text().await.unwrap_or_default(),
            }
            .into());
        }
        let mut buffer = String::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if self.cancellation.is_canceled() {
                return Ok(());
            }
            let chunk = chunk.map_err(|error| ApiError::Request(error.to_string()))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(index) = buffer.find('\n') {
                let line = buffer[..index].trim().to_owned();
                buffer = buffer[index + 1..].to_owned();
                if line.is_empty() {
                    continue;
                }
                tracing::debug!(kind = ?self.kind, line = %line, "received mihomo stream line");
                if let Some(event) = parse_stream_line(self.kind, &line)? {
                    let _ = self.events.send(event);
                }
            }
        }
        tracing::warn!(kind = ?self.kind, "mihomo http stream closed");
        Err(ApiError::StreamClosed.into())
    }

    async fn connect_websocket_once(&self) -> AppResult<()> {
        let (segments, query) = stream_path_and_query(self.kind, &self.options);
        let url = websocket_url(&self.endpoint.base_url, &segments, &query)?;
        tracing::info!(kind = ?self.kind, url = %url, "connecting mihomo websocket stream");
        let mut request = url
            .as_str()
            .into_client_request()
            .map_err(|error| ApiError::Request(error.to_string()))?;
        if let Some(secret) = &self.endpoint.secret {
            let value = format!("Bearer {secret}").parse().map_err(
                |error: tokio_tungstenite::tungstenite::http::header::InvalidHeaderValue| {
                    ApiError::Request(error.to_string())
                },
            )?;
            request.headers_mut().insert(AUTHORIZATION, value);
        }
        let (mut stream, _) = connect_async(request)
            .await
            .map_err(|error| ApiError::Request(error.to_string()))?;
        tracing::info!(kind = ?self.kind, "mihomo websocket stream connected");

        while let Some(message) = stream.next().await {
            if self.cancellation.is_canceled() {
                return Ok(());
            }
            let message = message.map_err(|error| ApiError::Request(error.to_string()))?;
            if message.is_close() {
                tracing::warn!(kind = ?self.kind, "mihomo websocket stream received close frame");
                return Err(ApiError::StreamClosed.into());
            }
            if !message.is_text() && !message.is_binary() {
                continue;
            }
            let payload = if message.is_text() {
                message
                    .to_text()
                    .map_err(|error| ApiError::Request(error.to_string()))?
                    .to_owned()
            } else {
                String::from_utf8_lossy(&message.into_data()).to_string()
            };
            for line in payload
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
            {
                tracing::debug!(kind = ?self.kind, line = %line, "received mihomo websocket stream line");
                if let Some(event) = parse_stream_line(self.kind, line)? {
                    let _ = self.events.send(event);
                }
            }
        }
        tracing::warn!(kind = ?self.kind, "mihomo websocket stream closed");
        Err(ApiError::StreamClosed.into())
    }
}

pub fn parse_stream_line(kind: StreamKind, line: &str) -> AppResult<Option<StreamEvent>> {
    let value: Value =
        serde_json::from_str(line).map_err(|error| ApiError::Json(error.to_string()))?;
    let event = match kind {
        StreamKind::Logs => StreamEvent::Log {
            level: value
                .get("type")
                .or_else(|| value.get("level"))
                .and_then(|value| value.as_str())
                .unwrap_or("info")
                .to_owned(),
            message: redact_log_value(
                value
                    .get("payload")
                    .or_else(|| value.get("message"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default(),
            ),
        },
        StreamKind::Traffic => StreamEvent::Traffic {
            upload: value
                .get("up")
                .or_else(|| value.get("upload"))
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            download: value
                .get("down")
                .or_else(|| value.get("download"))
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
        },
        StreamKind::Memory => StreamEvent::Memory {
            in_use: value
                .get("inuse")
                .or_else(|| value.get("in_use"))
                .and_then(|value| value.as_u64())
                .unwrap_or_default(),
            os_limit: value.get("oslimit").and_then(|value| value.as_u64()),
        },
        StreamKind::Connections => StreamEvent::Connections(value),
    };
    Ok(Some(event))
}

pub fn backoff_delay(policy: &ReconnectPolicy, attempt: usize) -> Duration {
    let multiplier = 2_u32.saturating_pow(attempt.saturating_sub(1) as u32);
    policy
        .initial_delay
        .saturating_mul(multiplier)
        .min(policy.max_delay)
}

fn stream_path_and_query(
    kind: StreamKind,
    options: &StreamOptions,
) -> (Vec<&'static str>, Vec<(&'static str, String)>) {
    match kind {
        StreamKind::Logs => {
            let mut query = Vec::new();
            if let Some(level) = &options.log_level {
                query.push(("level", level.clone()));
            }
            (vec!["logs"], query)
        }
        StreamKind::Traffic => (vec!["traffic"], Vec::new()),
        StreamKind::Memory => (vec!["memory"], Vec::new()),
        StreamKind::Connections => (
            vec!["connections"],
            options
                .connection_interval_ms
                .map(|interval| vec![("interval", interval.to_string())])
                .unwrap_or_default(),
        ),
    }
}

fn websocket_url(base: &str, segments: &[&str], query: &[(&str, String)]) -> AppResult<url::Url> {
    let mut url = build_url(base, segments, query)?;
    // mihomo external-controller 通常配置为 http/https 地址；连接流需要升级为
    // ws/wss，避免继续使用 HTTP body stream 伪装成 WebSocket。
    let scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        "ws" => "ws",
        "wss" => "wss",
        scheme => {
            return Err(ApiError::Request(format!(
                "unsupported websocket base url scheme: {scheme}"
            ))
            .into());
        }
    };
    url.set_scheme(scheme)
        .map_err(|_| ApiError::Request("failed to convert controller URL to websocket".into()))?;
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_redacts_log_stream_event() {
        let event = parse_stream_line(
            StreamKind::Logs,
            r#"{"type":"info","payload":"fetch secret=abc token=def"}"#,
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            event,
            StreamEvent::Log {
                level: "info".into(),
                message: "fetch secret=*** token=***".into()
            }
        );
    }

    #[test]
    fn parses_traffic_stream_alias_fields() {
        let event = parse_stream_line(StreamKind::Traffic, r#"{"upload":128,"download":256}"#)
            .unwrap()
            .unwrap();

        assert_eq!(
            event,
            StreamEvent::Traffic {
                upload: 128,
                download: 256
            }
        );
    }

    #[test]
    fn reconnect_backoff_is_bounded() {
        let policy = ReconnectPolicy {
            max_attempts: 10,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(350),
        };

        assert_eq!(backoff_delay(&policy, 1), Duration::from_millis(100));
        assert_eq!(backoff_delay(&policy, 3), Duration::from_millis(350));
        assert_eq!(backoff_delay(&policy, 10), Duration::from_millis(350));
    }

    #[test]
    fn default_connections_stream_interval_is_500ms() {
        let options = StreamOptions::default();
        let (_, query) = stream_path_and_query(StreamKind::Connections, &options);

        assert_eq!(query, vec![("interval", "500".to_string())]);
    }

    #[test]
    fn websocket_url_converts_controller_http_scheme() {
        let url = websocket_url(
            "http://127.0.0.1:9090",
            &["connections"],
            &[("interval", "500".into())],
        )
        .expect("http controller URL should convert to ws URL");

        assert_eq!(url.as_str(), "ws://127.0.0.1:9090/connections?interval=500");
    }

    #[tokio::test]
    async fn cancellation_stops_fake_task() {
        let cancellation = StreamCancellation::new();
        let observed = cancellation.clone();
        let handle = tokio::spawn(async move {
            while !observed.is_canceled() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        });

        cancellation.cancel();
        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .unwrap()
            .unwrap();
    }
}
