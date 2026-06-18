use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use air_config::ConfigDocument;
use air_error::{AppResult, ConfigError};
use air_mihomo::subscriptions::{
    ParsedSubscription, SubscriptionCacheMetadata, SubscriptionDiagnostic,
    SubscriptionPipelineError, SubscriptionRequestHeaders, SubscriptionSource,
    SubscriptionSourceKind, SubscriptionTimestamp, SubscriptionUpdateCacheStore,
    SubscriptionUpdateOutcome, SubscriptionUpdatePipeline, SubscriptionUpdateResult,
};
use air_storage::SubscriptionStore;
use air_telemetry::redaction::redact_log_value;

#[derive(Clone)]
pub struct SubscriptionController {
    store: SubscriptionStore,
}

impl SubscriptionController {
    pub fn new(store: SubscriptionStore) -> Self {
        Self { store }
    }

    pub fn load_projection(&self) -> AppResult<SubscriptionStateProjection> {
        let sources = self.store.load_sources()?;
        let caches = self.store.load_cache_metadata()?;
        Ok(self.projection_from_parts(sources, caches))
    }

    pub async fn import_url(
        &self,
        subscription_id: impl AsRef<str>,
        url: impl AsRef<str>,
    ) -> AppResult<SubscriptionStateProjection> {
        self.import_url_with_core_version(subscription_id, url, None)
            .await
    }

    pub async fn import_url_with_core_version(
        &self,
        subscription_id: impl AsRef<str>,
        url: impl AsRef<str>,
        core_version: Option<&str>,
    ) -> AppResult<SubscriptionStateProjection> {
        let mut source =
            self.remote_source(subscription_id.as_ref(), url.as_ref(), core_version)?;
        let staging = StagingSubscriptionStore::default();
        let pipeline = SubscriptionUpdatePipeline::new(staging.clone())
            .with_core_version(core_version.map(ToOwned::to_owned));
        let report = pipeline
            .update(&source)
            .await
            .map_err(subscription_pipeline_error)?;
        let content = staging
            .content()
            .ok_or_else(|| ConfigError::Subscription("订阅导入未产生可写入的缓存正文".into()))?;
        let result = report
            .metadata
            .last_update
            .clone()
            .ok_or_else(|| ConfigError::Subscription("订阅导入缺少更新结果".into()))?;
        if let Some(display_name) = report.display_name {
            source.name = display_name;
        }

        self.store
            .import_remote_subscription_cache(source, result, &content)?;
        self.load_projection()
    }

    pub async fn update(
        &self,
        source: &SubscriptionSource,
    ) -> AppResult<SubscriptionStateProjection> {
        self.update_with_core_version(source, None).await
    }

    pub async fn update_with_core_version(
        &self,
        source: &SubscriptionSource,
        core_version: Option<&str>,
    ) -> AppResult<SubscriptionStateProjection> {
        self.update_with_core_version_and_disabled_policy(source, core_version, false)
            .await
    }

    pub async fn refresh_cache_with_core_version(
        &self,
        source: &SubscriptionSource,
        core_version: Option<&str>,
    ) -> AppResult<SubscriptionStateProjection> {
        self.update_with_core_version_and_disabled_policy(source, core_version, true)
            .await
    }

    async fn update_with_core_version_and_disabled_policy(
        &self,
        source: &SubscriptionSource,
        core_version: Option<&str>,
        allow_disabled_source: bool,
    ) -> AppResult<SubscriptionStateProjection> {
        let pipeline = SubscriptionUpdatePipeline::new(self.store.clone())
            .with_core_version(core_version.map(ToOwned::to_owned));
        // 手动刷新用于更新订阅缓存；是否参与运行配置合并仍只由 enabled 状态决定。
        if allow_disabled_source {
            pipeline
                .update_inactive_cache(source)
                .await
                .map_err(subscription_pipeline_error)?;
        } else {
            pipeline
                .update(source)
                .await
                .map_err(subscription_pipeline_error)?;
        }
        self.load_projection()
    }

    pub fn save_source(
        &self,
        source: SubscriptionSource,
    ) -> AppResult<SubscriptionStateProjection> {
        self.store.save_source(source)?;
        self.load_projection()
    }

    pub fn reorder_sources(
        &self,
        ordered_ids: &[String],
    ) -> AppResult<SubscriptionStateProjection> {
        self.store.reorder_sources(ordered_ids)?;
        self.load_projection()
    }

    pub fn cached_yaml(&self, subscription_id: &str) -> AppResult<String> {
        let Some(bytes) = self.store.read_cached_content(subscription_id)? else {
            return Ok("# 当前订阅还没有可查看的缓存 YAML\n".to_string());
        };
        String::from_utf8(bytes).map_err(|error| {
            ConfigError::Subscription(format!("订阅缓存不是有效 UTF-8 YAML: {error}")).into()
        })
    }

    pub fn due_sources_at(&self, now: SubscriptionTimestamp) -> AppResult<Vec<SubscriptionSource>> {
        let sources = self.store.load_sources()?;
        let caches = self
            .store
            .load_cache_metadata()?
            .into_iter()
            .map(|cache| (cache.subscription_id.clone(), cache))
            .collect::<BTreeMap<_, _>>();

        // 定时更新只处理远程且启用的订阅；本地导入 YAML 没有远程拉取入口，不能被后台任务隐式改写。
        Ok(sources
            .into_iter()
            .filter(|source| source.enabled)
            .filter(|source| matches!(source.source_kind, SubscriptionSourceKind::Remote))
            .filter(|source| source.url.is_some())
            .filter(|source| {
                let Some(interval_secs) = source.update_interval_secs.filter(|secs| *secs > 0)
                else {
                    return false;
                };
                let interval_ms = interval_secs.saturating_mul(1000);
                let last_checked = caches
                    .get(&source.id)
                    .and_then(|cache| cache.last_update.as_ref())
                    .map(|result| result.checked_at);
                match last_checked {
                    Some(checked_at) => now.saturating_sub(checked_at) >= interval_ms,
                    None => true,
                }
            })
            .collect())
    }

    pub fn due_sources(&self) -> AppResult<Vec<SubscriptionSource>> {
        Ok(self.due_sources_at(now_timestamp())?)
    }

    pub fn import_file(&self, path: &Path) -> AppResult<SubscriptionStateProjection> {
        validate_yaml_file_for_subscription(path)?;
        let name = path
            .file_stem()
            .and_then(|name| name.to_str())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("本地订阅");
        self.store.import_local_subscription_file(name, path)?;
        self.load_projection()
    }

    pub fn select(&self, subscription_id: &str) -> AppResult<SubscriptionStateProjection> {
        // “选中订阅”在 app 层被定义为切换当前参与运行配置合并的订阅源。
        // 仓储只持久化 enabled 状态，缓存正文不在这里读写，避免 UI 选择动作破坏订阅缓存。
        self.store.select_source(subscription_id)?;
        self.load_projection()
    }

    pub fn delete(&self, subscription_id: &str) -> AppResult<SubscriptionStateProjection> {
        self.store.remove_source(subscription_id)?;
        self.load_projection()
    }

    pub fn mark_canceled(&self, subscription_id: &str) -> AppResult<SubscriptionStateProjection> {
        self.store.record_update_result(
            subscription_id,
            SubscriptionUpdateResult::canceled(now_timestamp(), "订阅更新已取消"),
            None,
        )?;
        self.load_projection()
    }

    fn projection_from_parts(
        &self,
        sources: Vec<SubscriptionSource>,
        caches: Vec<SubscriptionCacheMetadata>,
    ) -> SubscriptionStateProjection {
        let parser = air_mihomo::subscriptions::SubscriptionParser::default();
        let mut cache_map = BTreeMap::new();
        let mut parse_diagnostics = BTreeMap::new();
        let mut parsed_proxy_counts = BTreeMap::new();

        for cache in caches {
            let id = cache.subscription_id.clone();
            if let Some((count, diagnostics)) = self.parse_cached_content(&parser, &id, &cache) {
                parsed_proxy_counts.insert(id.clone(), count);
                if !diagnostics.is_empty() {
                    parse_diagnostics.insert(id.clone(), diagnostics);
                }
            }
            cache_map.insert(id, cache);
        }

        SubscriptionStateProjection {
            active_subscription_id: sources
                .iter()
                .find(|source| source.enabled)
                .or_else(|| sources.first())
                .map(|source| source.id.clone()),
            sources,
            caches: cache_map,
            parse_diagnostics,
            parsed_proxy_counts,
        }
    }

    fn parse_cached_content(
        &self,
        parser: &air_mihomo::subscriptions::SubscriptionParser,
        id: &str,
        cache: &SubscriptionCacheMetadata,
    ) -> Option<(usize, Vec<SubscriptionDiagnostic>)> {
        let Some(bytes) = self.store.read_cached_content(id).ok().flatten() else {
            return None;
        };
        let source = match String::from_utf8(bytes) {
            Ok(source) => source,
            Err(error) => {
                return Some((
                    0,
                    vec![SubscriptionDiagnostic::error(
                        "cache-utf8",
                        format!("订阅缓存不是有效 UTF-8: {error}"),
                    )],
                ));
            }
        };
        match parser.parse(&source) {
            Ok(ParsedSubscription {
                proxies,
                diagnostics,
                ..
            }) => {
                let mut diagnostics = diagnostics;
                if matches!(
                    cache.last_update.as_ref().map(|result| result.outcome),
                    Some(SubscriptionUpdateOutcome::Failed)
                ) && cache.last_success_at.is_some()
                {
                    diagnostics.push(SubscriptionDiagnostic::warning(
                        "using-stale-cache",
                        "最近更新失败，页面继续使用上次成功缓存",
                    ));
                }
                Some((proxies.len(), diagnostics))
            }
            Err(error) => Some((0, error.diagnostics())),
        }
    }

    fn remote_source(
        &self,
        subscription_id: &str,
        url: &str,
        core_version: Option<&str>,
    ) -> AppResult<SubscriptionSource> {
        let id = subscription_id.trim();
        if id.is_empty() {
            return Err(ConfigError::Subscription("订阅源 id 不能为空".into()).into());
        }
        if self
            .store
            .load_sources()?
            .iter()
            .any(|source| source.id == id)
        {
            return Err(ConfigError::Subscription(format!("订阅源已存在: {id}")).into());
        }

        let parsed = url::Url::parse(url.trim()).map_err(|error| {
            ConfigError::Subscription(format!(
                "订阅 URL 无效 {}: {error}",
                redact_log_value(url.trim())
            ))
        })?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err(ConfigError::Subscription("订阅 URL 必须是 http/https 地址".into()).into());
        }

        let mut source =
            SubscriptionSource::remote(id, import_name_from_url(&parsed), parsed.as_str());
        source.user_agent = Some(subscription_download_user_agent(core_version));
        source.proxy = Some("DIRECT".to_string());
        source.request_headers = SubscriptionRequestHeaders::default();
        source.source_kind = SubscriptionSourceKind::Remote;
        source
            .validate()
            .map_err(|error| ConfigError::Subscription(error.to_string()))?;
        Ok(source)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SubscriptionStateProjection {
    #[serde(default)]
    pub active_subscription_id: Option<String>,
    pub sources: Vec<SubscriptionSource>,
    pub caches: BTreeMap<String, SubscriptionCacheMetadata>,
    pub parse_diagnostics: BTreeMap<String, Vec<SubscriptionDiagnostic>>,
    pub parsed_proxy_counts: BTreeMap<String, usize>,
}

#[derive(Clone, Default)]
struct StagingSubscriptionStore {
    metadata: Arc<Mutex<HashMap<String, SubscriptionCacheMetadata>>>,
    content: Arc<Mutex<Option<Vec<u8>>>>,
}

impl StagingSubscriptionStore {
    fn content(&self) -> Option<Vec<u8>> {
        self.content
            .lock()
            .expect("staging subscription content lock should not be poisoned")
            .clone()
    }
}

#[async_trait]
impl SubscriptionUpdateCacheStore for StagingSubscriptionStore {
    async fn load_metadata(
        &self,
        subscription_id: &str,
    ) -> Result<Option<SubscriptionCacheMetadata>, SubscriptionPipelineError> {
        Ok(self
            .metadata
            .lock()
            .expect("staging subscription metadata lock should not be poisoned")
            .get(subscription_id)
            .cloned())
    }

    async fn read_cached_content(
        &self,
        _subscription_id: &str,
    ) -> Result<Option<Vec<u8>>, SubscriptionPipelineError> {
        Ok(None)
    }

    async fn record_update(
        &self,
        subscription_id: &str,
        result: SubscriptionUpdateResult,
        content: Option<&[u8]>,
    ) -> Result<SubscriptionCacheMetadata, SubscriptionPipelineError> {
        if let Some(content) = content {
            *self
                .content
                .lock()
                .expect("staging subscription content lock should not be poisoned") =
                Some(content.to_vec());
        }
        let mut metadata = self
            .metadata
            .lock()
            .expect("staging subscription metadata lock should not be poisoned")
            .remove(subscription_id)
            .unwrap_or_else(|| SubscriptionCacheMetadata::new(subscription_id));
        metadata.apply_update(result);
        self.metadata
            .lock()
            .expect("staging subscription metadata lock should not be poisoned")
            .insert(subscription_id.to_string(), metadata.clone());
        Ok(metadata)
    }
}

fn validate_yaml_file_for_subscription(path: &Path) -> AppResult<()> {
    if !is_yaml_path(path) {
        return Err(ConfigError::Subscription("只能导入 .yaml 或 .yml 订阅文件".into()).into());
    }
    let source = fs::read_to_string(path).map_err(|error| {
        ConfigError::Subscription(format!(
            "读取 YAML 文件失败: {}",
            redact_log_value(&error.to_string())
        ))
    })?;
    let document = ConfigDocument::parse(source)
        .map_err(|error| ConfigError::Subscription(redact_log_value(&error.to_string())))?;
    // 文件导入被明确限定为订阅缓存，不会导入为 profile；profile 导入继续走 LoadProfile。
    // 这里复用订阅解析器收集 base64 预留、YAML schema 等诊断，缓存写入仍由 SubscriptionStore 负责。
    air_mihomo::subscriptions::SubscriptionParser::default()
        .parse(document.source.as_str())
        .map_err(subscription_pipeline_error)?;
    Ok(())
}

fn subscription_pipeline_error(error: SubscriptionPipelineError) -> air_error::AppError {
    let diagnostics = error.diagnostics();
    let message = diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| error.to_string());
    ConfigError::Subscription(redact_log_value(&message)).into()
}

fn is_yaml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "yaml" | "yml"))
        .unwrap_or(false)
}

fn import_name_from_url(url: &url::Url) -> String {
    url.host_str()
        .map(|host| host.trim_start_matches("www.").to_string())
        .filter(|host| !host.is_empty())
        .unwrap_or_else(|| "远程订阅".to_string())
}

fn subscription_download_user_agent(core_version: Option<&str>) -> String {
    let version = core_version
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .map(|version| version.trim_start_matches('v'))
        .filter(|version| !version.is_empty())
        .unwrap_or("0.0.0");
    format!("clash.meta/v{version}")
}

fn now_timestamp() -> SubscriptionTimestamp {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as SubscriptionTimestamp
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;
    use air_storage::AppPaths;

    fn controller_in_temp() -> (tempfile::TempDir, SubscriptionController) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_base_dirs(
            &temp.path().join("config"),
            &temp.path().join("data"),
            &temp.path().join("cache"),
        );
        paths.init().unwrap();
        (
            temp,
            SubscriptionController::new(SubscriptionStore::new(paths)),
        )
    }

    fn yaml_subscription() -> &'static str {
        r#"
proxies:
  - name: alpha
    type: ss
    server: example.com
    port: 8388
    cipher: aes-128-gcm
    password: keep-secret
"#
    }

    struct OneShotServer {
        url: String,
    }

    impl OneShotServer {
        fn spawn(status: u16, body: &'static str) -> Self {
            Self::spawn_with_headers(status, Vec::new(), body)
        }

        fn spawn_with_headers(
            status: u16,
            headers: Vec<(&'static str, &'static str)>,
            body: &'static str,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("fake server should bind");
            let addr = listener
                .local_addr()
                .expect("fake server addr should exist");
            thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("fake request should arrive");
                let mut buffer = [0_u8; 1024];
                let _ = stream.read(&mut buffer);
                let mut response_headers = format!(
                    "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nConnection: close\r\n",
                    status,
                    body.len()
                );
                for (name, value) in headers {
                    response_headers.push_str(&format!("{name}: {value}\r\n"));
                }
                response_headers.push_str("\r\n");
                stream.write_all(response_headers.as_bytes()).unwrap();
                stream.write_all(body.as_bytes()).unwrap();
            });
            Self {
                url: format!("http://{addr}/sub.yaml?token=secret-token"),
            }
        }
    }

    #[tokio::test]
    async fn imports_remote_url_only_after_successful_download() {
        let (_temp, controller) = controller_in_temp();
        let server = OneShotServer::spawn_with_headers(
            200,
            vec![("Content-Disposition", "attachment;filename*=UTF-8''SSRDOG")],
            yaml_subscription(),
        );

        let projection = controller.import_url("sub-a", &server.url).await.unwrap();

        assert_eq!(projection.sources.len(), 1);
        assert_eq!(projection.sources[0].id, "sub-a");
        assert_eq!(projection.sources[0].name, "SSRDOG");
        assert_eq!(projection.parsed_proxy_counts.get("sub-a"), Some(&1));
        assert_eq!(
            projection
                .caches
                .get("sub-a")
                .and_then(|cache| cache.last_update.as_ref())
                .map(|result| result.outcome),
            Some(SubscriptionUpdateOutcome::Success)
        );
    }

    #[tokio::test]
    async fn failed_remote_import_does_not_create_index_entry() {
        let (_temp, controller) = controller_in_temp();
        let server = OneShotServer::spawn(200, "not: [valid");

        let error = controller
            .import_url("sub-a", &server.url)
            .await
            .expect_err("invalid YAML should fail");

        assert!(format!("{error}").contains("订阅源操作失败"));
        assert!(controller.load_projection().unwrap().sources.is_empty());
    }

    #[test]
    fn selects_active_subscription_for_merge_projection() {
        let (_temp, controller) = controller_in_temp();
        controller
            .store
            .save_source(SubscriptionSource::remote(
                "work",
                "Work",
                "https://example.test/sub",
            ))
            .unwrap();
        controller
            .store
            .save_source(SubscriptionSource::remote(
                "backup",
                "Backup",
                "https://backup.example.test/sub",
            ))
            .unwrap();

        let projection = controller.select("backup").unwrap();

        assert_eq!(projection.active_subscription_id.as_deref(), Some("backup"));
        assert!(
            !projection
                .sources
                .iter()
                .find(|source| source.id == "work")
                .unwrap()
                .enabled
        );
        assert!(
            projection
                .sources
                .iter()
                .find(|source| source.id == "backup")
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn imports_local_yaml_as_subscription_cache() {
        let (temp, controller) = controller_in_temp();
        let path = temp.path().join("local.yml");
        fs::write(&path, yaml_subscription()).unwrap();

        let projection = controller.import_file(&path).unwrap();

        assert_eq!(projection.sources.len(), 1);
        assert_eq!(projection.parsed_proxy_counts.values().next(), Some(&1));
        assert!(matches!(
            projection.sources[0].source_kind,
            SubscriptionSourceKind::LocalFile { .. }
        ));
    }

    #[test]
    fn due_sources_only_include_enabled_remote_sources_past_interval() {
        let (_temp, controller) = controller_in_temp();
        let mut due = SubscriptionSource::remote("due", "Due", "https://example.test/due");
        due.update_interval_secs = Some(3600);
        let mut fresh = SubscriptionSource::remote("fresh", "Fresh", "https://example.test/fresh");
        fresh.update_interval_secs = Some(3600);
        let mut disabled =
            SubscriptionSource::remote("disabled", "Disabled", "https://example.test/disabled");
        disabled.enabled = false;
        let local = SubscriptionSource::local_file("local", "Local", None);

        controller.store.save_source(due).unwrap();
        controller.store.save_source(fresh).unwrap();
        controller.store.save_source(disabled).unwrap();
        controller.store.save_source(local).unwrap();
        controller
            .store
            .record_update_result(
                "due",
                SubscriptionUpdateResult::success(1_000, 12),
                Some(yaml_subscription().as_bytes()),
            )
            .unwrap();
        controller
            .store
            .record_update_result(
                "fresh",
                SubscriptionUpdateResult::success(3_500_000, 12),
                Some(yaml_subscription().as_bytes()),
            )
            .unwrap();

        let due_sources = controller.due_sources_at(3_700_000).unwrap();

        assert_eq!(due_sources.len(), 1);
        assert_eq!(due_sources[0].id, "due");
    }
}
