//! 路由规则的领域配置模型。
//!
//! 配置层的 `RuleLine` 只负责保留 YAML 中的原始字符串；本模块在它之上解析规则类型、
//! 匹配参数、目标策略和附加参数。mihomo 按 `rules` 自上而下短路匹配，`SUB-RULE` 又会把
//! 流量分叉到命名子规则集，因此这里显式保留顺序索引，后续 UI 做拖拽排序或临时禁用时
//! 不能把规则当成无序集合处理。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use air_config::model::{MihomoConfigDocument, RuleLine};
use air_mihomo::dto::RulesResponse;

/// 完整规则配置集合。`rules` 是主规则链，`sub_rules` 只能被 `SUB-RULE` 跳转使用。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleCollection {
    pub rules: Vec<RuleSettings>,
    pub sub_rules: BTreeMap<String, Vec<RuleSettings>>,
}

impl RuleCollection {
    pub fn from_document(document: &MihomoConfigDocument) -> Self {
        Self {
            rules: document
                .rules
                .iter()
                .enumerate()
                .map(|(index, line)| RuleSettings::from_line(RuleScope::Rules, index, line))
                .collect(),
            sub_rules: document
                .sub_rules
                .iter()
                .map(|(name, rules)| {
                    (
                        name.clone(),
                        rules
                            .iter()
                            .enumerate()
                            .map(|(index, line)| {
                                RuleSettings::from_line(
                                    RuleScope::SubRules(name.clone()),
                                    index,
                                    line,
                                )
                            })
                            .collect(),
                    )
                })
                .collect(),
        }
    }

    /// 将领域模型写回配置文档。运行态禁用状态不属于 YAML 配置，不能在这里落盘。
    pub fn apply_to_document(&self, document: &mut MihomoConfigDocument) {
        document.rules = self
            .rules
            .iter()
            .map(RuleSettings::to_config_line)
            .collect();
        document.sub_rules = self
            .sub_rules
            .iter()
            .map(|(name, rules)| {
                (
                    name.clone(),
                    rules.iter().map(RuleSettings::to_config_line).collect(),
                )
            })
            .collect();
    }

    /// 套用 `/rules/disable` 这类运行态状态。该接口只按主规则索引工作，重启后由 mihomo 清空。
    pub fn apply_runtime_disable_overlay(&mut self, overlay: &RuleDisableOverlay) {
        for rule in &mut self.rules {
            rule.disable.temporary = overlay.disabled.get(&rule.index).copied();
        }
    }
}

/// 规则所在范围。范围和索引用于 UI 定位，也用于解释 `/rules/disable` 的主规则索引语义。
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuleScope {
    Rules,
    SubRules(String),
}

/// 单条规则的领域表示。未识别或格式不完整的规则会落入 `RuleBody::Raw`，保证原文可写回。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct RuleSettings {
    pub scope: RuleScope,
    pub index: usize,
    pub body: RuleBody,
    pub disable: RuleDisableState,
}

impl RuleSettings {
    pub fn from_line(scope: RuleScope, index: usize, line: &RuleLine) -> Self {
        Self {
            scope,
            index,
            body: RuleBody::parse(&line.raw),
            disable: RuleDisableState::default(),
        }
    }

    pub fn structured(scope: RuleScope, index: usize, rule: StructuredRule) -> Self {
        Self {
            scope,
            index,
            body: RuleBody::Structured(rule),
            disable: RuleDisableState::default(),
        }
    }

    pub fn to_config_line(&self) -> RuleLine {
        RuleLine {
            raw: self.body.format(),
        }
    }

    pub fn target_policy(&self) -> Option<&str> {
        match &self.body {
            RuleBody::Structured(rule) => match &rule.target {
                RuleTarget::Policy(policy) => Some(policy.as_str()),
                RuleTarget::SubRule(_) => None,
            },
            RuleBody::Raw(_) => None,
        }
    }
}

impl Default for RuleSettings {
    fn default() -> Self {
        Self {
            scope: RuleScope::Rules,
            index: 0,
            body: RuleBody::Raw(RawRule::default()),
            disable: RuleDisableState::default(),
        }
    }
}

/// 配置态禁用与运行态禁用分离保存。当前 mihomo YAML 规则没有统一的禁用字段，
/// `configured` 为后续导入注释禁用规则预留；`temporary` 只来自 `/rules/disable`。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct RuleDisableState {
    pub configured: bool,
    pub temporary: Option<bool>,
}

impl RuleDisableState {
    pub fn effective(&self) -> bool {
        self.temporary.unwrap_or(self.configured)
    }
}

/// 运行态禁用覆盖。key 是主 `rules` 中的 0 基索引，不适用于 `sub-rules`。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuleDisableOverlay {
    pub disabled: BTreeMap<usize, bool>,
}

impl RuleDisableOverlay {
    pub fn new(disabled: impl IntoIterator<Item = (usize, bool)>) -> Self {
        Self {
            disabled: disabled.into_iter().collect(),
        }
    }

    pub fn from_rules_response(response: &RulesResponse) -> Self {
        let disabled = response
            .rules
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                runtime_disabled_from_value(value).map(|disabled| (index, disabled))
            })
            .collect();
        Self { disabled }
    }

    pub fn to_patch(&self) -> RuleDisablePatch {
        RuleDisablePatch::from_iter(
            self.disabled
                .iter()
                .map(|(index, disabled)| (*index, *disabled)),
        )
    }
}

/// `/rules/disable` 的请求体。mihomo 要求 JSON object 的 key 为字符串形式的规则索引。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RuleDisablePatch {
    pub states: BTreeMap<String, bool>,
}

impl RuleDisablePatch {
    pub fn from_iter(states: impl IntoIterator<Item = (usize, bool)>) -> Self {
        Self {
            states: states
                .into_iter()
                .map(|(index, disabled)| (index.to_string(), disabled))
                .collect(),
        }
    }
}

/// 解析后的规则正文。`Raw` 分支用于未来 mihomo 规则类型和用户手写的非标准行。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuleBody {
    Structured(StructuredRule),
    Raw(RawRule),
}

impl RuleBody {
    pub fn parse(raw: &str) -> Self {
        StructuredRule::parse(raw)
            .map(Self::Structured)
            .unwrap_or_else(|| {
                Self::Raw(RawRule {
                    raw: raw.to_string(),
                    segments: split_rule_segments(raw)
                        .into_iter()
                        .map(|segment| segment.trim().to_string())
                        .collect(),
                })
            })
    }

    pub fn format(&self) -> String {
        match self {
            Self::Structured(rule) => rule.format(),
            Self::Raw(raw) => raw.raw.clone(),
        }
    }
}

impl Default for RuleBody {
    fn default() -> Self {
        Self::Raw(RawRule::default())
    }
}

/// 结构化规则。`parameters` 保存匹配参数，`target` 保存最终策略或子规则名，`options` 保存
/// `no-resolve` 等附加参数，避免后续 UI 只编辑策略时把尾部参数丢掉。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct StructuredRule {
    pub rule_type: RuleType,
    pub parameters: Vec<String>,
    pub target: RuleTarget,
    pub options: Vec<String>,
}

impl StructuredRule {
    pub fn new(
        rule_type: RuleType,
        parameters: Vec<String>,
        target: RuleTarget,
        options: Vec<String>,
    ) -> Self {
        Self {
            rule_type,
            parameters,
            target,
            options,
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let parts = split_rule_segments(raw)
            .into_iter()
            .map(|segment| segment.trim().to_string())
            .collect::<Vec<_>>();
        if parts.iter().any(|part| part.is_empty()) {
            return None;
        }

        let rule_type = RuleType::from_token(parts.first()?);
        if matches!(rule_type, RuleType::Other(_)) {
            return None;
        }

        match rule_type {
            RuleType::Match => {
                let policy = parts.get(1)?.clone();
                Some(Self::new(
                    rule_type,
                    Vec::new(),
                    RuleTarget::Policy(policy),
                    parts.iter().skip(2).cloned().collect(),
                ))
            }
            RuleType::SubRule => {
                if parts.len() < 3 {
                    return None;
                }
                Some(Self::new(
                    rule_type,
                    parts[1..parts.len() - 1].to_vec(),
                    RuleTarget::SubRule(parts.last()?.clone()),
                    Vec::new(),
                ))
            }
            _ => {
                if parts.len() < 3 {
                    return None;
                }
                Some(Self::new(
                    rule_type,
                    vec![parts[1].clone()],
                    RuleTarget::Policy(parts[2].clone()),
                    parts.iter().skip(3).cloned().collect(),
                ))
            }
        }
    }

    pub fn format(&self) -> String {
        let mut parts = Vec::with_capacity(2 + self.parameters.len() + self.options.len());
        parts.push(self.rule_type.as_str().to_string());
        parts.extend(self.parameters.iter().cloned());
        parts.push(self.target.name().to_string());
        parts.extend(self.options.iter().cloned());
        parts.join(",")
    }
}

/// 常见 mihomo 规则类型。`Other` 只用于构造新规则时的类型承载，解析未知类型时走 `Raw`。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuleType {
    Domain,
    DomainSuffix,
    DomainKeyword,
    DomainRegex,
    DomainWildcard,
    Geosite,
    Geoip,
    IpCidr,
    IpCidr6,
    IpAsn,
    ProcessName,
    ProcessPath,
    Network,
    DstPort,
    SrcPort,
    SrcIpCidr,
    RuleSet,
    SubRule,
    Match,
    Other(String),
}

impl RuleType {
    pub fn from_token(token: &str) -> Self {
        match normalize_rule_type(token).as_str() {
            "DOMAIN" => Self::Domain,
            "DOMAIN-SUFFIX" => Self::DomainSuffix,
            "DOMAIN-KEYWORD" => Self::DomainKeyword,
            "DOMAIN-REGEX" => Self::DomainRegex,
            "DOMAIN-WILDCARD" => Self::DomainWildcard,
            "GEOSITE" => Self::Geosite,
            "GEOIP" => Self::Geoip,
            "IP-CIDR" => Self::IpCidr,
            "IP-CIDR6" => Self::IpCidr6,
            "IP-ASN" => Self::IpAsn,
            "PROCESS-NAME" => Self::ProcessName,
            "PROCESS-PATH" => Self::ProcessPath,
            "NETWORK" => Self::Network,
            "DST-PORT" => Self::DstPort,
            "SRC-PORT" => Self::SrcPort,
            "SRC-IP-CIDR" => Self::SrcIpCidr,
            "RULE-SET" | "RULESET" => Self::RuleSet,
            "SUB-RULE" | "SUBRULE" => Self::SubRule,
            "MATCH" => Self::Match,
            _ => Self::Other(token.trim().to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Domain => "DOMAIN",
            Self::DomainSuffix => "DOMAIN-SUFFIX",
            Self::DomainKeyword => "DOMAIN-KEYWORD",
            Self::DomainRegex => "DOMAIN-REGEX",
            Self::DomainWildcard => "DOMAIN-WILDCARD",
            Self::Geosite => "GEOSITE",
            Self::Geoip => "GEOIP",
            Self::IpCidr => "IP-CIDR",
            Self::IpCidr6 => "IP-CIDR6",
            Self::IpAsn => "IP-ASN",
            Self::ProcessName => "PROCESS-NAME",
            Self::ProcessPath => "PROCESS-PATH",
            Self::Network => "NETWORK",
            Self::DstPort => "DST-PORT",
            Self::SrcPort => "SRC-PORT",
            Self::SrcIpCidr => "SRC-IP-CIDR",
            Self::RuleSet => "RULE-SET",
            Self::SubRule => "SUB-RULE",
            Self::Match => "MATCH",
            Self::Other(value) => value.as_str(),
        }
    }
}

impl Default for RuleType {
    fn default() -> Self {
        Self::Other(String::new())
    }
}

/// 规则目标。普通规则最终选择策略；`SUB-RULE` 的目标是子规则集名称。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "name", rename_all = "kebab-case")]
pub enum RuleTarget {
    Policy(String),
    SubRule(String),
}

impl RuleTarget {
    pub fn name(&self) -> &str {
        match self {
            Self::Policy(name) | Self::SubRule(name) => name,
        }
    }
}

impl Default for RuleTarget {
    fn default() -> Self {
        Self::Policy(String::new())
    }
}

/// 原始规则行。`segments` 只用于 UI 预览和排错，写回始终使用 `raw`。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct RawRule {
    pub raw: String,
    pub segments: Vec<String>,
}

fn runtime_disabled_from_value(value: &serde_json::Value) -> Option<bool> {
    value
        .get("disabled")
        .or_else(|| value.get("disable"))
        .and_then(serde_json::Value::as_bool)
}

fn normalize_rule_type(token: &str) -> String {
    token.trim().replace('_', "-").to_ascii_uppercase()
}

/// 规则中 AND/OR/NOT 或 SUB-RULE 条件表达式可能在括号内再包含逗号，顶层分割必须避开括号内部。
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

    fn docs_collection() -> RuleCollection {
        let document = ConfigDocument::parse(include_str!("../../../../docs/config.yaml"))
            .expect("docs/config.yaml should parse")
            .typed;
        RuleCollection::from_document(&document)
    }

    fn structured(rule: &RuleSettings) -> &StructuredRule {
        match &rule.body {
            RuleBody::Structured(rule) => rule,
            RuleBody::Raw(raw) => panic!("rule should be structured: {}", raw.raw),
        }
    }

    #[test]
    fn parses_docs_rules_and_sub_rules() {
        let collection = docs_collection();

        let rule_set = structured(&collection.rules[0]);
        assert_eq!(rule_set.rule_type, RuleType::RuleSet);
        assert_eq!(rule_set.parameters, vec!["rule1"]);
        assert_eq!(rule_set.target, RuleTarget::Policy("REJECT".to_string()));

        let sub_rule = structured(&collection.rules[8]);
        assert_eq!(sub_rule.rule_type, RuleType::SubRule);
        assert_eq!(
            sub_rule.parameters,
            vec!["(OR,((NETWORK,TCP),(NETWORK,UDP)))"]
        );
        assert_eq!(
            sub_rule.target,
            RuleTarget::SubRule("sub-rule-name1".to_string())
        );

        let nested = collection
            .sub_rules
            .get("sub-rule-name2")
            .expect("sub-rule-name2 should exist");
        assert_eq!(structured(&nested[0]).rule_type, RuleType::IpCidr);
        assert_eq!(structured(&nested[2]).rule_type, RuleType::Domain);
    }

    #[test]
    fn formats_structured_rules_stably() {
        let samples = [
            "DOMAIN,google.com,Proxy",
            "DOMAIN-SUFFIX,example.com,DIRECT",
            "DOMAIN-KEYWORD,google,ss1",
            "GEOSITE,cn,DIRECT",
            "GEOIP,CN,DIRECT,no-resolve",
            "IP-CIDR,1.1.1.1/32,REJECT,no-resolve",
            "PROCESS-NAME,Telegram.exe,Proxy",
            "MATCH,DIRECT",
            "RULE-SET,rule1,REJECT",
            "SUB-RULE,(AND,((NETWORK,UDP))),dns-sub",
        ];

        for sample in samples {
            let body = RuleBody::parse(sample);
            assert_eq!(body.format(), sample);
            assert_eq!(RuleBody::parse(&body.format()).format(), sample);
        }
    }

    #[test]
    fn preserves_unknown_and_incomplete_rules_as_raw() {
        let unknown = RuleBody::parse("FUTURE-RULE,a,b,c");
        assert!(matches!(unknown, RuleBody::Raw(_)));
        assert_eq!(unknown.format(), "FUTURE-RULE,a,b,c");

        let incomplete = RuleBody::parse("DOMAIN-SUFFIX,example.com");
        assert!(matches!(incomplete, RuleBody::Raw(_)));
        assert_eq!(incomplete.format(), "DOMAIN-SUFFIX,example.com");
    }

    #[test]
    fn writes_collection_back_without_losing_sub_rules() {
        let mut document = ConfigDocument::parse(
            r#"
rules:
  - DOMAIN-SUFFIX,example.com,DIRECT
  - SUB-RULE,(OR,((NETWORK,TCP),(NETWORK,UDP))),nested
sub-rules:
  nested:
    - IP-CIDR,1.1.1.1/32,REJECT,no-resolve
"#,
        )
        .expect("rules fixture should parse")
        .typed;
        let mut collection = RuleCollection::from_document(&document);
        let first = structured(&collection.rules[0]).clone();
        collection.rules[0] = RuleSettings::structured(
            RuleScope::Rules,
            0,
            StructuredRule::new(
                first.rule_type,
                first.parameters,
                RuleTarget::Policy("Proxy".to_string()),
                first.options,
            ),
        );

        collection.apply_to_document(&mut document);

        assert_eq!(document.rules[0].raw, "DOMAIN-SUFFIX,example.com,Proxy");
        assert_eq!(
            document.rules[1].raw,
            "SUB-RULE,(OR,((NETWORK,TCP),(NETWORK,UDP))),nested"
        );
        assert_eq!(
            document.sub_rules["nested"][0].raw,
            "IP-CIDR,1.1.1.1/32,REJECT,no-resolve"
        );
    }

    #[test]
    fn keeps_runtime_disable_state_out_of_config_writeback() {
        let mut document = ConfigDocument::parse(
            r#"
rules:
  - MATCH,DIRECT
  - DOMAIN,example.com,Proxy
"#,
        )
        .expect("rules fixture should parse")
        .typed;
        let original = document.rules.clone();
        let mut collection = RuleCollection::from_document(&document);

        collection.apply_runtime_disable_overlay(&RuleDisableOverlay::new([(1, true)]));
        assert!(!collection.rules[0].disable.effective());
        assert!(collection.rules[1].disable.effective());

        collection.apply_to_document(&mut document);
        assert_eq!(document.rules, original);

        let patch = RuleDisableOverlay::new([(0, false), (1, true)]).to_patch();
        let json = serde_json::to_value(&patch).expect("patch should serialize");
        assert_eq!(json["0"], false);
        assert_eq!(json["1"], true);
    }

    #[test]
    fn reads_runtime_disable_overlay_from_rules_response() {
        let response = RulesResponse {
            rules: vec![
                serde_json::json!({"type": "Match", "disabled": false}),
                serde_json::json!({"type": "Domain", "disable": true}),
                serde_json::json!({"type": "RuleSet"}),
            ],
            extra: BTreeMap::new(),
        };

        let overlay = RuleDisableOverlay::from_rules_response(&response);
        assert_eq!(overlay.disabled.get(&0), Some(&false));
        assert_eq!(overlay.disabled.get(&1), Some(&true));
        assert!(!overlay.disabled.contains_key(&2));
    }
}
