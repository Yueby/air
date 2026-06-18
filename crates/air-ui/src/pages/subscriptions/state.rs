use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gpui::{Context, IntoElement, ParentElement, Render, ScrollHandle, Styled, Window, div};

use air_app::{AppCommand, SubscriptionStateProjection};
use air_mihomo::subscriptions::{
    SubscriptionCacheMetadata, SubscriptionDiagnostic, SubscriptionRequestHeaders,
    SubscriptionSource, SubscriptionSourceKind,
};
#[cfg(test)]
use air_mihomo::subscriptions::{SubscriptionUpdateOutcome, SubscriptionUpdateResult};
use air_telemetry::redaction::redact_log_value;

use super::format::*;
use super::render::*;

#[derive(Clone, Debug)]
pub struct SubscriptionPageState {
    pub(crate) sources: Vec<SubscriptionSource>,
    caches: BTreeMap<String, SubscriptionCacheMetadata>,
    parse_diagnostics: BTreeMap<String, Vec<SubscriptionDiagnostic>>,
    pub(crate) parsed_proxy_counts: BTreeMap<String, usize>,
    pub(crate) selected_id: Option<String>,
    pub(crate) updating_id: Option<String>,
    pub(crate) import_url: String,
    pub(crate) import_status: SubscriptionImportStatus,
    pub(crate) modal: SubscriptionModalState,
    form: SubscriptionFormState,
    config_form: SubscriptionConfigFormState,
    yaml_preview_subscription_id: Option<String>,
    yaml_preview_loading: bool,
    yaml_preview_contents: String,
    pub(crate) notice: Option<SubscriptionNotice>,
    pub(super) card_scroll_handle: ScrollHandle,
}

#[derive(Clone, Debug)]
pub(super) struct SubscriptionCardDrag {
    pub(super) id: String,
}

impl Render for SubscriptionCardDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .shadow_md()
            .opacity(0.94)
            .child(self.id.clone())
    }
}

impl SubscriptionPageState {
    pub fn empty() -> Self {
        Self::from_projection(SubscriptionStateProjection::default())
    }

    pub fn from_projection(projection: SubscriptionStateProjection) -> Self {
        let selected_id = projection
            .active_subscription_id
            .clone()
            .filter(|id| projection.sources.iter().any(|source| source.id == *id))
            .or_else(|| projection.sources.first().map(|source| source.id.clone()));
        let form = selected_id
            .as_deref()
            .and_then(|id| projection.sources.iter().find(|source| source.id == id))
            .map(SubscriptionFormState::from_source)
            .unwrap_or_default();

        Self {
            sources: projection.sources,
            caches: projection.caches,
            parse_diagnostics: projection.parse_diagnostics,
            parsed_proxy_counts: projection.parsed_proxy_counts,
            selected_id,
            updating_id: None,
            import_url: String::new(),
            import_status: SubscriptionImportStatus::Idle,
            modal: SubscriptionModalState::None,
            form,
            config_form: SubscriptionConfigFormState::default(),
            yaml_preview_subscription_id: None,
            yaml_preview_loading: false,
            yaml_preview_contents: String::new(),
            notice: None,
            card_scroll_handle: ScrollHandle::default(),
        }
    }

    pub fn apply_projection(&mut self, projection: SubscriptionStateProjection) {
        let previous_selected = self.selected_id.clone();
        let active_id = projection.active_subscription_id.clone();
        self.sources = projection.sources;
        self.caches = projection.caches;
        self.parse_diagnostics = projection.parse_diagnostics;
        self.parsed_proxy_counts = projection.parsed_proxy_counts;
        self.selected_id = active_id
            .filter(|id| self.sources.iter().any(|source| source.id == *id))
            .or_else(|| {
                previous_selected.filter(|id| self.sources.iter().any(|source| source.id == *id))
            })
            .or_else(|| {
                self.sources
                    .iter()
                    .find(|source| source.enabled)
                    .or_else(|| self.sources.first())
                    .map(|source| source.id.clone())
            });
        if let Some(source) = self.selected_source() {
            self.form = SubscriptionFormState::from_source(source);
        }
        self.updating_id = None;
        self.import_status = SubscriptionImportStatus::Idle;
    }

    pub fn mark_update_canceled(&mut self, subscription_id: &str) {
        if self.updating_id.as_deref() == Some(subscription_id) {
            self.updating_id = None;
        }
        self.notice = Some(SubscriptionNotice::warning("订阅更新已取消"));
    }

    pub fn apply_user_error(&mut self, message: impl AsRef<str>) {
        self.updating_id = None;
        if self.import_status == SubscriptionImportStatus::Importing {
            self.import_status = SubscriptionImportStatus::Failed;
        }
        self.notice = Some(SubscriptionNotice::error(redact_log_value(
            message.as_ref(),
        )));
    }

    #[cfg(test)]
    pub fn fake_for_test() -> Self {
        let mut sources = vec![
            SubscriptionSource::remote(
                "work",
                "Work",
                "https://example.test/sub.yaml?token=secret-token",
            ),
            SubscriptionSource::remote("backup", "Backup", "https://backup.example.test/clash"),
            SubscriptionSource::remote(
                "base64",
                "Reserved Base64",
                "https://nodes.example.test/sub",
            ),
        ];
        sources[1].enabled = false;

        let mut caches = BTreeMap::new();
        let mut work = SubscriptionCacheMetadata::new("work");
        work.content_path = Some("work.yaml".into());
        work.last_success_at = Some(1_779_403_200_000);
        work.last_update = Some(SubscriptionUpdateResult {
            checked_at: 1_779_403_200_000,
            outcome: SubscriptionUpdateOutcome::Success,
            status_code: Some(200),
            bytes: Some(42_120),
            etag: Some("\"work-a\"".to_string()),
            last_modified: Some("Thu, 21 May 2026 08:00:00 GMT".to_string()),
            user_info: None,
            message: None,
        });
        caches.insert("work".to_string(), work);

        let mut backup = SubscriptionCacheMetadata::new("backup");
        backup.content_path = Some("backup.yaml".into());
        backup.last_success_at = Some(1_779_316_800_000);
        backup.last_failure_at = Some(1_779_489_600_000);
        backup.last_update = Some(SubscriptionUpdateResult::failed(
            1_779_489_600_000,
            "订阅下载失败: timeout reading https://backup.example.test/clash?token=secret",
        ));
        caches.insert("backup".to_string(), backup);

        let mut base64 = SubscriptionCacheMetadata::new("base64");
        base64.last_failure_at = Some(1_779_500_000_000);
        base64.last_update = Some(SubscriptionUpdateResult::failed(
            1_779_500_000_000,
            "base64 节点订阅解析接口已预留，当前版本尚未实现转换",
        ));
        caches.insert("base64".to_string(), base64);

        let parse_diagnostics = BTreeMap::from([
            (
                "backup".to_string(),
                vec![SubscriptionDiagnostic::warning(
                    "using-stale-cache",
                    "最近更新失败，页面继续使用上次成功缓存",
                )],
            ),
            (
                "base64".to_string(),
                vec![SubscriptionDiagnostic::error(
                    "base64-parser-reserved",
                    "base64 节点订阅解析接口已预留，当前版本尚未实现转换",
                )],
            ),
        ]);
        let parsed_proxy_counts = BTreeMap::from([
            ("work".to_string(), 38),
            ("backup".to_string(), 21),
            ("base64".to_string(), 0),
        ]);

        let selected_id = sources.first().map(|source| source.id.clone());
        let form = selected_id
            .as_deref()
            .and_then(|id| sources.iter().find(|source| source.id == id))
            .map(SubscriptionFormState::from_source)
            .unwrap_or_default();

        Self {
            sources,
            caches,
            parse_diagnostics,
            parsed_proxy_counts,
            selected_id,
            updating_id: None,
            import_url: String::new(),
            import_status: SubscriptionImportStatus::Idle,
            modal: SubscriptionModalState::None,
            form,
            config_form: SubscriptionConfigFormState::default(),
            yaml_preview_subscription_id: None,
            yaml_preview_loading: false,
            yaml_preview_contents: String::new(),
            notice: None,
            card_scroll_handle: ScrollHandle::default(),
        }
    }

    #[cfg(test)]
    pub fn bump_cache_checked_at_for_test(&mut self, subscription_id: &str, delta_ms: u64) {
        // 测试只需要制造“同一订阅出现新一轮诊断”的时间变化，不暴露生产路径的内部缓存可变引用。
        if let Some(cache) = self.caches.get_mut(subscription_id)
            && let Some(last_update) = cache.last_update.as_mut()
        {
            last_update.checked_at += delta_ms;
        }
    }

    pub fn begin_add(&mut self) -> SubscriptionFormState {
        self.modal = SubscriptionModalState::Add;
        self.form = SubscriptionFormState {
            id: next_source_id(&self.sources),
            enabled: true,
            proxy: "DIRECT".to_string(),
            ..SubscriptionFormState::default()
        };
        self.form.clone()
    }

    pub fn begin_edit_selected(&mut self) -> SubscriptionFormState {
        let Some(source) = self.selected_source().cloned() else {
            self.notice = Some(SubscriptionNotice::error("请先选择要编辑的订阅源"));
            return self.form.clone();
        };
        self.modal = SubscriptionModalState::EditSubscription;
        self.form = SubscriptionFormState::from_source(&source);
        self.yaml_preview_subscription_id = Some(source.id.clone());
        self.yaml_preview_loading = true;
        self.yaml_preview_contents = "# 正在读取订阅缓存\n".to_string();
        self.form.clone()
    }

    pub fn begin_edit_by_id(&mut self, id: impl AsRef<str>) -> Option<SubscriptionFormState> {
        let id = id.as_ref();
        let Some(source) = self.sources.iter().find(|source| source.id == id).cloned() else {
            self.notice = Some(SubscriptionNotice::error("订阅源不存在"));
            return None;
        };
        self.modal = SubscriptionModalState::EditSubscription;
        self.form = SubscriptionFormState::from_source(&source);
        self.yaml_preview_subscription_id = Some(source.id.clone());
        self.yaml_preview_loading = true;
        self.yaml_preview_contents = "# 正在读取订阅缓存\n".to_string();
        Some(self.form.clone())
    }

    pub fn apply_yaml_preview(&mut self, subscription_id: &str, contents: String) -> bool {
        if self.yaml_preview_subscription_id.as_deref() == Some(subscription_id) {
            self.yaml_preview_loading = false;
            self.yaml_preview_contents = contents;
            true
        } else {
            false
        }
    }

    pub fn yaml_preview_contents(&self) -> String {
        self.yaml_preview_contents.clone()
    }

    pub fn take_notice(&mut self) -> Option<SubscriptionNotice> {
        self.notice.take()
    }

    pub fn begin_edit_config_selected(&mut self) -> SubscriptionConfigFormState {
        let Some(item) = self.selected_item_model() else {
            self.notice = Some(SubscriptionNotice::error("请先选择要编辑配置的订阅源"));
            return self.config_form.clone();
        };
        // 配置编辑弹窗当前只修改订阅 UI 元数据，真实 YAML 内容仍通过仓储/命令导入；
        // 这样不会在 UI 层反序列化再写回用户手写 YAML，从边界上避免破坏未知字段和注释排版。
        self.modal = SubscriptionModalState::EditConfig;
        self.config_form = SubscriptionConfigFormState::from_item(&item);
        self.config_form.clone()
    }

    pub fn select(&mut self, id: impl Into<String>) -> Option<AppCommand> {
        let id = id.into();
        if self.selected_id.as_deref() == Some(id.as_str()) {
            return None;
        }
        if self.sources.iter().any(|source| source.id == id) {
            self.selected_id = Some(id.clone());
            self.notice = None;
            Some(AppCommand::SelectSubscription {
                subscription_id: id,
            })
        } else {
            None
        }
    }

    pub fn close_modal(&mut self) {
        self.modal = SubscriptionModalState::None;
        self.yaml_preview_loading = false;
    }

    pub fn update_import_url(&mut self, value: impl Into<String>) {
        self.import_url = value.into();
        if self.import_status == SubscriptionImportStatus::Failed {
            self.import_status = SubscriptionImportStatus::Idle;
        }
        self.notice = None;
    }

    pub fn update_form_field(&mut self, field: SubscriptionFormField, value: impl Into<String>) {
        let value = value.into();
        match field {
            SubscriptionFormField::Name => self.form.name = value,
            SubscriptionFormField::Url => self.form.url = value,
            SubscriptionFormField::IntervalHours => self.form.interval_hours = value,
            SubscriptionFormField::UserAgent => self.form.user_agent = value,
            SubscriptionFormField::Proxy => self.form.proxy = value,
            SubscriptionFormField::RequestHeaders => self.form.request_headers = value,
            SubscriptionFormField::ImportUrl => self.update_import_url(value),
        }
    }

    pub fn update_config_form_field(
        &mut self,
        field: SubscriptionConfigFormField,
        value: impl Into<String>,
    ) {
        let value = value.into();
        match field {
            SubscriptionConfigFormField::Name => self.config_form.name = value,
            SubscriptionConfigFormField::IntervalHours => self.config_form.interval_hours = value,
            SubscriptionConfigFormField::ProxyCount => self.config_form.proxy_count = value,
            SubscriptionConfigFormField::UsageUsedGb => self.config_form.usage_used_gb = value,
            SubscriptionConfigFormField::UsageTotalGb => self.config_form.usage_total_gb = value,
        }
    }

    pub fn save_form(&mut self) -> Option<AppCommand> {
        let source = match self.modal {
            SubscriptionModalState::Add => {
                let Ok(source) = self.form.to_source() else {
                    self.notice = Some(SubscriptionNotice::error("订阅名称和 URL 不能为空"));
                    return None;
                };
                if self.sources.iter().any(|item| item.id == source.id) {
                    self.notice = Some(SubscriptionNotice::error("订阅源 id 已存在"));
                    return None;
                }
                self.selected_id = Some(source.id.clone());
                source
            }
            SubscriptionModalState::EditSubscription => {
                let Some(existing) = self.sources.iter().find(|item| item.id == self.form.id)
                else {
                    self.notice = Some(SubscriptionNotice::error("订阅源不存在"));
                    return None;
                };
                let Ok(source) = self.form.to_source_with_existing(existing) else {
                    self.notice = Some(SubscriptionNotice::error(
                        "订阅名称不能为空，远程订阅 URL 不能为空",
                    ));
                    return None;
                };
                source
            }
            SubscriptionModalState::None | SubscriptionModalState::EditConfig => return None,
        };
        self.modal = SubscriptionModalState::None;
        self.notice = Some(SubscriptionNotice::success("已提交订阅保存"));
        Some(AppCommand::SaveSubscriptionSource { source })
    }

    pub fn save_config_form(&mut self) {
        if self.modal != SubscriptionModalState::EditConfig {
            return;
        }
        let Some(id) = self.selected_id.clone() else {
            self.notice = Some(SubscriptionNotice::error("请先选择要保存的订阅源"));
            return;
        };
        let name = self.config_form.name.trim();
        if name.is_empty() {
            self.notice = Some(SubscriptionNotice::error("配置名称不能为空"));
            return;
        }

        if let Some(source) = self.sources.iter_mut().find(|source| source.id == id) {
            source.name = name.to_string();
            source.update_interval_secs = parse_positive_u64(&self.config_form.interval_hours)
                .map(|hours| hours.saturating_mul(3600));
        }
        if let Some(count) = parse_positive_usize(&self.config_form.proxy_count) {
            self.parsed_proxy_counts.insert(id, count);
        }
        self.notice = Some(SubscriptionNotice::success(
            "已更新订阅展示配置，原始 YAML 内容保持不变",
        ));
        self.modal = SubscriptionModalState::None;
    }

    pub fn toggle_selected_enabled(&mut self) {
        let Some(id) = self.selected_id.clone() else {
            return;
        };
        if let Some(source) = self.sources.iter_mut().find(|source| source.id == id) {
            source.enabled = !source.enabled;
            self.form.enabled = source.enabled;
            self.notice = Some(SubscriptionNotice::success(if source.enabled {
                "订阅源已启用"
            } else {
                "订阅源已禁用"
            }));
        }
    }

    pub fn delete_selected(&mut self) -> Option<AppCommand> {
        let Some(id) = self.selected_id.clone() else {
            self.notice = Some(SubscriptionNotice::error("请先选择要删除的订阅源"));
            return None;
        };
        self.sources.retain(|source| source.id != id);
        self.caches.remove(&id);
        self.parse_diagnostics.remove(&id);
        self.parsed_proxy_counts.remove(&id);
        self.selected_id = self.sources.first().map(|source| source.id.clone());
        self.notice = Some(SubscriptionNotice::success("已删除订阅源及其缓存状态"));
        Some(AppCommand::DeleteSubscription {
            subscription_id: id,
        })
    }

    pub fn delete_by_id(&mut self, id: impl Into<String>) -> Option<AppCommand> {
        let id = id.into();
        if !self.sources.iter().any(|source| source.id == id) {
            return None;
        }
        self.sources.retain(|source| source.id != id);
        self.caches.remove(&id);
        self.parse_diagnostics.remove(&id);
        self.parsed_proxy_counts.remove(&id);
        if self.selected_id.as_deref() == Some(id.as_str()) {
            self.selected_id = self.sources.first().map(|source| source.id.clone());
        }
        self.notice = Some(SubscriptionNotice::success("已删除订阅源及其缓存状态"));
        Some(AppCommand::DeleteSubscription {
            subscription_id: id,
        })
    }

    pub fn import_url(&mut self) -> Option<AppCommand> {
        let url = self.import_url.trim().to_string();
        if !is_valid_subscription_url(&url) {
            self.import_status = SubscriptionImportStatus::Failed;
            self.notice = Some(SubscriptionNotice::error(
                "请输入 http 或 https 开头的有效订阅链接",
            ));
            return None;
        }

        let id = next_source_id(&self.sources);
        let command_url = url.clone();
        self.import_url.clear();
        self.import_status = SubscriptionImportStatus::Importing;
        self.notice = Some(SubscriptionNotice::success("已提交订阅链接导入"));
        Some(AppCommand::ImportSubscriptionUrl {
            subscription_id: id,
            url: command_url,
        })
    }

    pub fn validate_yaml_file_selection(&self, path: &Path) -> SubscriptionYamlImportValidation {
        validate_yaml_file_selection(path)
    }

    pub fn import_yaml_file(&mut self, path: PathBuf) -> Option<AppCommand> {
        let validation = self.validate_yaml_file_selection(&path);
        if !validation.accepted {
            self.import_status = SubscriptionImportStatus::Failed;
            self.notice = Some(SubscriptionNotice::error(validation.message));
            return None;
        }
        self.import_status = SubscriptionImportStatus::Importing;
        self.notice = Some(SubscriptionNotice::success("已提交 YAML 文件导入"));
        Some(AppCommand::ImportSubscriptionFile { path })
    }

    pub fn update_selected(&mut self) -> Option<AppCommand> {
        let source = self.selected_source()?.clone();
        self.updating_id = Some(source.id.clone());
        self.notice = Some(SubscriptionNotice::success("已派发后台更新命令"));
        Some(AppCommand::UpdateSubscription {
            subscription_id: source.id,
        })
    }

    pub fn update_by_id(&mut self, id: impl Into<String>) -> Option<AppCommand> {
        let id = id.into();
        let Some(source) = self.sources.iter().find(|source| source.id == id).cloned() else {
            return None;
        };
        self.updating_id = Some(source.id.clone());
        self.notice = Some(SubscriptionNotice::success("已派发后台更新命令"));
        Some(AppCommand::UpdateSubscription {
            subscription_id: source.id,
        })
    }

    pub fn reorder_before(&mut self, dragged_id: &str, target_id: &str) -> Option<AppCommand> {
        if dragged_id == target_id {
            return None;
        }
        let from = self
            .sources
            .iter()
            .position(|source| source.id == dragged_id)?;
        let to = self
            .sources
            .iter()
            .position(|source| source.id == target_id)?;
        let source = self.sources.remove(from);
        let insert_at = if from < to { to.saturating_sub(1) } else { to };
        self.sources.insert(insert_at, source);
        let ordered_ids = self
            .sources
            .iter()
            .map(|source| source.id.clone())
            .collect::<Vec<_>>();
        self.notice = Some(SubscriptionNotice::success("已提交订阅排序"));
        Some(AppCommand::ReorderSubscriptions { ordered_ids })
    }

    pub fn cancel_update(&mut self) -> Option<AppCommand> {
        let id = self.updating_id.take()?;
        self.notice = Some(SubscriptionNotice::warning("已请求取消订阅更新"));
        Some(AppCommand::CancelSubscriptionUpdate {
            subscription_id: id,
        })
    }

    pub fn view_model(&self) -> SubscriptionPageViewModel {
        let items = self
            .sources
            .iter()
            .map(|source| {
                let cache = self.caches.get(&source.id);
                let diagnostics = self
                    .parse_diagnostics
                    .get(&source.id)
                    .cloned()
                    .unwrap_or_default();
                SubscriptionListItem::from_source(
                    source,
                    cache,
                    diagnostics,
                    self.parsed_proxy_counts
                        .get(&source.id)
                        .copied()
                        .unwrap_or(0),
                    self.selected_id.as_deref() == Some(source.id.as_str()),
                    self.updating_id.as_deref() == Some(source.id.as_str()),
                )
            })
            .collect::<Vec<_>>();
        let selected = items.iter().find(|item| item.selected).cloned();

        SubscriptionPageViewModel {
            items,
            selected,
            import_url: self.import_url.clone(),
            import_url_valid: is_valid_subscription_url(&self.import_url),
            import_status: self.import_status,
            modal: self.modal,
            form: self.form.clone(),
            config_form: self.config_form.clone(),
            yaml_preview_loading: self.yaml_preview_loading,
            notice: self.notice.clone(),
        }
    }

    pub(crate) fn selected_source(&self) -> Option<&SubscriptionSource> {
        let id = self.selected_id.as_deref()?;
        self.sources.iter().find(|source| source.id == id)
    }

    fn selected_item_model(&self) -> Option<SubscriptionListItem> {
        self.view_model().selected
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SubscriptionModalState {
    #[default]
    None,
    Add,
    EditSubscription,
    EditConfig,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SubscriptionImportStatus {
    #[default]
    Idle,
    Importing,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionFormField {
    Name,
    Url,
    IntervalHours,
    UserAgent,
    Proxy,
    RequestHeaders,
    ImportUrl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionConfigFormField {
    Name,
    IntervalHours,
    ProxyCount,
    UsageUsedGb,
    UsageTotalGb,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionFormState {
    pub id: String,
    pub name: String,
    pub url: String,
    pub interval_hours: String,
    pub user_agent: String,
    pub proxy: String,
    pub request_headers: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionConfigFormState {
    pub id: String,
    pub name: String,
    pub interval_hours: String,
    pub proxy_count: String,
    pub usage_used_gb: String,
    pub usage_total_gb: String,
    pub source_label: String,
}

impl Default for SubscriptionConfigFormState {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            interval_hours: "24".to_string(),
            proxy_count: "0".to_string(),
            usage_used_gb: "0".to_string(),
            usage_total_gb: "0".to_string(),
            source_label: String::new(),
        }
    }
}

impl SubscriptionConfigFormState {
    fn from_item(item: &SubscriptionListItem) -> Self {
        Self {
            id: item.id.clone(),
            name: item.name.clone(),
            interval_hours: "24".to_string(),
            proxy_count: item.node_count.to_string(),
            usage_used_gb: format!("{:.1}", item.usage.used_gb),
            usage_total_gb: format!("{:.1}", item.usage.total_gb),
            source_label: item.url_label.clone(),
        }
    }
}

impl Default for SubscriptionFormState {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            url: String::new(),
            interval_hours: "24".to_string(),
            user_agent: String::new(),
            proxy: "DIRECT".to_string(),
            request_headers: String::new(),
            enabled: true,
        }
    }
}

impl SubscriptionFormState {
    fn from_source(source: &SubscriptionSource) -> Self {
        Self {
            id: source.id.clone(),
            name: source.name.clone(),
            url: source
                .url
                .as_ref()
                .map(|url| url.as_str().to_string())
                .unwrap_or_default(),
            interval_hours: source
                .update_interval_secs
                .map(|secs| secs / 3600)
                .unwrap_or(0)
                .to_string(),
            user_agent: source.user_agent.clone().unwrap_or_default(),
            proxy: source.proxy.clone().unwrap_or_default(),
            request_headers: source
                .request_headers
                .iter()
                .map(|(name, value)| format!("{name}: {value}"))
                .collect::<Vec<_>>()
                .join("\n"),
            enabled: source.enabled,
        }
    }

    pub(crate) fn to_source(&self) -> Result<SubscriptionSource, ()> {
        let id = self.id.trim();
        let name = self.name.trim();
        let url = self.url.trim();
        if id.is_empty() || name.is_empty() || url.is_empty() {
            return Err(());
        }

        let hours = self.interval_hours.trim().parse::<u64>().unwrap_or(24);
        let mut source = SubscriptionSource::remote(id, name, url);
        source.update_interval_secs = (hours > 0).then_some(hours * 3600);
        source.user_agent = optional_text(&self.user_agent);
        source.proxy = optional_text(&self.proxy);
        source.request_headers =
            SubscriptionRequestHeaders::new(parse_headers(&self.request_headers));
        source.enabled = self.enabled;
        source.source_kind = SubscriptionSourceKind::Remote;
        source.validate().map_err(|_| ())?;
        Ok(source)
    }

    fn to_source_with_existing(
        &self,
        existing: &SubscriptionSource,
    ) -> Result<SubscriptionSource, ()> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(());
        }

        let mut source = existing.clone();
        source.name = name.to_string();
        source.update_interval_secs = parse_positive_u64(&self.interval_hours)
            .or(Some(24))
            .map(|hours| hours.saturating_mul(3600))
            .filter(|secs| *secs > 0);
        source.user_agent = optional_text(&self.user_agent);
        source.proxy = optional_text(&self.proxy);
        source.request_headers =
            SubscriptionRequestHeaders::new(parse_headers(&self.request_headers));
        source.enabled = self.enabled;
        if matches!(source.source_kind, SubscriptionSourceKind::Remote) {
            let url = self.url.trim();
            if url.is_empty() {
                return Err(());
            }
            source.url = Some(air_mihomo::subscriptions::SubscriptionUrl::new(url));
        }
        source.validate().map_err(|_| ())?;
        Ok(source)
    }
}

#[derive(Clone, Debug)]
pub struct SubscriptionPageViewModel {
    pub items: Vec<SubscriptionListItem>,
    pub selected: Option<SubscriptionListItem>,
    pub import_url: String,
    pub import_url_valid: bool,
    pub import_status: SubscriptionImportStatus,
    pub modal: SubscriptionModalState,
    pub form: SubscriptionFormState,
    pub config_form: SubscriptionConfigFormState,
    pub yaml_preview_loading: bool,
    pub notice: Option<SubscriptionNotice>,
}

#[derive(Clone, Debug)]
pub struct SubscriptionListItem {
    pub id: String,
    pub name: String,
    pub url_label: String,
    pub enabled: bool,
    pub updating: bool,
    pub selected: bool,
    pub node_count: usize,
    pub last_checked_at: Option<u64>,
    pub last_success: String,
    pub last_checked: String,
    pub last_checked_tooltip: String,
    pub last_error: Option<String>,
    pub cache_state: SubscriptionCacheState,
    pub cache_label: String,
    pub usage: SubscriptionUsageView,
    pub diagnostics: Vec<SubscriptionDiagnosticView>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubscriptionUsageView {
    pub used_gb: f32,
    pub total_gb: f32,
    pub percent: f32,
    pub label: String,
    pub expires_label: Option<String>,
    pub expires_tooltip: Option<String>,
}
