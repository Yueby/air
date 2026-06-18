//! 代理节点仓储领域服务。
//!
//! 本模块只处理配置文档中的静态节点列表和编辑语义，不访问 mihomo external-controller。
//! 运行态测速结果可以作为元数据注入，用于列表筛选和排序，但仓储本身不会发起测速请求。

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use air_config::model::{MihomoConfigDocument, RuleLine};

use super::{ProxyNodeCollection, ProxyNodeDisplay, ProxyNodeSettings};

/// 节点列表仓储。它以 `MihomoConfigDocument` 为输入，写回时只覆盖 `proxies` 段。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProxyNodeRepository {
    collection: ProxyNodeCollection,
    metadata: BTreeMap<String, ProxyNodeMetadata>,
}

impl ProxyNodeRepository {
    pub fn from_document(document: &MihomoConfigDocument) -> Self {
        Self {
            collection: ProxyNodeCollection::from_document(document),
            metadata: BTreeMap::new(),
        }
    }

    pub fn apply_to_document(&self, document: &mut MihomoConfigDocument) {
        self.collection.apply_to_document(document);
    }

    pub fn len(&self) -> usize {
        self.collection.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.collection.nodes.is_empty()
    }

    pub fn get(&self, name: &str) -> Option<&ProxyNodeSettings> {
        self.collection.find(name)
    }

    pub fn nodes(&self) -> &[ProxyNodeSettings] {
        &self.collection.nodes
    }

    /// 注入列表展示所需的外部元数据。调用方可以从订阅缓存或运行态 API 汇总这些信息。
    pub fn set_metadata(&mut self, name: impl Into<String>, metadata: ProxyNodeMetadata) {
        self.metadata.insert(name.into(), metadata);
    }

    pub fn set_delay_status(&mut self, name: impl Into<String>, status: ProxyDelayStatus) {
        let name = name.into();
        self.metadata.entry(name).or_default().delay_status = status;
    }

    pub fn set_source(&mut self, name: impl Into<String>, source: ProxyNodeSource) {
        let name = name.into();
        self.metadata.entry(name).or_default().source = source;
    }

    pub fn list(&self, query: &ProxyNodeListQuery) -> Vec<ProxyNodeListItem> {
        let mut items = self
            .collection
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                let metadata = self.metadata_for(&node.common.name);
                if query.matches(node, metadata) {
                    Some(ProxyNodeListItem::from_node(index, node, metadata))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        sort_list_items(&mut items, query.sort);
        items
    }

    pub fn add(&mut self, node: ProxyNodeSettings) -> Result<(), ProxyNodeRepositoryError> {
        self.add_at(self.collection.nodes.len(), node)
    }

    pub fn add_at(
        &mut self,
        index: usize,
        node: ProxyNodeSettings,
    ) -> Result<(), ProxyNodeRepositoryError> {
        let name = normalized_node_name(&node)?;
        self.ensure_name_available(&name, None)?;
        let index = index.min(self.collection.nodes.len());
        self.collection.nodes.insert(index, node);
        Ok(())
    }

    pub fn update(
        &mut self,
        current_name: &str,
        replacement: ProxyNodeSettings,
    ) -> Result<(), ProxyNodeRepositoryError> {
        let index = self
            .index_of(current_name)
            .ok_or_else(|| ProxyNodeRepositoryError::NotFound(current_name.to_string()))?;
        let next_name = normalized_node_name(&replacement)?;
        self.ensure_name_available(&next_name, Some(current_name))?;

        self.collection.nodes[index] = replacement;
        if current_name != next_name {
            if let Some(metadata) = self.metadata.remove(current_name) {
                self.metadata.insert(next_name, metadata);
            }
        }
        Ok(())
    }

    pub fn duplicate(
        &mut self,
        source_name: &str,
        new_name: impl Into<String>,
    ) -> Result<ProxyNodeSettings, ProxyNodeRepositoryError> {
        let new_name = new_name.into();
        self.ensure_non_empty_name(&new_name)?;
        self.ensure_name_available(&new_name, None)?;
        let source = self
            .get(source_name)
            .ok_or_else(|| ProxyNodeRepositoryError::NotFound(source_name.to_string()))?;
        let cloned = source.cloned_as(new_name.clone());
        self.collection.nodes.push(cloned.clone());
        self.metadata
            .entry(new_name)
            .or_insert_with(|| ProxyNodeMetadata {
                source: ProxyNodeSource::Local,
                ..ProxyNodeMetadata::default()
            });
        Ok(cloned)
    }

    pub fn duplicate_many(
        &mut self,
        requests: &[DuplicateProxyNodeRequest],
    ) -> Result<Vec<ProxyNodeSettings>, ProxyNodeRepositoryError> {
        let mut new_names = BTreeSet::new();
        for request in requests {
            self.ensure_non_empty_name(&request.new_name)?;
            self.ensure_name_available(&request.new_name, None)?;
            if !new_names.insert(request.new_name.as_str()) {
                return Err(ProxyNodeRepositoryError::DuplicateName(
                    request.new_name.clone(),
                ));
            }
        }

        let mut created = Vec::new();
        for request in requests {
            created.push(self.duplicate(&request.source_name, request.new_name.clone())?);
        }
        Ok(created)
    }

    pub fn delete(
        &mut self,
        name: &str,
        document: &MihomoConfigDocument,
        policy: DeleteProxyNodePolicy,
    ) -> Result<DeletedProxyNodes, ProxyNodeRepositoryError> {
        self.delete_many([name], document, policy)
    }

    pub fn delete_many<'a>(
        &mut self,
        names: impl IntoIterator<Item = &'a str>,
        document: &MihomoConfigDocument,
        policy: DeleteProxyNodePolicy,
    ) -> Result<DeletedProxyNodes, ProxyNodeRepositoryError> {
        let names = names.into_iter().collect::<BTreeSet<_>>();
        if names.is_empty() {
            return Ok(DeletedProxyNodes::default());
        }

        for name in &names {
            if self.index_of(name).is_none() {
                return Err(ProxyNodeRepositoryError::NotFound((*name).to_string()));
            }
        }

        let impact = self.analyze_delete(names.iter().copied(), document);
        if impact.has_references() && policy == DeleteProxyNodePolicy::ProtectReferences {
            return Err(ProxyNodeRepositoryError::Referenced(Box::new(impact)));
        }

        let mut removed = Vec::new();
        let mut kept = Vec::new();
        for node in self.collection.nodes.drain(..) {
            if names.contains(node.common.name.as_str()) {
                self.metadata.remove(&node.common.name);
                removed.push(node);
            } else {
                kept.push(node);
            }
        }
        self.collection.nodes = kept;

        Ok(DeletedProxyNodes { removed, impact })
    }

    pub fn analyze_delete<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
        document: &MihomoConfigDocument,
    ) -> ProxyNodeDeleteImpact {
        let names = names
            .into_iter()
            .filter(|name| !name.trim().is_empty())
            .map(ToOwned::to_owned)
            .collect::<BTreeSet<_>>();
        let mut impact = ProxyNodeDeleteImpact {
            names: names.iter().cloned().collect(),
            ..ProxyNodeDeleteImpact::default()
        };

        for (group_index, group) in document.proxy_groups.iter().enumerate() {
            for (proxy_index, proxy_name) in group.proxies.iter().enumerate() {
                if names.contains(proxy_name) {
                    impact.group_references.push(ProxyGroupReference {
                        node_name: proxy_name.clone(),
                        group_name: group.name.clone(),
                        group_index,
                        proxy_index,
                    });
                }
            }
        }

        for (rule_index, rule) in document.rules.iter().enumerate() {
            push_rule_reference(
                &mut impact,
                &names,
                RuleReferenceScope::Rules,
                rule_index,
                rule,
            );
        }

        for (sub_rule_name, rules) in &document.sub_rules {
            for (rule_index, rule) in rules.iter().enumerate() {
                push_rule_reference(
                    &mut impact,
                    &names,
                    RuleReferenceScope::SubRules(sub_rule_name.clone()),
                    rule_index,
                    rule,
                );
            }
        }

        impact
    }

    fn metadata_for(&self, name: &str) -> &ProxyNodeMetadata {
        self.metadata.get(name).unwrap_or(&DEFAULT_METADATA)
    }

    fn index_of(&self, name: &str) -> Option<usize> {
        self.collection
            .nodes
            .iter()
            .position(|node| node.common.name == name)
    }

    fn ensure_name_available(
        &self,
        name: &str,
        current_name: Option<&str>,
    ) -> Result<(), ProxyNodeRepositoryError> {
        if self
            .collection
            .nodes
            .iter()
            .any(|node| node.common.name == name && Some(node.common.name.as_str()) != current_name)
        {
            return Err(ProxyNodeRepositoryError::DuplicateName(name.to_string()));
        }
        Ok(())
    }

    fn ensure_non_empty_name(&self, name: &str) -> Result<(), ProxyNodeRepositoryError> {
        if name.trim().is_empty() {
            return Err(ProxyNodeRepositoryError::EmptyName);
        }
        Ok(())
    }
}

/// 节点来源用于 UI 筛选。当前配置文档中的 `proxies` 都是本地节点，订阅来源可由后续任务注入。
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyNodeSource {
    Local,
    Provider(String),
    Subscription(String),
    Unknown,
}

impl Default for ProxyNodeSource {
    fn default() -> Self {
        Self::Local
    }
}

impl ProxyNodeSource {
    pub fn label(&self) -> String {
        match self {
            Self::Local => "local".to_string(),
            Self::Provider(name) => format!("provider:{name}"),
            Self::Subscription(name) => format!("subscription:{name}"),
            Self::Unknown => "unknown".to_string(),
        }
    }
}

/// 延迟状态只描述已有测量结果，不触发测速。`delay_ms` 可选，便于未来运行态页面排序。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyDelayStatus {
    #[default]
    Unknown,
    Untested,
    Testing,
    Available,
    Slow,
    Failed,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyNodeMetadata {
    pub source: ProxyNodeSource,
    pub delay_status: ProxyDelayStatus,
    pub delay_ms: Option<u64>,
}

const DEFAULT_METADATA: ProxyNodeMetadata = ProxyNodeMetadata {
    source: ProxyNodeSource::Local,
    delay_status: ProxyDelayStatus::Unknown,
    delay_ms: None,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyNodeSort {
    #[default]
    DocumentOrder,
    NameAsc,
    NameDesc,
    TypeAsc,
    TypeDesc,
    SourceAsc,
    DelayAsc,
    DelayDesc,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyNodeListQuery {
    pub name_contains: Option<String>,
    pub kinds: BTreeSet<String>,
    pub sources: BTreeSet<ProxyNodeSource>,
    pub delay_statuses: BTreeSet<ProxyDelayStatus>,
    pub sort: ProxyNodeSort,
}

impl ProxyNodeListQuery {
    fn matches(&self, node: &ProxyNodeSettings, metadata: &ProxyNodeMetadata) -> bool {
        if let Some(name_contains) = self.name_contains.as_deref() {
            if !contains_case_insensitive(&node.common.name, name_contains) {
                return false;
            }
        }

        if !self.kinds.is_empty() && !self.kinds.contains(node.common.kind.as_str()) {
            return false;
        }

        if !self.sources.is_empty() && !self.sources.contains(&metadata.source) {
            return false;
        }

        if !self.delay_statuses.is_empty() && !self.delay_statuses.contains(&metadata.delay_status)
        {
            return false;
        }

        true
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProxyNodeListItem {
    pub index: usize,
    pub name: String,
    pub kind: String,
    pub source: ProxyNodeSource,
    pub delay_status: ProxyDelayStatus,
    pub delay_ms: Option<u64>,
    pub display: ProxyNodeDisplay,
}

impl ProxyNodeListItem {
    fn from_node(index: usize, node: &ProxyNodeSettings, metadata: &ProxyNodeMetadata) -> Self {
        Self {
            index,
            name: node.common.name.clone(),
            kind: node.common.kind.as_str().to_string(),
            source: metadata.source.clone(),
            delay_status: metadata.delay_status,
            delay_ms: metadata.delay_ms,
            display: node.redacted_display(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DuplicateProxyNodeRequest {
    pub source_name: String,
    pub new_name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeleteProxyNodePolicy {
    ProtectReferences,
    Force,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeletedProxyNodes {
    /// 批量删除返回值按原配置顺序排列，避免 UI 根据用户勾选顺序产生不可预测的撤销顺序。
    pub removed: Vec<ProxyNodeSettings>,
    pub impact: ProxyNodeDeleteImpact,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyNodeDeleteImpact {
    pub names: Vec<String>,
    pub group_references: Vec<ProxyGroupReference>,
    pub rule_references: Vec<ProxyRuleReference>,
}

impl ProxyNodeDeleteImpact {
    pub fn has_references(&self) -> bool {
        !self.group_references.is_empty() || !self.rule_references.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProxyGroupReference {
    pub node_name: String,
    pub group_name: String,
    pub group_index: usize,
    pub proxy_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProxyRuleReference {
    pub node_name: String,
    pub scope: RuleReferenceScope,
    pub rule_index: usize,
    pub path: String,
    pub raw: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuleReferenceScope {
    Rules,
    SubRules(String),
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ProxyNodeRepositoryError {
    #[error("代理节点名称不能为空")]
    EmptyName,
    #[error("代理节点不存在: {0}")]
    NotFound(String),
    #[error("代理节点名称重复: {0}")]
    DuplicateName(String),
    #[error("代理节点仍被引用，不能安全删除")]
    Referenced(Box<ProxyNodeDeleteImpact>),
}

fn normalized_node_name(node: &ProxyNodeSettings) -> Result<String, ProxyNodeRepositoryError> {
    let name = node.common.name.trim();
    if name.is_empty() {
        return Err(ProxyNodeRepositoryError::EmptyName);
    }
    Ok(name.to_string())
}

fn sort_list_items(items: &mut [ProxyNodeListItem], sort: ProxyNodeSort) {
    items.sort_by(|left, right| {
        let ordering = match sort {
            ProxyNodeSort::DocumentOrder => Ordering::Equal,
            ProxyNodeSort::NameAsc => compare_text(&left.name, &right.name),
            ProxyNodeSort::NameDesc => compare_text(&right.name, &left.name),
            ProxyNodeSort::TypeAsc => compare_text(&left.kind, &right.kind),
            ProxyNodeSort::TypeDesc => compare_text(&right.kind, &left.kind),
            ProxyNodeSort::SourceAsc => left.source.cmp(&right.source),
            ProxyNodeSort::DelayAsc => compare_delay(left, right),
            ProxyNodeSort::DelayDesc => compare_delay(right, left),
        };
        ordering.then_with(|| left.index.cmp(&right.index))
    });
}

fn compare_text(left: &str, right: &str) -> Ordering {
    left.to_ascii_lowercase()
        .cmp(&right.to_ascii_lowercase())
        .then_with(|| left.cmp(right))
}

fn compare_delay(left: &ProxyNodeListItem, right: &ProxyNodeListItem) -> Ordering {
    left.delay_status
        .cmp(&right.delay_status)
        .then_with(|| left.delay_ms.cmp(&right.delay_ms))
}

fn contains_case_insensitive(value: &str, needle: &str) -> bool {
    let needle = needle.trim();
    needle.is_empty() || value.to_lowercase().contains(&needle.to_lowercase())
}

fn push_rule_reference(
    impact: &mut ProxyNodeDeleteImpact,
    names: &BTreeSet<String>,
    scope: RuleReferenceScope,
    rule_index: usize,
    rule: &RuleLine,
) {
    let Some(policy) = rule_policy_reference(&rule.raw) else {
        return;
    };
    if !names.contains(policy) {
        return;
    }

    // mihomo 规则的最终策略仍以节点名称作为引用键；重命名或删除节点时必须先分析影响，
    // 否则规则文本不会自动更新，运行时会退化为“引用不存在”的配置错误。
    impact.rule_references.push(ProxyRuleReference {
        node_name: policy.to_string(),
        path: rule_policy_path(&scope, rule_index),
        scope,
        rule_index,
        raw: rule.raw.clone(),
    });
}

fn rule_policy_reference(rule: &str) -> Option<&str> {
    let parts = split_rule_segments(rule);
    if parts.len() < 2 {
        return None;
    }

    let rule_type = parts[0].trim().to_ascii_uppercase();
    if matches!(rule_type.as_str(), "SUB-RULE" | "SUBRULE") {
        return None;
    }

    parts
        .last()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

fn rule_policy_path(scope: &RuleReferenceScope, rule_index: usize) -> String {
    match scope {
        RuleReferenceScope::Rules => format!("rules[{rule_index}].policy"),
        RuleReferenceScope::SubRules(name) => format!("sub-rules.{name}[{rule_index}].policy"),
    }
}

/// 规则中 AND/OR/NOT 可能在括号内再包含逗号，顶层分割必须避开括号内部。
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

#[cfg(test)]
mod tests {
    use super::*;
    use air_config::ConfigDocument;

    fn repository_fixture() -> (MihomoConfigDocument, ProxyNodeRepository) {
        let document = ConfigDocument::parse(
            r#"
proxies:
  - name: beta
    type: http
    server: b.example
    port: 443
  - name: alpha
    type: direct
  - name: gamma
    type: ss
    server: g.example
    port: 8388
    password: keep
    cipher: aes-128-gcm
proxy-groups:
  - name: Select
    type: select
    proxies:
      - alpha
      - gamma
rules:
  - DOMAIN-SUFFIX,example.com,alpha
  - SUB-RULE,(OR,((NETWORK,TCP),(NETWORK,UDP))),nested
sub-rules:
  nested:
    - DOMAIN,inner.example,gamma
"#,
        )
        .expect("fixture should parse")
        .typed;
        let repository = ProxyNodeRepository::from_document(&document);
        (document, repository)
    }

    #[test]
    fn adds_updates_duplicates_and_writes_back_nodes() {
        let (mut document, mut repository) = repository_fixture();

        let mut delta = repository
            .get("alpha")
            .expect("alpha should exist")
            .cloned_as("delta");
        delta.common.udp = Some(true);
        repository.add(delta).expect("new node should be added");

        let renamed = repository
            .get("beta")
            .expect("beta should exist")
            .cloned_as("beta-renamed");
        repository
            .update("beta", renamed)
            .expect("node should be renamed");

        repository
            .duplicate("gamma", "gamma-copy")
            .expect("node should be duplicated");
        repository.apply_to_document(&mut document);

        let names = document
            .proxies
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["beta-renamed", "alpha", "gamma", "delta", "gamma-copy"]
        );
        assert_eq!(document.proxies[4].password, document.proxies[2].password);
    }

    #[test]
    fn rejects_duplicate_and_empty_names() {
        let (_, mut repository) = repository_fixture();

        let duplicate = repository
            .get("alpha")
            .expect("alpha should exist")
            .cloned_as("beta");
        assert_eq!(
            repository
                .add(duplicate)
                .expect_err("duplicate should fail"),
            ProxyNodeRepositoryError::DuplicateName("beta".to_string())
        );

        let empty = repository
            .get("alpha")
            .expect("alpha should exist")
            .cloned_as(" ");
        assert_eq!(
            repository
                .update("alpha", empty)
                .expect_err("empty should fail"),
            ProxyNodeRepositoryError::EmptyName
        );
    }

    #[test]
    fn lists_with_filters_and_stable_sorting() {
        let (_, mut repository) = repository_fixture();
        repository.set_metadata(
            "gamma",
            ProxyNodeMetadata {
                source: ProxyNodeSource::Subscription("sub-a".to_string()),
                delay_status: ProxyDelayStatus::Available,
                delay_ms: Some(80),
            },
        );
        repository.set_delay_status("alpha", ProxyDelayStatus::Available);

        let query = ProxyNodeListQuery {
            delay_statuses: BTreeSet::from([ProxyDelayStatus::Available]),
            sort: ProxyNodeSort::DelayAsc,
            ..ProxyNodeListQuery::default()
        };
        let names = repository
            .list(&query)
            .into_iter()
            .map(|item| item.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["alpha", "gamma"]);

        let query = ProxyNodeListQuery {
            name_contains: Some("A".to_string()),
            kinds: BTreeSet::from(["direct".to_string(), "ss".to_string()]),
            sort: ProxyNodeSort::NameAsc,
            ..ProxyNodeListQuery::default()
        };
        let names = repository
            .list(&query)
            .into_iter()
            .map(|item| item.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["alpha", "gamma"]);
    }

    #[test]
    fn protected_delete_returns_group_and_rule_impact_without_mutation() {
        let (document, mut repository) = repository_fixture();

        let error = repository
            .delete("gamma", &document, DeleteProxyNodePolicy::ProtectReferences)
            .expect_err("referenced node should not be deleted");

        let ProxyNodeRepositoryError::Referenced(impact) = error else {
            panic!("expected referenced impact");
        };
        assert!(impact.has_references());
        assert_eq!(impact.group_references[0].group_name, "Select");
        assert_eq!(impact.rule_references[0].path, "sub-rules.nested[0].policy");
        assert!(repository.get("gamma").is_some());
    }

    #[test]
    fn force_delete_and_batch_delete_keep_document_order() {
        let (mut document, mut repository) = repository_fixture();

        let deleted = repository
            .delete_many(["gamma", "beta"], &document, DeleteProxyNodePolicy::Force)
            .expect("force delete should remove referenced nodes");

        let removed_names = deleted
            .removed
            .iter()
            .map(|node| node.common.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(removed_names, vec!["beta", "gamma"]);
        assert!(deleted.impact.has_references());

        repository.apply_to_document(&mut document);
        let remaining_names = document
            .proxies
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(remaining_names, vec!["alpha"]);
    }
}
