//! 规则集合 provider 的领域模型。
//!
//! 配置层的 `RuleProvider` 负责 YAML 往返和未知字段保留；本模块在其上补充
//! GUI 与服务层需要的语义：provider 类型、行为、运行态 API 映射、RULE-SET 引用校验，
//! 以及可替换的更新接口。规则文件下载仍由 mihomo 自身处理，这里不直接访问网络或文件系统。

use std::collections::{BTreeMap, BTreeSet};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use air_config::ConfigDiagnostic;
#[cfg(test)]
use air_config::ConfigDiagnosticSeverity;
use air_config::model::{
    MihomoConfigDocument, ProviderKind, RuleLine, RuleProvider as ConfigRuleProvider,
};
use air_error::AppResult;
use air_mihomo::client::MihomoHttpClient;
use air_mihomo::dto::ProvidersResponse;

/// mihomo rule-provider 支持的主要 behavior。
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuleProviderBehavior {
    Classical,
    Domain,
    IpCidr,
    Other(String),
}

impl RuleProviderBehavior {
    pub fn from_option(value: Option<&str>) -> Option<Self> {
        value.map(|value| match normalize_token(value).as_str() {
            "classical" => Self::Classical,
            "domain" => Self::Domain,
            "ipcidr" | "ip-cidr" => Self::IpCidr,
            _ => Self::Other(value.trim().to_string()),
        })
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Classical => "classical",
            Self::Domain => "domain",
            Self::IpCidr => "ipcidr",
            Self::Other(value) => value.as_str(),
        }
    }
}

/// 规则集合文件格式。`mrs` 只适用于 domain/ipcidr behavior，classical 仍应使用 yaml/text。
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuleProviderFormat {
    Yaml,
    Text,
    Mrs,
    Other(String),
}

impl RuleProviderFormat {
    pub fn from_option(value: Option<&str>) -> Option<Self> {
        value.map(|value| match normalize_token(value).as_str() {
            "yaml" => Self::Yaml,
            "text" => Self::Text,
            "mrs" => Self::Mrs,
            _ => Self::Other(value.trim().to_string()),
        })
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Yaml => "yaml",
            Self::Text => "text",
            Self::Mrs => "mrs",
            Self::Other(value) => value.as_str(),
        }
    }
}

/// 配置文件中的 rule-providers 集合，按名称索引。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleProviderCollection {
    pub providers: BTreeMap<String, RuleProviderSettings>,
}

impl RuleProviderCollection {
    pub fn from_document(document: &MihomoConfigDocument) -> Self {
        Self {
            providers: document
                .rule_providers
                .iter()
                .map(|(name, provider)| {
                    (
                        name.clone(),
                        RuleProviderSettings::from_config(name, provider),
                    )
                })
                .collect(),
        }
    }

    /// 将领域模型写回配置文档。`raw` 副本负责保留暂未建模的扩展字段。
    pub fn apply_to_document(&self, document: &mut MihomoConfigDocument) {
        document.rule_providers = self
            .providers
            .iter()
            .map(|(name, provider)| (name.clone(), provider.to_config()))
            .collect();
    }

    pub fn find(&self, name: &str) -> Option<&RuleProviderSettings> {
        self.providers.get(name)
    }

    pub fn find_mut(&mut self, name: &str) -> Option<&mut RuleProviderSettings> {
        self.providers.get_mut(name)
    }

    pub fn validate(&self, document: &MihomoConfigDocument) -> Vec<ConfigDiagnostic> {
        let mut validator = RuleProviderValidator::new(self, document);
        validator.validate();
        validator.diagnostics
    }

    pub fn referenced_by_rules(&self, document: &MihomoConfigDocument) -> Vec<RuleSetReference> {
        collect_rule_set_references(document)
    }
}

/// 单个 rule provider 的可编辑配置。
#[derive(Clone, PartialEq)]
pub struct RuleProviderSettings {
    pub name: String,
    pub kind: ProviderKind,
    pub behavior: Option<RuleProviderBehavior>,
    pub interval: Option<u64>,
    pub path: Option<String>,
    pub url: Option<String>,
    pub proxy: Option<String>,
    pub format: Option<RuleProviderFormat>,
    pub payload: Vec<String>,
    raw: ConfigRuleProvider,
}

impl RuleProviderSettings {
    pub fn from_config(name: &str, provider: &ConfigRuleProvider) -> Self {
        Self {
            name: name.to_string(),
            kind: provider.kind.clone(),
            behavior: RuleProviderBehavior::from_option(provider.behavior.as_deref()),
            interval: provider.interval,
            path: normalize_optional_string(provider.path.as_deref()),
            url: normalize_optional_string(provider.url.as_deref()),
            proxy: normalize_optional_string(provider.proxy.as_deref()),
            format: RuleProviderFormat::from_option(provider.format.as_deref()),
            payload: provider.payload.clone(),
            raw: provider.clone(),
        }
    }

    pub fn to_config(&self) -> ConfigRuleProvider {
        let mut provider = self.raw.clone();
        provider.kind = self.kind.clone();
        provider.behavior = self
            .behavior
            .as_ref()
            .map(|value| value.as_str().to_string());
        provider.interval = self.interval;
        provider.path = normalize_optional_string(self.path.as_deref());
        provider.url = normalize_optional_string(self.url.as_deref());
        provider.proxy = normalize_optional_string(self.proxy.as_deref());
        provider.format = self.format.as_ref().map(|value| value.as_str().to_string());
        provider.payload = self.payload.clone();
        provider
    }

    pub fn rename(&mut self, new_name: impl Into<String>) {
        self.name = new_name.into();
    }

    pub fn raw_provider(&self) -> ConfigRuleProvider {
        self.to_config()
    }
}

impl std::fmt::Debug for RuleProviderSettings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuleProviderSettings")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("behavior", &self.behavior)
            .field("interval", &self.interval)
            .field("path", &self.path)
            .field("url", &self.url.as_ref().map(|_| "<set>"))
            .field("proxy", &self.proxy)
            .field("format", &self.format)
            .field("payload", &self.payload)
            .field("raw", &"<preserved>")
            .finish()
    }
}

/// RULE-SET 对 provider 的引用位置。主规则和 sub-rules 都允许引用 rule-providers。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RuleSetReference {
    pub provider_name: String,
    pub scope: RuleProviderReferenceScope,
    pub rule_index: usize,
    pub path: String,
    pub raw: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuleProviderReferenceScope {
    Rules,
    SubRules(String),
}

/// `/providers/rules` 或 `/providers/rules/{name}` 返回的单个 provider 状态。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct RuleProviderRuntimeState {
    pub name: String,
    pub provider_type: Option<String>,
    pub vehicle_type: Option<String>,
    pub behavior: Option<String>,
    pub rule_count: Option<u64>,
    pub updated_at: Option<String>,
    pub raw: Value,
}

impl RuleProviderRuntimeState {
    pub fn from_api_value(name_hint: &str, value: Value) -> Self {
        Self {
            name: string_field(&value, &["name"]).unwrap_or_else(|| name_hint.to_string()),
            provider_type: string_field(&value, &["type", "providerType", "provider-type"]),
            vehicle_type: string_field(&value, &["vehicleType", "vehicle-type"]),
            behavior: string_field(&value, &["behavior"]),
            rule_count: u64_field(&value, &["ruleCount", "rule-count", "count"]),
            updated_at: string_field(&value, &["updatedAt", "updated-at"]),
            raw: value,
        }
    }
}

/// `/providers/rules` 响应的领域层视图。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct RuleProviderRuntimeCollection {
    pub providers: BTreeMap<String, RuleProviderRuntimeState>,
    pub extra: BTreeMap<String, Value>,
}

impl From<ProvidersResponse> for RuleProviderRuntimeCollection {
    fn from(response: ProvidersResponse) -> Self {
        Self {
            providers: response
                .providers
                .into_iter()
                .map(|(name, value)| {
                    let state = RuleProviderRuntimeState::from_api_value(&name, value);
                    (name, state)
                })
                .collect(),
            extra: response.extra,
        }
    }
}

/// rule provider API 的最小接口。领域服务依赖 trait，测试可直接注入 mock。
#[async_trait]
pub trait RuleProviderApi: Send + Sync {
    async fn rule_providers(&self) -> AppResult<ProvidersResponse>;
    async fn rule_provider(&self, name: &str) -> AppResult<Value>;
    async fn update_rule_provider(&self, name: &str) -> AppResult<()>;
}

#[async_trait]
impl RuleProviderApi for MihomoHttpClient {
    async fn rule_providers(&self) -> AppResult<ProvidersResponse> {
        MihomoHttpClient::rule_providers(self).await
    }

    async fn rule_provider(&self, name: &str) -> AppResult<Value> {
        MihomoHttpClient::rule_provider(self, name).await
    }

    async fn update_rule_provider(&self, name: &str) -> AppResult<()> {
        MihomoHttpClient::update_rule_provider(self, name).await
    }
}

/// rule provider 更新服务。这里只发起 mihomo 的更新命令，不下载或解析规则文件。
pub struct RuleProviderUpdateService<A> {
    api: A,
}

impl<A> RuleProviderUpdateService<A>
where
    A: RuleProviderApi,
{
    pub fn new(api: A) -> Self {
        Self { api }
    }

    pub async fn list_runtime(&self) -> AppResult<RuleProviderRuntimeCollection> {
        Ok(self.api.rule_providers().await?.into())
    }

    pub async fn runtime_state(&self, name: &str) -> AppResult<RuleProviderRuntimeState> {
        let value = self.api.rule_provider(name).await?;
        Ok(RuleProviderRuntimeState::from_api_value(name, value))
    }

    pub async fn update(&self, name: &str) -> AppResult<()> {
        self.api.update_rule_provider(name).await
    }
}

struct RuleProviderValidator<'a> {
    collection: &'a RuleProviderCollection,
    document: &'a MihomoConfigDocument,
    diagnostics: Vec<ConfigDiagnostic>,
}

impl<'a> RuleProviderValidator<'a> {
    fn new(collection: &'a RuleProviderCollection, document: &'a MihomoConfigDocument) -> Self {
        Self {
            collection,
            document,
            diagnostics: Vec::new(),
        }
    }

    fn validate(&mut self) {
        self.validate_provider_names_and_fields();
        self.validate_rule_set_references();
    }

    fn validate_provider_names_and_fields(&mut self) {
        for (name, provider) in &self.collection.providers {
            let base = format!("rule-providers.{name}");
            validate_provider_name(name, &mut self.diagnostics, &base);
            validate_provider_kind(provider, &mut self.diagnostics, &base);
            validate_provider_behavior(provider, &mut self.diagnostics, &base);
        }
    }

    fn validate_rule_set_references(&mut self) {
        let provider_names = self
            .collection
            .providers
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();

        for reference in collect_rule_set_references(self.document) {
            if !provider_names.contains(&reference.provider_name) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{}.provider", reference.path),
                    format!(
                        "RULE-SET 引用了不存在的 rule provider `{}`",
                        reference.provider_name
                    ),
                    Some(
                        "请新增对应的 rule-providers 条目，或修改 RULE-SET 的 provider 名称。"
                            .to_string(),
                    ),
                ));
            }
        }
    }
}

fn validate_provider_name(name: &str, diagnostics: &mut Vec<ConfigDiagnostic>, base: &str) {
    if name.trim().is_empty() {
        diagnostics.push(ConfigDiagnostic::error(
            format!("{base}.name"),
            "rule provider 名称不能为空",
            Some("请为 rule-providers 的键填写非空名称。".to_string()),
        ));
        return;
    }

    if name.trim() != name {
        diagnostics.push(ConfigDiagnostic::error(
            format!("{base}.name"),
            format!("rule provider 名称 `{name}` 不能包含首尾空白"),
            Some("请移除名称首尾空白，并同步修改 RULE-SET 引用。".to_string()),
        ));
    }

    if name.contains(',') || name.chars().any(char::is_control) {
        diagnostics.push(ConfigDiagnostic::error(
            format!("{base}.name"),
            format!("rule provider 名称 `{name}` 不能包含逗号或控制字符"),
            Some("RULE-SET 使用逗号分隔字段，请改用不含逗号的 provider 名称。".to_string()),
        ));
    }
}

fn validate_provider_kind(
    provider: &RuleProviderSettings,
    diagnostics: &mut Vec<ConfigDiagnostic>,
    base: &str,
) {
    match &provider.kind {
        ProviderKind::Http => {
            if provider
                .url
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                diagnostics.push(ConfigDiagnostic::error(
                    format!("{base}.url"),
                    "http rule provider 缺少 url",
                    Some("请填写远程规则集合 URL。".to_string()),
                ));
            }
        }
        ProviderKind::File => {
            if provider
                .path
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                diagnostics.push(ConfigDiagnostic::error(
                    format!("{base}.path"),
                    "file rule provider 缺少 path",
                    Some("请填写本地规则集合文件路径。".to_string()),
                ));
            }
        }
        ProviderKind::Inline => {
            if provider.payload.is_empty() {
                diagnostics.push(ConfigDiagnostic::warning(
                    format!("{base}.payload"),
                    "inline rule provider 没有 payload",
                    Some("请补充规则条目，或删除空的 inline provider。".to_string()),
                ));
            }
        }
        ProviderKind::Other(value) if value.trim().is_empty() => {
            diagnostics.push(ConfigDiagnostic::error(
                format!("{base}.type"),
                "rule provider 缺少 type",
                Some("可选 type: http、file、inline。".to_string()),
            ));
        }
        ProviderKind::Other(value) => diagnostics.push(ConfigDiagnostic::warning(
            format!("{base}.type"),
            format!("rule provider 使用了未识别类型 `{value}`"),
            Some("如果这是 mihomo 新增类型，可保留；否则请改为 http、file 或 inline。".to_string()),
        )),
    }
}

fn validate_provider_behavior(
    provider: &RuleProviderSettings,
    diagnostics: &mut Vec<ConfigDiagnostic>,
    base: &str,
) {
    if provider.behavior.is_none() {
        diagnostics.push(ConfigDiagnostic::warning(
            format!("{base}.behavior"),
            "rule provider 缺少 behavior",
            Some(
                "建议填写 classical、domain 或 ipcidr，以便 mihomo 正确解析规则集合。".to_string(),
            ),
        ));
    }

    if matches!(
        (&provider.behavior, &provider.format),
        (
            Some(RuleProviderBehavior::Classical),
            Some(RuleProviderFormat::Mrs)
        )
    ) {
        diagnostics.push(ConfigDiagnostic::error(
            format!("{base}.format"),
            "classical rule provider 不支持 mrs 格式",
            Some("请改用 yaml/text，或将 behavior 改为 domain/ipcidr。".to_string()),
        ));
    }

    if let Some(RuleProviderBehavior::Other(value)) = &provider.behavior {
        diagnostics.push(ConfigDiagnostic::warning(
            format!("{base}.behavior"),
            format!("rule provider 使用了未识别 behavior `{value}`"),
            Some(
                "如果这是 mihomo 新增 behavior，可保留；否则请改为 classical、domain 或 ipcidr。"
                    .to_string(),
            ),
        ));
    }

    if let Some(RuleProviderFormat::Other(value)) = &provider.format {
        diagnostics.push(ConfigDiagnostic::warning(
            format!("{base}.format"),
            format!("rule provider 使用了未识别 format `{value}`"),
            Some("如果这是 mihomo 新增格式，可保留；否则请改为 yaml、text 或 mrs。".to_string()),
        ));
    }
}

fn collect_rule_set_references(document: &MihomoConfigDocument) -> Vec<RuleSetReference> {
    let mut references = Vec::new();

    for (index, rule) in document.rules.iter().enumerate() {
        push_rule_set_reference(
            &mut references,
            RuleProviderReferenceScope::Rules,
            index,
            rule,
        );
    }

    for (sub_rule_name, rules) in &document.sub_rules {
        for (index, rule) in rules.iter().enumerate() {
            push_rule_set_reference(
                &mut references,
                RuleProviderReferenceScope::SubRules(sub_rule_name.clone()),
                index,
                rule,
            );
        }
    }

    references
}

fn push_rule_set_reference(
    references: &mut Vec<RuleSetReference>,
    scope: RuleProviderReferenceScope,
    rule_index: usize,
    rule: &RuleLine,
) {
    let parts = split_rule_segments(&rule.raw);
    if parts.len() < 2 {
        return;
    }

    let rule_type = normalize_rule_type(parts[0]);
    if !matches!(rule_type.as_str(), "RULE-SET" | "RULESET") {
        return;
    }

    let provider_name = parts[1].trim();
    if provider_name.is_empty() {
        return;
    }

    references.push(RuleSetReference {
        provider_name: provider_name.to_string(),
        path: rule_provider_path(&scope, rule_index),
        scope,
        rule_index,
        raw: rule.raw.clone(),
    });
}

fn rule_provider_path(scope: &RuleProviderReferenceScope, rule_index: usize) -> String {
    match scope {
        RuleProviderReferenceScope::Rules => format!("rules[{rule_index}]"),
        RuleProviderReferenceScope::SubRules(name) => {
            format!("sub-rules.{name}[{rule_index}]")
        }
    }
}

fn split_rule_segments(rule: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (index, ch) in rule.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&rule[start..index]);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(&rule[start..]);
    parts
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(|value| value.as_str().map(ToOwned::to_owned))
}

fn u64_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(Value::as_u64)
}

fn normalize_token(value: &str) -> String {
    value.trim().replace('_', "-").to_ascii_lowercase()
}

fn normalize_rule_type(value: &str) -> String {
    value.trim().replace('_', "-").to_ascii_uppercase()
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use air_config::ConfigDocument;

    fn docs_document() -> MihomoConfigDocument {
        ConfigDocument::parse(include_str!("../../../../docs/config.yaml"))
            .expect("docs/config.yaml should parse")
            .typed
    }

    fn has_error_at(diagnostics: &[ConfigDiagnostic], path: &str) -> bool {
        diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == ConfigDiagnosticSeverity::Error && diagnostic.path == path
        })
    }

    #[test]
    fn parses_docs_rule_providers() {
        let document = docs_document();
        let collection = RuleProviderCollection::from_document(&document);

        let rule1 = collection.find("rule1").expect("rule1 should exist");
        assert_eq!(rule1.kind, ProviderKind::Http);
        assert_eq!(rule1.behavior, Some(RuleProviderBehavior::Classical));
        assert_eq!(rule1.interval, Some(259200));
        assert_eq!(rule1.proxy.as_deref(), Some("DIRECT"));

        let rule2 = collection.find("rule2").expect("rule2 should exist");
        assert_eq!(rule2.kind, ProviderKind::File);

        let rule4 = collection.find("rule4").expect("rule4 should exist");
        assert_eq!(rule4.kind, ProviderKind::Inline);
        assert_eq!(
            rule4.payload,
            vec![
                ".blogger.com".to_string(),
                "*.*.microsoft.com".to_string(),
                "books.itunes.apple.com".to_string()
            ]
        );
        assert!(
            collection
                .validate(&document)
                .iter()
                .all(|diagnostic| { diagnostic.severity != ConfigDiagnosticSeverity::Error })
        );
    }

    #[test]
    fn inline_payload_order_is_preserved_on_writeback() {
        let mut document = ConfigDocument::parse(
            r#"
rule-providers:
  inline-rules:
    type: inline
    behavior: domain
    payload:
      - first.example
      - second.example
      - third.example
"#,
        )
        .expect("fixture should parse")
        .typed;
        let mut collection = RuleProviderCollection::from_document(&document);
        let provider = collection
            .find_mut("inline-rules")
            .expect("provider should exist");
        provider.payload.push("fourth.example".to_string());

        collection.apply_to_document(&mut document);

        assert_eq!(
            document.rule_providers["inline-rules"].payload,
            vec![
                "first.example".to_string(),
                "second.example".to_string(),
                "third.example".to_string(),
                "fourth.example".to_string()
            ]
        );
    }

    #[test]
    fn validates_provider_names_and_rule_set_references() {
        let document = ConfigDocument::parse(
            r#"
rule-providers:
  " bad,name ":
    type: http
    behavior: classical
rules:
  - RULE-SET,missing,DIRECT
sub-rules:
  nested:
    - RULE-SET,missing-too,REJECT
"#,
        )
        .expect("fixture should parse")
        .typed;
        let collection = RuleProviderCollection::from_document(&document);

        let diagnostics = collection.validate(&document);

        assert!(has_error_at(&diagnostics, "rule-providers. bad,name .name"));
        assert!(has_error_at(&diagnostics, "rule-providers. bad,name .url"));
        assert!(has_error_at(&diagnostics, "rules[0].provider"));
        assert!(has_error_at(&diagnostics, "sub-rules.nested[0].provider"));
    }

    #[test]
    fn maps_rule_provider_runtime_api_response() {
        let response = ProvidersResponse {
            providers: BTreeMap::from([(
                "reject-list".to_string(),
                serde_json::json!({
                    "name": "reject-list",
                    "type": "Rule",
                    "vehicleType": "HTTP",
                    "behavior": "Classical",
                    "ruleCount": 42,
                    "updatedAt": "2026-05-21T00:00:00Z"
                }),
            )]),
            extra: BTreeMap::new(),
        };

        let runtime: RuleProviderRuntimeCollection = response.into();
        let state = runtime
            .providers
            .get("reject-list")
            .expect("runtime state should exist");

        assert_eq!(state.name, "reject-list");
        assert_eq!(state.vehicle_type.as_deref(), Some("HTTP"));
        assert_eq!(state.rule_count, Some(42));

        let single = RuleProviderRuntimeState::from_api_value(
            "fallback-name",
            serde_json::json!({"rule-count": 3}),
        );
        assert_eq!(single.name, "fallback-name");
        assert_eq!(single.rule_count, Some(3));
    }

    #[derive(Clone, Default)]
    struct MockRuleProviderApi {
        updated: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait]
    impl RuleProviderApi for MockRuleProviderApi {
        async fn rule_providers(&self) -> AppResult<ProvidersResponse> {
            Ok(ProvidersResponse {
                providers: BTreeMap::from([(
                    "mock".to_string(),
                    serde_json::json!({"name": "mock", "ruleCount": 1}),
                )]),
                extra: BTreeMap::new(),
            })
        }

        async fn rule_provider(&self, name: &str) -> AppResult<Value> {
            Ok(serde_json::json!({"name": name, "ruleCount": 7}))
        }

        async fn update_rule_provider(&self, name: &str) -> AppResult<()> {
            self.updated
                .lock()
                .expect("mock mutex")
                .push(name.to_string());
            Ok(())
        }
    }

    #[tokio::test]
    async fn update_service_uses_injected_api_trait() {
        let api = MockRuleProviderApi::default();
        let updated = api.updated.clone();
        let service = RuleProviderUpdateService::new(api);

        let list = service.list_runtime().await.expect("list should succeed");
        assert_eq!(list.providers["mock"].rule_count, Some(1));

        let state = service
            .runtime_state("mock")
            .await
            .expect("single state should succeed");
        assert_eq!(state.rule_count, Some(7));

        service.update("mock").await.expect("update should succeed");
        assert_eq!(
            updated.lock().expect("mock mutex").as_slice(),
            &["mock".to_string()]
        );
    }
}
