use async_trait::async_trait;
use reqwest::{Method, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use url::Url;

use air_error::{ApiError, AppResult};
use air_mihomo::MihomoEndpoint;
use air_mihomo::dto::{
    ConfigsResponse, ConnectionsResponse, DelayResponse, GroupDelayResponse, GroupsResponse,
    PathPayloadRequest, ProvidersResponse, ProxiesResponse, RulesResponse, SelectProxyRequest,
    VersionResponse,
};
use air_mihomo::{MihomoApiClient, ProxySummary, RuntimeInfo};
use air_telemetry::redaction::redact_log_value;

#[derive(Clone, Debug)]
pub struct MihomoHttpClient {
    http: reqwest::Client,
    endpoint: MihomoEndpoint,
}

impl MihomoHttpClient {
    pub fn new(endpoint: MihomoEndpoint) -> Self {
        Self {
            http: controller_http_client(),
            endpoint,
        }
    }

    pub async fn version(&self) -> AppResult<VersionResponse> {
        self.get_json(&["version"], &[]).await
    }

    pub async fn configs(&self) -> AppResult<ConfigsResponse> {
        self.get_json(&["configs"], &[]).await
    }

    pub async fn reload_configs(&self, force: bool, path: &str, payload: &str) -> AppResult<()> {
        self.request_empty(
            Method::PUT,
            &["configs"],
            &[("force", force.to_string())],
            Some(&PathPayloadRequest { path, payload }),
        )
        .await
    }

    pub async fn reload_configs_default(&self) -> AppResult<()> {
        // mihomo 的 PUT /configs 支持空 path/payload，表示按当前核心默认配置路径重载。
        // Air 在调用前会先写出 core.runtime.config.yaml；这里保持 body 为空对象语义，避免把配置正文暴露进 API 日志。
        self.request_empty(Method::PUT, &["configs"], &[], Some(&serde_json::json!({})))
            .await
    }

    pub async fn patch_configs<T: Serialize + ?Sized>(&self, patch: &T) -> AppResult<()> {
        self.request_empty(Method::PATCH, &["configs"], &[], Some(patch))
            .await
    }

    pub async fn proxies(&self) -> AppResult<ProxiesResponse> {
        self.get_json(&["proxies"], &[]).await
    }

    pub async fn proxy(&self, name: &str) -> AppResult<Value> {
        self.get_json(&["proxies", name], &[]).await
    }

    pub async fn select_proxy(&self, group: &str, proxy: &str) -> AppResult<()> {
        self.request_empty(
            Method::PUT,
            &["proxies", group],
            &[],
            Some(&SelectProxyRequest { name: proxy }),
        )
        .await
    }

    pub async fn proxy_delay(
        &self,
        name: &str,
        url: &str,
        timeout_ms: u64,
    ) -> AppResult<DelayResponse> {
        self.get_json(
            &["proxies", name, "delay"],
            &[("url", url.to_owned()), ("timeout", timeout_ms.to_string())],
        )
        .await
    }

    pub async fn groups(&self) -> AppResult<GroupsResponse> {
        self.get_json(&["group"], &[]).await
    }

    pub async fn group(&self, name: &str) -> AppResult<Value> {
        self.get_json(&["group", name], &[]).await
    }

    pub async fn clear_group_fixed(&self, name: &str) -> AppResult<()> {
        self.request_empty::<Value>(Method::DELETE, &["group", name], &[], None)
            .await
    }

    pub async fn group_delay(
        &self,
        name: &str,
        url: &str,
        timeout_ms: u64,
    ) -> AppResult<GroupDelayResponse> {
        self.get_json(
            &["group", name, "delay"],
            &[("url", url.to_owned()), ("timeout", timeout_ms.to_string())],
        )
        .await
    }

    pub async fn rules(&self) -> AppResult<RulesResponse> {
        self.get_json(&["rules"], &[]).await
    }

    pub async fn disable_rules<T: Serialize + ?Sized>(&self, patch: &T) -> AppResult<()> {
        self.request_empty(Method::PATCH, &["rules", "disable"], &[], Some(patch))
            .await
    }

    pub async fn proxy_providers(&self) -> AppResult<ProvidersResponse> {
        self.get_json(&["providers", "proxies"], &[]).await
    }

    pub async fn proxy_provider(&self, name: &str) -> AppResult<Value> {
        self.get_json(&["providers", "proxies", name], &[]).await
    }

    pub async fn update_proxy_provider(&self, name: &str) -> AppResult<()> {
        self.request_empty::<Value>(Method::PUT, &["providers", "proxies", name], &[], None)
            .await
    }

    pub async fn healthcheck_proxy_provider(&self, name: &str) -> AppResult<()> {
        self.request_empty::<Value>(
            Method::GET,
            &["providers", "proxies", name, "healthcheck"],
            &[],
            None,
        )
        .await
    }

    pub async fn healthcheck_provider_proxy(
        &self,
        provider: &str,
        proxy: &str,
        url: &str,
        timeout_ms: u64,
    ) -> AppResult<DelayResponse> {
        self.get_json(
            &["providers", "proxies", provider, proxy, "healthcheck"],
            &[("url", url.to_owned()), ("timeout", timeout_ms.to_string())],
        )
        .await
    }

    pub async fn rule_providers(&self) -> AppResult<ProvidersResponse> {
        self.get_json(&["providers", "rules"], &[]).await
    }

    pub async fn rule_provider(&self, name: &str) -> AppResult<Value> {
        self.get_json(&["providers", "rules", name], &[]).await
    }

    pub async fn update_rule_provider(&self, name: &str) -> AppResult<()> {
        self.request_empty::<Value>(Method::PUT, &["providers", "rules", name], &[], None)
            .await
    }

    pub async fn connections(&self) -> AppResult<ConnectionsResponse> {
        self.get_json(&["connections"], &[]).await
    }

    pub async fn close_all_connections(&self) -> AppResult<()> {
        self.request_empty::<Value>(Method::DELETE, &["connections"], &[], None)
            .await
    }

    pub async fn close_connection(&self, id: &str) -> AppResult<()> {
        self.request_empty::<Value>(Method::DELETE, &["connections", id], &[], None)
            .await
    }

    pub async fn flush_fakeip_cache(&self) -> AppResult<()> {
        self.request_empty::<Value>(Method::POST, &["cache", "fakeip", "flush"], &[], None)
            .await
    }

    pub async fn flush_dns_cache(&self) -> AppResult<()> {
        self.request_empty::<Value>(Method::POST, &["cache", "dns", "flush"], &[], None)
            .await
    }

    pub async fn restart_core(&self, path: &str, payload: &str) -> AppResult<()> {
        self.request_empty(
            Method::POST,
            &["restart"],
            &[],
            Some(&PathPayloadRequest { path, payload }),
        )
        .await
    }

    pub async fn restart_core_default(&self) -> AppResult<()> {
        // 当前 mihomo API 文档中的 POST /restart 无入参；重启前由 app 层重新生成运行时配置。
        self.request_empty::<Value>(Method::POST, &["restart"], &[], None)
            .await
    }

    pub async fn upgrade_core(&self, path: &str, payload: &str) -> AppResult<()> {
        self.request_empty(
            Method::POST,
            &["upgrade"],
            &[],
            Some(&PathPayloadRequest { path, payload }),
        )
        .await
    }

    pub async fn upgrade_ui(&self) -> AppResult<()> {
        self.request_empty::<Value>(Method::POST, &["upgrade", "ui"], &[], None)
            .await
    }

    pub async fn upgrade_geo(&self, path: &str, payload: &str) -> AppResult<()> {
        self.request_empty(
            Method::POST,
            &["upgrade", "geo"],
            &[],
            Some(&PathPayloadRequest { path, payload }),
        )
        .await
    }

    pub async fn update_geo(&self, path: &str, payload: &str) -> AppResult<()> {
        self.request_empty(
            Method::POST,
            &["configs", "geo"],
            &[],
            Some(&PathPayloadRequest { path, payload }),
        )
        .await
    }

    pub async fn dns_query(&self, name: &str, record_type: &str) -> AppResult<Value> {
        self.get_json(
            &["dns", "query"],
            &[("name", name.to_owned()), ("type", record_type.to_owned())],
        )
        .await
    }

    pub async fn debug_gc(&self) -> AppResult<()> {
        self.request_empty::<Value>(Method::PUT, &["debug", "gc"], &[], None)
            .await
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        segments: &[&str],
        query: &[(&str, String)],
    ) -> AppResult<T> {
        let method = Method::GET;
        let url = self.request_url(segments, query)?;
        let started_at = Instant::now();
        log_mihomo_api_request(&method, &url, None, self.endpoint.secret.is_some());
        let response = self.request(method.clone(), url.clone()).send().await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                log_mihomo_api_request_error(&method, &url, started_at, &error.to_string());
                return Err(ApiError::Request(error.to_string()).into());
            }
        };
        decode_response(response, &method, &url, started_at).await
    }

    async fn request_empty<T: Serialize + ?Sized>(
        &self,
        method: Method,
        segments: &[&str],
        query: &[(&str, String)],
        body: Option<&T>,
    ) -> AppResult<()> {
        let url = self.request_url(segments, query)?;
        let body_log = redacted_json_log_body(body);
        let started_at = Instant::now();
        log_mihomo_api_request(
            &method,
            &url,
            body_log.as_deref(),
            self.endpoint.secret.is_some(),
        );
        let mut request = self.request(method.clone(), url.clone());
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                log_mihomo_api_request_error(&method, &url, started_at, &error.to_string());
                return Err(ApiError::Request(error.to_string()).into());
            }
        };
        ensure_success(response, &method, &url, started_at).await
    }

    fn request(&self, method: Method, url: Url) -> reqwest::RequestBuilder {
        let mut request = self.http.request(method, url);
        if let Some(secret) = &self.endpoint.secret {
            request = request.bearer_auth(secret);
        }
        request
    }

    fn request_url(&self, segments: &[&str], query: &[(&str, String)]) -> AppResult<Url> {
        build_url(&self.endpoint.base_url, segments, query)
    }
}

fn controller_http_client() -> reqwest::Client {
    // external-controller 始终是本机/局域网控制面请求，不能继承 HTTP_PROXY/all_proxy 等
    // 环境代理。否则用户开启系统代理或 TUN 时，`127.0.0.1:9090/version` 会被发到代理
    // 端口，引发“核心已监听但健康检查超时”的假失败。
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("mihomo controller client options should be valid")
}

#[async_trait]
impl MihomoApiClient for MihomoHttpClient {
    async fn runtime_info(&self, endpoint: &MihomoEndpoint) -> AppResult<RuntimeInfo> {
        let client = MihomoHttpClient::new(endpoint.clone());
        let version = client.version().await?;
        Ok(RuntimeInfo {
            version: Some(version.version),
            uptime_seconds: None,
        })
    }

    async fn proxies(&self, endpoint: &MihomoEndpoint) -> AppResult<Vec<ProxySummary>> {
        let client = MihomoHttpClient::new(endpoint.clone());
        let proxies = client.proxies().await?;
        Ok(proxies
            .proxies
            .into_iter()
            .map(|(name, proxy)| ProxySummary {
                name,
                kind: proxy.kind,
            })
            .collect())
    }

    async fn select_proxy(
        &self,
        endpoint: &MihomoEndpoint,
        group: &str,
        proxy: &str,
    ) -> AppResult<()> {
        let client = MihomoHttpClient::new(endpoint.clone());
        MihomoHttpClient::select_proxy(&client, group, proxy).await
    }
}

#[async_trait]
pub trait MihomoHealthCheck: Send + Sync {
    async fn health_version(&self) -> AppResult<String>;
}

#[async_trait]
impl MihomoHealthCheck for MihomoHttpClient {
    async fn health_version(&self) -> AppResult<String> {
        Ok(self.version().await?.version)
    }
}

#[async_trait]
impl<T> MihomoHealthCheck for Arc<T>
where
    T: MihomoHealthCheck + ?Sized,
{
    async fn health_version(&self) -> AppResult<String> {
        (**self).health_version().await
    }
}

pub fn build_url(base: &str, segments: &[&str], query: &[(&str, String)]) -> AppResult<Url> {
    let mut url = Url::parse(base).map_err(|error| ApiError::Request(error.to_string()))?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| ApiError::Request("external-controller URL 不能作为路径基准".into()))?;
        path.clear();
        for segment in segments {
            path.push(segment);
        }
    }
    if !query.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query {
            pairs.append_pair(key, value);
        }
    }
    Ok(url)
}

async fn decode_response<T: DeserializeOwned>(
    response: reqwest::Response,
    method: &Method,
    url: &Url,
    started_at: Instant,
) -> AppResult<T> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| ApiError::Request(error.to_string()))?;
    log_mihomo_api_response(method, url, status, started_at, &body);
    if !status.is_success() {
        return Err(http_status_error(status, &body).into());
    }
    let value: Value =
        serde_json::from_str(&body).map_err(|error| ApiError::Json(error.to_string()))?;
    if let Some(message) = value.get("error").and_then(|value| value.as_str()) {
        return Err(ApiError::Business(message.to_owned()).into());
    }
    serde_json::from_value(value).map_err(|error| ApiError::Json(error.to_string()).into())
}

async fn ensure_success(
    response: reqwest::Response,
    method: &Method,
    url: &Url,
    started_at: Instant,
) -> AppResult<()> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| ApiError::Request(error.to_string()))?;
    log_mihomo_api_response(method, url, status, started_at, &body);
    if status.is_success() {
        Ok(())
    } else {
        Err(http_status_error(status, &body).into())
    }
}

fn http_status_error(status: StatusCode, body: &str) -> ApiError {
    ApiError::HttpStatus {
        status: status.as_u16(),
        body: redact_log_value(body),
    }
}

fn redacted_json_log_body<T: Serialize + ?Sized>(body: Option<&T>) -> Option<String> {
    body.map(|body| match serde_json::to_string(body) {
        Ok(value) => redact_log_value(&value),
        Err(error) => format!(
            "<json serialize failed: {}>",
            redact_log_value(&error.to_string())
        ),
    })
}

fn log_mihomo_api_request(method: &Method, url: &Url, body: Option<&str>, auth_present: bool) {
    // mihomo API 日志集中在 HTTP client 边界输出；URL query、请求体和后续响应体统一脱敏，
    // 这样能排查接口参数与返回内容，同时不把 controller secret 或订阅 token 写进日志。
    tracing::info!(
        event = "request",
        method = %method,
        url = %url,
        auth_present,
        body = body.unwrap_or("<empty>"),
        "mihomo api request"
    );
}

fn log_mihomo_api_response(
    method: &Method,
    url: &Url,
    status: StatusCode,
    started_at: Instant,
    body: &str,
) {
    tracing::info!(
        event = "response",
        method = %method,
        url = %url,
        status = status.as_u16(),
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        body = %body,
        "mihomo api response"
    );
}

fn log_mihomo_api_request_error(method: &Method, url: &Url, started_at: Instant, error: &str) {
    tracing::warn!(
        event = "request_error",
        method = %method,
        url = %url,
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        error = %error,
        "mihomo api request failed"
    );
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    #[test]
    fn url_builder_encodes_path_segments_and_query() {
        let url = build_url(
            "http://127.0.0.1:9090",
            &["proxies", "香港 节点", "delay"],
            &[("url", "https://example.test/a?b=1".into())],
        )
        .unwrap();

        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:9090/proxies/%E9%A6%99%E6%B8%AF%20%E8%8A%82%E7%82%B9/delay?url=https%3A%2F%2Fexample.test%2Fa%3Fb%3D1"
        );
    }

    #[test]
    fn provider_proxy_healthcheck_url_matches_docs_api() {
        let url = build_url(
            "http://127.0.0.1:9090",
            &["providers", "proxies", "订阅 A", "香港 节点", "healthcheck"],
            &[
                ("url", "https://example.test/ping".into()),
                ("timeout", "5000".into()),
            ],
        )
        .unwrap();

        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:9090/providers/proxies/%E8%AE%A2%E9%98%85%20A/%E9%A6%99%E6%B8%AF%20%E8%8A%82%E7%82%B9/healthcheck?url=https%3A%2F%2Fexample.test%2Fping&timeout=5000"
        );
    }

    #[test]
    fn mihomo_api_log_body_redacts_secret_fields() {
        let body = serde_json::json!({
            "mode": "rule",
            "secret": "controller-secret",
            "authentication": ["user:password"]
        });

        let redacted = redacted_json_log_body(Some(&body)).expect("body log should be rendered");

        assert!(redacted.contains("\"mode\":\"rule\""));
        assert!(!redacted.contains("controller-secret"));
        assert!(!redacted.contains("password"));
    }

    #[tokio::test]
    async fn sends_authorization_header() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 2048];
            let len = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..len]).to_string();
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer secret-value")
            );
            let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 20\r\n\r\n{\"version\":\"1.19.0\"}";
            stream.write_all(response.as_bytes()).unwrap();
        });
        let client = MihomoHttpClient::new(MihomoEndpoint {
            base_url: format!("http://{addr}"),
            secret: Some("secret-value".into()),
        });

        let version = client.version().await.unwrap();

        assert_eq!(version.version, "1.19.0");
        handle.join().unwrap();
    }

    #[tokio::test]
    async fn proxy_delay_accepts_mihomo_single_node_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 2048];
            let len = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..len]).to_string();
            assert!(request.starts_with(
                "GET /proxies/%F0%9F%87%AD%F0%9F%87%B0%20Hong%20Kong%E4%B8%A801/delay?"
            ));
            let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 13\r\n\r\n{\"delay\":934}";
            stream.write_all(response.as_bytes()).unwrap();
        });
        let client = MihomoHttpClient::new(MihomoEndpoint {
            base_url: format!("http://{addr}"),
            secret: None,
        });

        let delay = client
            .proxy_delay("🇭🇰 Hong Kong丨01", "https://example.test/ping", 5000)
            .await
            .expect("mihomo single node delay body should parse");

        assert_eq!(delay.delay, 934);
        handle.join().unwrap();
    }

    #[tokio::test]
    async fn separates_http_status_and_json_errors() {
        let status = http_status_error(StatusCode::UNAUTHORIZED, "secret=abc");
        assert!(matches!(status, ApiError::HttpStatus { .. }));
        assert!(!status.to_string().contains("abc"));
    }
}
