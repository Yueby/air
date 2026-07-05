//! 代理组的领域配置模型。
//!
//! 配置层的 `ProxyGroup` 负责 YAML 往返和未知字段保留；本模块在其上补充 GUI 和后续
//! 合并任务需要的业务语义：成员来源解析、运行态选择状态，以及少量安全的基础编辑操作。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use air_config::model::{MihomoConfigDocument, ProxyGroup, ProxyGroupKind};
use air_mihomo::dto::ProxyResponse;

/// 配置文档中的代理组集合。集合层只覆盖 `proxy-groups` section，不触碰节点和规则。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProxyGroupCollection {
    pub groups: Vec<ProxyGroupSettings>,
}

impl ProxyGroupCollection {
    pub fn from_document(document: &MihomoConfigDocument) -> Self {
        Self {
            groups: document
                .proxy_groups
                .iter()
                .map(ProxyGroupSettings::from_config)
                .collect(),
        }
    }

    /// 将代理组写回文档。未知组类型和未建模扩展字段来自每个组保存的 `raw` 副本。
    pub fn apply_to_document(&self, document: &mut MihomoConfigDocument) {
        document.proxy_groups = self
            .groups
            .iter()
            .map(ProxyGroupSettings::to_config)
            .collect();
    }

    pub fn find(&self, name: &str) -> Option<&ProxyGroupSettings> {
        self.groups.iter().find(|group| group.common.name == name)
    }

    /// mihomo 运行态响应里的组名大小写可能和本地配置不完全一致，匹配配置组时优先保留配置里的原始名称。
    pub fn find_case_insensitive(&self, name: &str) -> Option<&ProxyGroupSettings> {
        self.find(name).or_else(|| {
            self.groups
                .iter()
                .find(|group| group.common.name.eq_ignore_ascii_case(name))
        })
    }

    pub fn find_mut(&mut self, name: &str) -> Option<&mut ProxyGroupSettings> {
        self.groups
            .iter_mut()
            .find(|group| group.common.name == name)
    }

    pub fn resolve_references(
        &self,
        document: &MihomoConfigDocument,
    ) -> Vec<ProxyGroupMemberReference> {
        self.groups
            .iter()
            .flat_map(|group| group.resolve_references(document))
            .collect()
    }

    /// 将 `/proxies` 或 `/providers/proxies` 中拿到的组响应映射为 UI 可消费的选择状态。
    pub fn selection_states(
        &self,
        document: &MihomoConfigDocument,
        responses: &BTreeMap<String, ProxyResponse>,
        all_responses: &BTreeMap<String, ProxyResponse>,
    ) -> Vec<ProxyGroupSelectionState> {
        responses
            .iter()
            .map(|(name, response)| {
                self.find_case_insensitive(name)
                    .map(|group| group.selection_state(response, all_responses, document))
                    .unwrap_or_else(|| {
                        runtime_only_selection_state(
                            name.clone(),
                            response,
                            all_responses,
                            document,
                        )
                    })
            })
            .collect()
    }

    pub fn selection_states_in_config_order(
        &self,
        document: &MihomoConfigDocument,
        responses: &BTreeMap<String, ProxyResponse>,
        all_responses: &BTreeMap<String, ProxyResponse>,
    ) -> Vec<ProxyGroupSelectionState> {
        let mut states = Vec::new();
        let mut consumed = BTreeSet::new();

        for group in &self.groups {
            if let Some((name, response)) =
                lookup_proxy_response_entry(responses, &group.common.name)
            {
                consumed.insert(name.clone());
                states.push(group.selection_state(response, all_responses, document));
            }
        }

        for (name, response) in responses {
            if consumed.contains(name) {
                continue;
            }
            // 配置之外的运行态组来自 mihomo 内部或外部覆写，订阅顺序无法锚定时稳定追加。
            states.push(runtime_only_selection_state(
                name.clone(),
                response,
                all_responses,
                document,
            ));
        }

        states
    }

    /// 按 mihomo `GLOBAL` 组的 `all` 顺序排列代理组，对齐 Clash Verge / Sparkle 等客户端的展示。
    ///
    /// `GLOBAL.all` 是内核加载配置后的权威顺序（同时包含代理组与节点），这里只挑出其中的可见
    /// 代理组；未出现在 `GLOBAL.all` 中的可见组再按配置 `proxy-groups` 顺序兜底，最后追加纯运行态组。
    /// 之所以不直接用配置文档顺序，是因为订阅缓存文档可能缺少 `proxy-groups`（例如仅含节点的订阅），
    /// 导致像“节点选择”这类应置顶的组被当成运行态组沉到末尾，与用户熟悉的客户端顺序不一致。
    pub fn selection_states_in_runtime_order(
        &self,
        document: &MihomoConfigDocument,
        responses: &BTreeMap<String, ProxyResponse>,
        all_responses: &BTreeMap<String, ProxyResponse>,
    ) -> Vec<ProxyGroupSelectionState> {
        let mut states = Vec::new();
        let mut consumed = BTreeSet::new();

        // 1. 依 GLOBAL.all 的权威顺序输出可见代理组。
        if let Some((_, global)) = lookup_proxy_response_entry(all_responses, "GLOBAL") {
            for member_name in &global.all {
                if let Some((key, response)) = lookup_proxy_response_entry(responses, member_name) {
                    if consumed.insert(key.clone()) {
                        states.push(self.selection_state_for(
                            key,
                            response,
                            all_responses,
                            document,
                        ));
                    }
                }
            }
        }

        // 2. GLOBAL.all 未覆盖的配置组按配置顺序兜底。
        for group in &self.groups {
            if let Some((key, response)) =
                lookup_proxy_response_entry(responses, &group.common.name)
            {
                if consumed.insert(key.clone()) {
                    states.push(group.selection_state(response, all_responses, document));
                }
            }
        }

        // 3. 仍未输出的纯运行态组稳定追加。
        for (name, response) in responses {
            if consumed.contains(name) {
                continue;
            }
            states.push(runtime_only_selection_state(
                name.clone(),
                response,
                all_responses,
                document,
            ));
        }

        states
    }

    /// 依据配置是否存在同名组，选择使用配置组投影或纯运行态投影。
    fn selection_state_for(
        &self,
        name: &str,
        response: &ProxyResponse,
        all_responses: &BTreeMap<String, ProxyResponse>,
        document: &MihomoConfigDocument,
    ) -> ProxyGroupSelectionState {
        match self.find_case_insensitive(name) {
            Some(group) => group.selection_state(response, all_responses, document),
            None => {
                runtime_only_selection_state(name.to_string(), response, all_responses, document)
            }
        }
    }
}

/// 单个代理组的领域表示。`raw` 私有保存，保证基础编辑不会丢弃高级字段。
#[derive(Clone, PartialEq)]
pub struct ProxyGroupSettings {
    pub common: ProxyGroupCommonSettings,
    pub members: ProxyGroupMemberSettings,
    pub health_check: ProxyGroupHealthCheckSettings,
    pub balancing: ProxyGroupBalancingSettings,
    raw: ProxyGroup,
}

impl ProxyGroupSettings {
    pub fn from_config(group: &ProxyGroup) -> Self {
        Self {
            common: ProxyGroupCommonSettings::from_config(group),
            members: ProxyGroupMemberSettings::from_config(group),
            health_check: ProxyGroupHealthCheckSettings::from_config(group),
            balancing: ProxyGroupBalancingSettings::from_config(group),
            raw: group.clone(),
        }
    }

    pub fn to_config(&self) -> ProxyGroup {
        let mut group = self.raw.clone();
        self.common.apply_to_config(&mut group);
        self.members.apply_to_config(&mut group);
        self.health_check.apply_to_config(&mut group);
        self.balancing.apply_to_config(&mut group);
        group
    }

    pub fn raw_group(&self) -> ProxyGroup {
        self.to_config()
    }

    pub fn rename(&mut self, new_name: impl Into<String>) {
        let new_name = new_name.into();
        self.common.name = new_name.clone();
        self.raw.name = new_name;
    }

    pub fn add_proxy_member(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !name.trim().is_empty() && !self.members.proxies.iter().any(|item| item == &name) {
            self.members.proxies.push(name);
        }
    }

    pub fn remove_proxy_member(&mut self, name: &str) -> bool {
        remove_string_member(&mut self.members.proxies, name)
    }

    pub fn add_provider_member(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !name.trim().is_empty() && !self.members.use_providers.iter().any(|item| item == &name) {
            self.members.use_providers.push(name);
        }
    }

    pub fn remove_provider_member(&mut self, name: &str) -> bool {
        remove_string_member(&mut self.members.use_providers, name)
    }

    /// 解析组内显式成员引用。`use` 引用的是 provider，`proxies` 引用的是节点、其他组或内置策略。
    pub fn resolve_references(
        &self,
        document: &MihomoConfigDocument,
    ) -> Vec<ProxyGroupMemberReference> {
        let index = ReferenceIndex::from_document(document);
        let mut references = Vec::new();

        for (member_index, name) in self.members.proxies.iter().enumerate() {
            references.push(ProxyGroupMemberReference {
                group_name: self.common.name.clone(),
                member_name: name.clone(),
                origin: ProxyGroupMemberOrigin::Proxies,
                index: member_index,
                source: index.resolve_proxy_member(name),
            });
        }

        for (member_index, name) in self.members.use_providers.iter().enumerate() {
            references.push(ProxyGroupMemberReference {
                group_name: self.common.name.clone(),
                member_name: name.clone(),
                origin: ProxyGroupMemberOrigin::UseProviders,
                index: member_index,
                source: index.resolve_provider_member(name),
            });
        }

        references
    }

    /// 把 mihomo API 的 `/proxies/{group}` 响应衔接到静态配置。API 的 `all` 可能包含
    /// provider 展开后的节点，因此这里再次按完整文档解析来源，而不是只看当前组的配置列表。
    pub fn selection_state(
        &self,
        response: &ProxyResponse,
        all_responses: &BTreeMap<String, ProxyResponse>,
        document: &MihomoConfigDocument,
    ) -> ProxyGroupSelectionState {
        let index = ReferenceIndex::from_runtime(document, all_responses);
        let configured = self
            .resolve_references(document)
            .into_iter()
            .map(|reference| reference.member_name)
            .collect::<BTreeSet<_>>();

        ProxyGroupSelectionState {
            group_name: self.common.name.clone(),
            configured_kind: self.common.kind.clone(),
            api_kind: empty_to_none(response.kind.trim()).map(ToOwned::to_owned),
            selected: empty_to_none(response.now.trim()).map(ToOwned::to_owned),
            members: response
                .all
                .iter()
                .enumerate()
                .map(|(index_in_api, name)| ProxyGroupRuntimeMember {
                    name: name.clone(),
                    source: index.resolve_any_member(name),
                    protocol: index.proxy_protocol(name),
                    selected: response.now == *name,
                    configured: configured.contains(name),
                    index_in_api,
                    // 成员节点历史测速来自 `/proxies` 的叶子或嵌套组对象；代理组页在未手动测速前
                    // 需要回显这份最近一次结果，因此在领域投影阶段一并带给 UI。
                    history: lookup_proxy_response(all_responses, name)
                        .map(|member| member.history.clone())
                        .unwrap_or_default(),
                })
                .collect(),
            history: response.history.clone(),
        }
    }
}

impl std::fmt::Debug for ProxyGroupSettings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProxyGroupSettings")
            .field("common", &self.common)
            .field("members", &self.members)
            .field("health_check", &self.health_check)
            .field("balancing", &self.balancing)
            .field("raw", &"<preserved>")
            .finish()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyGroupCommonSettings {
    pub name: String,
    pub kind: ProxyGroupKind,
    pub disable_udp: Option<bool>,
}

impl ProxyGroupCommonSettings {
    fn from_config(group: &ProxyGroup) -> Self {
        Self {
            name: group.name.clone(),
            kind: group.kind.clone(),
            disable_udp: group.disable_udp,
        }
    }

    fn apply_to_config(&self, group: &mut ProxyGroup) {
        group.name = self.name.clone();
        group.kind = self.kind.clone();
        group.disable_udp = self.disable_udp;
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyGroupMemberSettings {
    pub proxies: Vec<String>,
    pub use_providers: Vec<String>,
    pub filter: Option<String>,
    pub exclude_filter: Option<String>,
}

impl ProxyGroupMemberSettings {
    fn from_config(group: &ProxyGroup) -> Self {
        Self {
            proxies: group.proxies.clone(),
            use_providers: group.use_providers.clone(),
            filter: normalize_optional_string(group.filter.as_deref()),
            exclude_filter: normalize_optional_string(group.exclude_filter.as_deref()),
        }
    }

    fn apply_to_config(&self, group: &mut ProxyGroup) {
        group.proxies = self.proxies.clone();
        group.use_providers = self.use_providers.clone();
        group.filter = normalize_optional_string(self.filter.as_deref());
        group.exclude_filter = normalize_optional_string(self.exclude_filter.as_deref());
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyGroupHealthCheckSettings {
    pub url: Option<String>,
    pub interval: Option<u64>,
    pub tolerance: Option<u64>,
    pub lazy: Option<bool>,
    pub expected_status: Option<Value>,
}

impl ProxyGroupHealthCheckSettings {
    fn from_config(group: &ProxyGroup) -> Self {
        Self {
            url: normalize_optional_string(group.url.as_deref()),
            interval: group.interval,
            tolerance: group.tolerance,
            lazy: group.lazy,
            expected_status: group.expected_status.clone(),
        }
    }

    fn apply_to_config(&self, group: &mut ProxyGroup) {
        group.url = normalize_optional_string(self.url.as_deref());
        group.interval = self.interval;
        group.tolerance = self.tolerance;
        group.lazy = self.lazy;
        group.expected_status = self.expected_status.clone();
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyGroupBalancingSettings {
    pub strategy: Option<String>,
}

impl ProxyGroupBalancingSettings {
    fn from_config(group: &ProxyGroup) -> Self {
        Self {
            strategy: normalize_optional_string(group.strategy.as_deref()),
        }
    }

    fn apply_to_config(&self, group: &mut ProxyGroup) {
        group.strategy = normalize_optional_string(self.strategy.as_deref());
    }
}

/// 成员来源。`ProxyProvider` 通常来自 `use` 字段，运行态 API 中也可能出现 provider 展开后的节点。
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyGroupMemberSource {
    ProxyNode,
    ProxyGroup,
    ProxyProvider,
    BuiltInPolicy,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyGroupMemberOrigin {
    Proxies,
    UseProviders,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProxyGroupMemberReference {
    pub group_name: String,
    pub member_name: String,
    pub origin: ProxyGroupMemberOrigin,
    pub index: usize,
    pub source: ProxyGroupMemberSource,
}

/// `/proxies/{group}` 的运行态选择状态。这里只记录 API 已返回的信息，不触发真实延迟测试。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyGroupSelectionState {
    pub group_name: String,
    pub configured_kind: ProxyGroupKind,
    pub api_kind: Option<String>,
    pub selected: Option<String>,
    pub members: Vec<ProxyGroupRuntimeMember>,
    pub history: Vec<serde_json::Value>,
}

/// 代理组运行态投影由 app 层从 mihomo API 聚合后投递给 UI。
/// 配置结构仍然从本地 YAML 读取，运行态只覆盖当前选择、API 展开的成员和历史数据。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyGroupRuntimeProjection {
    pub states: Vec<ProxyGroupSelectionState>,
}

impl Default for ProxyGroupSelectionState {
    fn default() -> Self {
        Self {
            group_name: String::new(),
            configured_kind: ProxyGroupKind::Other(String::new()),
            api_kind: None,
            selected: None,
            members: Vec::new(),
            history: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProxyGroupRuntimeMember {
    pub name: String,
    pub source: ProxyGroupMemberSource,
    #[serde(default)]
    pub protocol: Option<String>,
    pub selected: bool,
    pub configured: bool,
    pub index_in_api: usize,
    #[serde(default)]
    pub history: Vec<serde_json::Value>,
}

struct ReferenceIndex {
    proxy_nodes: BTreeSet<String>,
    proxy_protocols: BTreeMap<String, String>,
    proxy_groups: BTreeSet<String>,
    proxy_providers: BTreeSet<String>,
}

impl ReferenceIndex {
    fn from_document(document: &MihomoConfigDocument) -> Self {
        Self {
            proxy_nodes: document
                .proxies
                .iter()
                .map(|proxy| proxy.name.clone())
                .collect(),
            proxy_protocols: document
                .proxies
                .iter()
                .map(|proxy| (proxy.name.clone(), proxy.kind.as_str().to_string()))
                .collect(),
            proxy_groups: document
                .proxy_groups
                .iter()
                .map(|group| group.name.clone())
                .collect(),
            proxy_providers: document.proxy_providers.keys().cloned().collect(),
        }
    }

    fn from_runtime(
        document: &MihomoConfigDocument,
        all_responses: &BTreeMap<String, ProxyResponse>,
    ) -> Self {
        let mut index = Self::from_document(document);
        for (name, response) in all_responses {
            if is_builtin_policy(name) || is_runtime_proxy_group_response(response) {
                continue;
            }
            let runtime_name = empty_to_none(response.name.trim()).unwrap_or(name);
            if runtime_name.trim().is_empty() {
                continue;
            }
            index.proxy_nodes.insert(runtime_name.to_string());
            if let Some(kind) = empty_to_none(response.kind.trim()) {
                // 运行态 `/proxies` 是代理页当前展示的权威数据源；订阅展开节点可能不在本地 YAML 中，
                // 因此这里优先把 API 返回的 type 写入索引，避免 UI 回退成 Unknown。
                index
                    .proxy_protocols
                    .insert(runtime_name.to_string(), kind.to_string());
            }
        }
        index
    }

    fn resolve_proxy_member(&self, name: &str) -> ProxyGroupMemberSource {
        if self.proxy_nodes.contains(name) {
            ProxyGroupMemberSource::ProxyNode
        } else if self.proxy_groups.contains(name) {
            ProxyGroupMemberSource::ProxyGroup
        } else if is_builtin_policy(name) {
            ProxyGroupMemberSource::BuiltInPolicy
        } else if self.proxy_providers.contains(name) {
            ProxyGroupMemberSource::ProxyProvider
        } else {
            ProxyGroupMemberSource::Unresolved
        }
    }

    fn resolve_provider_member(&self, name: &str) -> ProxyGroupMemberSource {
        if self.proxy_providers.contains(name) {
            ProxyGroupMemberSource::ProxyProvider
        } else {
            ProxyGroupMemberSource::Unresolved
        }
    }

    fn resolve_any_member(&self, name: &str) -> ProxyGroupMemberSource {
        if self.proxy_nodes.contains(name) {
            ProxyGroupMemberSource::ProxyNode
        } else if self.proxy_groups.contains(name) {
            ProxyGroupMemberSource::ProxyGroup
        } else if self.proxy_providers.contains(name) {
            ProxyGroupMemberSource::ProxyProvider
        } else if is_builtin_policy(name) {
            ProxyGroupMemberSource::BuiltInPolicy
        } else {
            ProxyGroupMemberSource::Unresolved
        }
    }

    fn proxy_protocol(&self, name: &str) -> Option<String> {
        self.proxy_protocols.get(name).cloned()
    }
}

fn remove_string_member(values: &mut Vec<String>, name: &str) -> bool {
    let original_len = values.len();
    values.retain(|item| item != name);
    original_len != values.len()
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn empty_to_none(value: &str) -> Option<&str> {
    if value.is_empty() { None } else { Some(value) }
}

fn is_builtin_policy(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_uppercase().as_str(),
        "DIRECT" | "REJECT" | "REJECT-DROP" | "PASS" | "COMPATIBLE" | "GLOBAL"
    )
}

fn is_runtime_proxy_group_response(response: &ProxyResponse) -> bool {
    if !response.all.is_empty() {
        return true;
    }
    matches!(
        response.kind.trim().to_ascii_lowercase().as_str(),
        "selector"
            | "select"
            | "urltest"
            | "url-test"
            | "fallback"
            | "loadbalance"
            | "load-balance"
            | "relay"
    )
}

fn runtime_only_selection_state(
    group_name: String,
    response: &ProxyResponse,
    all_responses: &BTreeMap<String, ProxyResponse>,
    document: &MihomoConfigDocument,
) -> ProxyGroupSelectionState {
    let index = ReferenceIndex::from_runtime(document, all_responses);
    ProxyGroupSelectionState {
        group_name,
        configured_kind: proxy_group_kind_from_api(&response.kind),
        api_kind: empty_to_none(response.kind.trim()).map(ToOwned::to_owned),
        selected: empty_to_none(response.now.trim()).map(ToOwned::to_owned),
        members: response
            .all
            .iter()
            .enumerate()
            .map(|(index_in_api, name)| ProxyGroupRuntimeMember {
                name: name.clone(),
                source: index.resolve_any_member(name),
                protocol: index.proxy_protocol(name),
                selected: response.now == *name,
                configured: false,
                index_in_api,
                history: lookup_proxy_response(all_responses, name)
                    .map(|member| member.history.clone())
                    .unwrap_or_default(),
            })
            .collect(),
        history: response.history.clone(),
    }
}

fn lookup_proxy_response<'a>(
    responses: &'a BTreeMap<String, ProxyResponse>,
    name: &str,
) -> Option<&'a ProxyResponse> {
    lookup_proxy_response_entry(responses, name).map(|(_, response)| response)
}

fn lookup_proxy_response_entry<'a>(
    responses: &'a BTreeMap<String, ProxyResponse>,
    name: &str,
) -> Option<(&'a String, &'a ProxyResponse)> {
    responses.get_key_value(name).or_else(|| {
        responses
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
    })
}

fn proxy_group_kind_from_api(kind: &str) -> ProxyGroupKind {
    match kind.trim().to_ascii_lowercase().as_str() {
        "selector" | "select" => ProxyGroupKind::Select,
        "urltest" | "url-test" => ProxyGroupKind::UrlTest,
        "fallback" => ProxyGroupKind::Fallback,
        "loadbalance" | "load-balance" => ProxyGroupKind::LoadBalance,
        "relay" => ProxyGroupKind::Relay,
        _ => ProxyGroupKind::Other(kind.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use air_config::ConfigDocument;
    use serde_json::json;

    fn docs_document() -> MihomoConfigDocument {
        ConfigDocument::parse(include_str!("../../../../docs/config.yaml"))
            .expect("docs/config.yaml should parse")
            .typed
    }

    fn docs_collection() -> (MihomoConfigDocument, ProxyGroupCollection) {
        let document = docs_document();
        let collection = ProxyGroupCollection::from_document(&document);
        (document, collection)
    }

    fn reference_for<'a>(
        references: &'a [ProxyGroupMemberReference],
        group_name: &str,
        origin: ProxyGroupMemberOrigin,
        member_name: &str,
    ) -> &'a ProxyGroupMemberReference {
        references
            .iter()
            .find(|reference| {
                reference.group_name == group_name
                    && reference.origin == origin
                    && reference.member_name == member_name
            })
            .unwrap_or_else(|| panic!("missing reference {group_name}/{member_name}"))
    }

    #[test]
    fn parses_docs_config_proxy_group_variants() {
        let (_, collection) = docs_collection();

        assert!(matches!(
            collection
                .find("relay")
                .expect("relay should exist")
                .common
                .kind,
            ProxyGroupKind::Relay
        ));
        assert!(matches!(
            collection
                .find("auto")
                .expect("auto should exist")
                .common
                .kind,
            ProxyGroupKind::UrlTest
        ));
        assert!(matches!(
            collection
                .find("fallback-auto")
                .expect("fallback should exist")
                .common
                .kind,
            ProxyGroupKind::Fallback
        ));
        assert!(matches!(
            collection
                .find("load-balance")
                .expect("load-balance should exist")
                .common
                .kind,
            ProxyGroupKind::LoadBalance
        ));
        assert!(matches!(
            collection
                .find("Proxy")
                .expect("Proxy should exist")
                .common
                .kind,
            ProxyGroupKind::Select
        ));

        let auto = collection.find("auto").expect("auto should exist");
        assert_eq!(auto.health_check.interval, Some(300));
        assert_eq!(
            auto.health_check.url.as_deref(),
            Some("https://cp.cloudflare.com/generate_204")
        );
    }

    #[test]
    fn resolves_node_group_provider_and_builtin_references() {
        let (document, collection) = docs_collection();
        let references = collection.resolve_references(&document);

        assert_eq!(
            reference_for(&references, "Proxy", ProxyGroupMemberOrigin::Proxies, "ss1").source,
            ProxyGroupMemberSource::ProxyNode
        );
        assert_eq!(
            reference_for(
                &references,
                "Proxy",
                ProxyGroupMemberOrigin::Proxies,
                "auto"
            )
            .source,
            ProxyGroupMemberSource::ProxyGroup
        );
        assert_eq!(
            reference_for(
                &references,
                "UseProvider",
                ProxyGroupMemberOrigin::UseProviders,
                "provider1"
            )
            .source,
            ProxyGroupMemberSource::ProxyProvider
        );
        assert_eq!(
            reference_for(
                &references,
                "UseProvider",
                ProxyGroupMemberOrigin::Proxies,
                "DIRECT"
            )
            .source,
            ProxyGroupMemberSource::BuiltInPolicy
        );
    }

    #[test]
    fn preserves_unknown_group_type_and_extensions() {
        let document = ConfigDocument::parse(
            r#"
proxy-groups:
  - name: future
    type: smart-select
    proxies:
      - DIRECT
    use:
      - missing-provider
    url: https://example.com/check
    future-field:
      nested: true
"#,
        )
        .expect("unknown group type should parse")
        .typed;
        let mut collection = ProxyGroupCollection::from_document(&document);
        let group = collection
            .find_mut("future")
            .expect("future group should exist");

        assert!(matches!(
            group.common.kind,
            ProxyGroupKind::Other(ref value) if value == "smart-select"
        ));
        group.add_proxy_member("REJECT");
        group.health_check.interval = Some(120);

        let mut written = MihomoConfigDocument::default();
        collection.apply_to_document(&mut written);

        assert_eq!(written.proxy_groups[0].kind, document.proxy_groups[0].kind);
        assert!(
            written.proxy_groups[0]
                .proxies
                .contains(&"REJECT".to_string())
        );
        assert_eq!(written.proxy_groups[0].interval, Some(120));
        assert!(
            written.proxy_groups[0]
                .extensions
                .contains_key("future-field")
        );
    }

    #[test]
    fn maps_proxy_api_response_to_selection_state() {
        let (document, collection) = docs_collection();
        let group = collection.find("Proxy").expect("Proxy should exist");
        let response = ProxyResponse {
            name: String::new(),
            all: vec![
                "ss1".to_string(),
                "auto".to_string(),
                "DIRECT".to_string(),
                "provider-expanded".to_string(),
            ],
            now: "auto".to_string(),
            kind: "Selector".to_string(),
            history: Vec::new(),
            extra: BTreeMap::new(),
        };

        let responses = BTreeMap::from([(group.common.name.clone(), response.clone())]);
        let state = group.selection_state(&response, &responses, &document);

        assert_eq!(state.selected.as_deref(), Some("auto"));
        assert_eq!(state.api_kind.as_deref(), Some("Selector"));
        assert!(state.members.iter().any(|member| {
            member.name == "ss1"
                && member.source == ProxyGroupMemberSource::ProxyNode
                && member.configured
        }));
        assert!(state.members.iter().any(|member| {
            member.name == "auto"
                && member.source == ProxyGroupMemberSource::ProxyGroup
                && member.selected
        }));
        assert!(state.members.iter().any(|member| {
            member.name == "DIRECT" && member.source == ProxyGroupMemberSource::BuiltInPolicy
        }));
        assert!(state.members.iter().any(|member| {
            member.name == "provider-expanded"
                && member.source == ProxyGroupMemberSource::Unresolved
                && !member.configured
        }));
    }

    #[test]
    fn runtime_members_use_leaf_proxy_info_from_proxies_response() {
        let document = ConfigDocument::parse(
            r#"
proxy-groups:
  - name: Proxy
    type: select
    proxies:
      - provider-expanded
"#,
        )
        .expect("runtime-only leaf fixture should parse")
        .typed;
        let collection = ProxyGroupCollection::from_document(&document);
        let group = collection.find("Proxy").expect("Proxy group should exist");
        let group_response = ProxyResponse {
            name: "Proxy".to_string(),
            all: vec!["provider-expanded".to_string()],
            now: "provider-expanded".to_string(),
            kind: "Selector".to_string(),
            history: Vec::new(),
            extra: BTreeMap::new(),
        };
        let responses = BTreeMap::from([
            ("Proxy".to_string(), group_response.clone()),
            (
                "provider-expanded".to_string(),
                ProxyResponse {
                    name: "provider-expanded".to_string(),
                    all: Vec::new(),
                    now: String::new(),
                    kind: "Vless".to_string(),
                    history: Vec::new(),
                    extra: BTreeMap::new(),
                },
            ),
        ]);

        let state = group.selection_state(&group_response, &responses, &document);
        let member = state
            .members
            .iter()
            .find(|member| member.name == "provider-expanded")
            .expect("provider expanded leaf should be projected");

        assert_eq!(member.source, ProxyGroupMemberSource::ProxyNode);
        assert_eq!(member.protocol.as_deref(), Some("Vless"));
    }

    #[test]
    fn maps_runtime_only_group_response_from_merged_config() {
        let (document, collection) = docs_collection();
        let mut responses = BTreeMap::new();
        responses.insert(
            "SSRDOG".to_string(),
            ProxyResponse {
                name: "SSRDOG".to_string(),
                all: vec!["DIRECT".to_string(), "ss1".to_string()],
                now: "DIRECT".to_string(),
                kind: "Selector".to_string(),
                history: Vec::new(),
                extra: BTreeMap::new(),
            },
        );

        let states = collection.selection_states(&document, &responses, &responses);
        let state = states
            .iter()
            .find(|state| state.group_name == "SSRDOG")
            .expect("runtime-only group should be retained");

        assert_eq!(state.configured_kind, ProxyGroupKind::Select);
        assert_eq!(state.selected.as_deref(), Some("DIRECT"));
        assert!(state.members.iter().any(|member| member.name == "ss1"));
    }

    #[test]
    fn runtime_members_keep_protocol_from_merged_config() {
        let document = ConfigDocument::parse(
            r#"
proxies:
  - name: tls-node
    type: anytls
    server: example.com
    port: 443
proxy-groups:
  - name: Proxy
    type: select
    proxies:
      - tls-node
"#,
        )
        .expect("runtime config with anytls should parse")
        .typed;
        let collection = ProxyGroupCollection::from_document(&document);
        let mut responses = BTreeMap::new();
        responses.insert(
            "Proxy".to_string(),
            ProxyResponse {
                name: "Proxy".to_string(),
                all: vec!["tls-node".to_string()],
                now: "tls-node".to_string(),
                kind: "Selector".to_string(),
                history: Vec::new(),
                extra: BTreeMap::new(),
            },
        );
        responses.insert(
            "tls-node".to_string(),
            ProxyResponse {
                name: "tls-node".to_string(),
                all: Vec::new(),
                now: String::new(),
                kind: "AnyTLS".to_string(),
                history: vec![json!({
                    "time": "2026-05-29T10:00:00+08:00",
                    "delay": 166,
                })],
                extra: BTreeMap::new(),
            },
        );

        let state = collection
            .selection_states(&document, &responses, &responses)
            .into_iter()
            .next()
            .expect("projection should include proxy group");

        assert_eq!(state.members[0].source, ProxyGroupMemberSource::ProxyNode);
        assert_eq!(state.members[0].protocol.as_deref(), Some("AnyTLS"));
        assert_eq!(state.members[0].history.len(), 1);
    }

    #[test]
    fn matches_runtime_group_names_case_insensitively() {
        let (document, collection) = docs_collection();
        let mut responses = BTreeMap::new();
        responses.insert(
            "proxy".to_string(),
            ProxyResponse {
                name: "proxy".to_string(),
                all: vec!["DIRECT".to_string()],
                now: "DIRECT".to_string(),
                kind: "SELECTOR".to_string(),
                history: Vec::new(),
                extra: BTreeMap::new(),
            },
        );

        let states = collection.selection_states(&document, &responses, &responses);
        let state = states.first().expect("runtime state should be mapped");

        assert_eq!(state.group_name, "Proxy");
        assert_eq!(state.configured_kind, ProxyGroupKind::Select);
        assert_eq!(state.api_kind.as_deref(), Some("SELECTOR"));
    }

    #[test]
    fn basic_member_operations_write_back_stably() {
        let (mut document, mut collection) = docs_collection();
        let group = collection
            .find_mut("Proxy")
            .expect("Proxy group should exist");

        group.add_proxy_member("DIRECT");
        group.add_proxy_member("DIRECT");
        assert!(group.remove_proxy_member("ss2"));
        group.rename("Proxy Renamed");
        collection.apply_to_document(&mut document);

        let written = document
            .proxy_groups
            .iter()
            .find(|group| group.name == "Proxy Renamed")
            .expect("renamed group should be written");
        assert_eq!(
            written
                .proxies
                .iter()
                .filter(|name| name.as_str() == "DIRECT")
                .count(),
            1
        );
        assert!(!written.proxies.contains(&"ss2".to_string()));
    }

    #[test]
    fn orders_groups_by_global_all_like_other_clients() {
        // 配置里只定义了 Proxy，运行态却先出现“节点选择”。按配置顺序会把运行态组沉底，
        // 这里验证新方法跟随 GLOBAL.all 把“节点选择”排在最前，与 Sparkle / Clash Verge 一致。
        let document = ConfigDocument::parse(
            r#"
proxy-groups:
  - name: Proxy
    type: select
    proxies:
      - DIRECT
"#,
        )
        .expect("config should parse")
        .typed;
        let collection = ProxyGroupCollection::from_document(&document);

        let mut responses = BTreeMap::new();
        responses.insert(
            "节点选择".to_string(),
            ProxyResponse {
                name: "节点选择".to_string(),
                all: vec!["Proxy".to_string(), "DIRECT".to_string()],
                now: "Proxy".to_string(),
                kind: "Selector".to_string(),
                history: Vec::new(),
                extra: BTreeMap::new(),
            },
        );
        responses.insert(
            "Proxy".to_string(),
            ProxyResponse {
                name: "Proxy".to_string(),
                all: vec!["DIRECT".to_string()],
                now: "DIRECT".to_string(),
                kind: "Selector".to_string(),
                history: Vec::new(),
                extra: BTreeMap::new(),
            },
        );

        let mut all_responses = responses.clone();
        all_responses.insert(
            "GLOBAL".to_string(),
            ProxyResponse {
                name: "GLOBAL".to_string(),
                all: vec!["节点选择".to_string(), "Proxy".to_string()],
                now: "节点选择".to_string(),
                kind: "Selector".to_string(),
                history: Vec::new(),
                extra: BTreeMap::new(),
            },
        );

        let runtime_ordered: Vec<String> = collection
            .selection_states_in_runtime_order(&document, &responses, &all_responses)
            .into_iter()
            .map(|state| state.group_name)
            .collect();
        assert_eq!(
            runtime_ordered,
            vec!["节点选择".to_string(), "Proxy".to_string()]
        );

        // 对照：旧的配置顺序方法会把运行态组“节点选择”排到配置组之后。
        let config_ordered: Vec<String> = collection
            .selection_states_in_config_order(&document, &responses, &all_responses)
            .into_iter()
            .map(|state| state.group_name)
            .collect();
        assert_eq!(
            config_ordered,
            vec!["Proxy".to_string(), "节点选择".to_string()]
        );
    }
}
