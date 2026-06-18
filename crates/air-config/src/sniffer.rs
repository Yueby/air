//! sniffer 域名嗅探配置的领域编辑模型。
//!
//! YAML 层只保留 mihomo 的原始结构；本模块负责把 `sniffer` section 拆成 GUI 易展示的
//! 常规开关、协议嗅探和域名规则。当前设置页只暴露 mihomo 文档中明确支持的
//! HTTP/TLS/QUIC 三类协议，未知字段仍由 YAML 扩展映射兜底保留。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

use super::model::{MihomoConfigDocument, SnifferConfig, StringValueMap};
use super::{ConfigDiagnostic, ConfigDiagnosticSeverity};

/// 设置页明确支持的协议配置分组。
pub const WELL_KNOWN_SNIFFER_PROTOCOLS: &[&str] = &["HTTP", "TLS", "QUIC"];

/// sniffer section 的领域设置。
///
/// 写回时只覆盖已建模字段，`SnifferConfig.extensions` 与协议内部未知字段会继续保留。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SnifferConfigSettings {
    pub enable: Option<bool>,
    pub force_dns_mapping: Option<bool>,
    pub parse_pure_ip: Option<bool>,
    pub override_destination: Option<bool>,
    pub protocols: Vec<SnifferProtocolSettings>,
    pub force_domain: Vec<String>,
    pub skip_domain: Vec<String>,
    pub skip_src_address: Vec<String>,
    pub skip_dst_address: Vec<String>,
    pub sniffing: Vec<String>,
    pub port_whitelist: Vec<String>,
}

impl SnifferConfigSettings {
    pub fn from_document(document: &MihomoConfigDocument) -> Self {
        document
            .sniffer
            .as_ref()
            .map(Self::from_config)
            .unwrap_or_default()
    }

    pub fn from_config(config: &SnifferConfig) -> Self {
        Self {
            enable: config.enable,
            force_dns_mapping: config.force_dns_mapping,
            parse_pure_ip: config.parse_pure_ip,
            override_destination: config.override_destination,
            protocols: config
                .sniff
                .iter()
                .map(|(name, value)| SnifferProtocolSettings::from_yaml(name, value))
                .collect(),
            force_domain: config.force_domain.clone(),
            skip_domain: config.skip_domain.clone(),
            skip_src_address: config.skip_src_address.clone(),
            skip_dst_address: config.skip_dst_address.clone(),
            sniffing: config.sniffing.clone(),
            port_whitelist: config.port_whitelist.clone(),
        }
    }

    /// 将 sniffer 设置写回完整文档，保留 DNS/TUN 等其他 section 和 sniffer 未知扩展字段。
    pub fn apply_to_document(&self, document: &mut MihomoConfigDocument) {
        let mut sniffer = document.sniffer.clone().unwrap_or_default();
        sniffer.enable = self.enable;
        sniffer.force_dns_mapping = self.force_dns_mapping;
        sniffer.parse_pure_ip = self.parse_pure_ip;
        sniffer.override_destination = self.override_destination;
        sniffer.sniff = self
            .protocols
            .iter()
            .filter_map(|protocol| {
                let name = protocol.name.trim();
                (!name.is_empty()).then(|| (name.to_string(), protocol.to_yaml_value()))
            })
            .collect();
        sniffer.force_domain = normalized_strings(&self.force_domain);
        sniffer.skip_domain = normalized_strings(&self.skip_domain);
        sniffer.skip_src_address = normalized_strings(&self.skip_src_address);
        sniffer.skip_dst_address = normalized_strings(&self.skip_dst_address);
        sniffer.sniffing = normalized_strings(&self.sniffing);
        sniffer.port_whitelist = normalized_strings(&self.port_whitelist);

        if self.is_empty() && sniffer.extensions.is_empty() {
            document.sniffer = None;
        } else {
            document.sniffer = Some(sniffer);
        }
    }

    pub fn validate(&self) -> Vec<ConfigDiagnostic> {
        let mut validator = SnifferConfigValidator::new(self);
        validator.validate();
        validator.diagnostics
    }

    fn is_empty(&self) -> bool {
        self.enable.is_none()
            && self.force_dns_mapping.is_none()
            && self.parse_pure_ip.is_none()
            && self.override_destination.is_none()
            && self.protocols.is_empty()
            && self.force_domain.is_empty()
            && self.skip_domain.is_empty()
            && self.skip_src_address.is_empty()
            && self.skip_dst_address.is_empty()
            && self.sniffing.is_empty()
            && self.port_whitelist.is_empty()
    }
}

/// 单个 sniffer 协议的编辑模型。
///
/// `passthrough` 用于保存 null、标量或当前版本未理解的协议写法；只要用户开始编辑 ports
/// 或 override-destination，就会按映射结构规范化写回。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SnifferProtocolSettings {
    pub name: String,
    pub ports: Vec<String>,
    pub override_destination: Option<bool>,
    pub extensions: StringValueMap,
    pub passthrough: Option<Value>,
}

impl SnifferProtocolSettings {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    fn from_yaml(name: &str, value: &Value) -> Self {
        let Value::Mapping(mapping) = value else {
            return Self {
                name: name.to_string(),
                passthrough: Some(value.clone()),
                ..Default::default()
            };
        };

        let mut protocol = Self::new(name);
        for (key, value) in mapping {
            let Some(key) = key.as_str() else {
                continue;
            };
            match key {
                "ports" => protocol.ports = ports_from_value(value),
                "override-destination" => protocol.override_destination = value.as_bool(),
                _ => {
                    protocol.extensions.insert(key.to_string(), value.clone());
                }
            }
        }
        protocol
    }

    fn to_yaml_value(&self) -> Value {
        if self.ports.is_empty()
            && self.override_destination.is_none()
            && self.extensions.is_empty()
            && let Some(value) = &self.passthrough
        {
            return value.clone();
        }

        let mut mapping = Mapping::new();
        for (key, value) in &self.extensions {
            mapping.insert(Value::String(key.clone()), value.clone());
        }
        if !self.ports.is_empty() {
            mapping.insert(
                Value::String("ports".to_string()),
                Value::Sequence(
                    normalized_strings(&self.ports)
                        .into_iter()
                        .map(Value::String)
                        .collect(),
                ),
            );
        }
        if let Some(override_destination) = self.override_destination {
            mapping.insert(
                Value::String("override-destination".to_string()),
                Value::Bool(override_destination),
            );
        }

        if mapping.is_empty() {
            Value::Null
        } else {
            Value::Mapping(mapping)
        }
    }
}

/// GUI 表单 view model：按页面自然分区组织，避免 UI 组件理解 YAML 细节。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SnifferConfigFormViewModel {
    pub general: SnifferGeneralViewModel,
    pub protocols: SnifferProtocolGroupViewModel,
    pub domain_rules: SnifferDomainRulesViewModel,
    pub legacy: SnifferLegacyViewModel,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

impl From<&SnifferConfigSettings> for SnifferConfigFormViewModel {
    fn from(settings: &SnifferConfigSettings) -> Self {
        Self {
            general: SnifferGeneralViewModel {
                enable: SnifferBooleanFormValue::from_option(settings.enable),
                force_dns_mapping: SnifferBooleanFormValue::from_option(settings.force_dns_mapping),
                parse_pure_ip: SnifferBooleanFormValue::from_option(settings.parse_pure_ip),
                override_destination: SnifferBooleanFormValue::from_option(
                    settings.override_destination,
                ),
            },
            protocols: SnifferProtocolGroupViewModel {
                well_known_protocols: WELL_KNOWN_SNIFFER_PROTOCOLS
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                items: settings
                    .protocols
                    .iter()
                    .map(SnifferProtocolViewModel::from)
                    .collect(),
            },
            domain_rules: SnifferDomainRulesViewModel {
                force_domain: settings.force_domain.clone(),
                skip_domain: settings.skip_domain.clone(),
                skip_src_address: settings.skip_src_address.clone(),
                skip_dst_address: settings.skip_dst_address.clone(),
            },
            legacy: SnifferLegacyViewModel {
                sniffing: settings.sniffing.clone(),
                port_whitelist: settings.port_whitelist.clone(),
            },
            diagnostics: settings.validate(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SnifferGeneralViewModel {
    pub enable: SnifferBooleanFormValue,
    pub force_dns_mapping: SnifferBooleanFormValue,
    pub parse_pure_ip: SnifferBooleanFormValue,
    pub override_destination: SnifferBooleanFormValue,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SnifferProtocolGroupViewModel {
    pub well_known_protocols: Vec<String>,
    pub items: Vec<SnifferProtocolViewModel>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SnifferProtocolViewModel {
    pub name: String,
    pub ports: Vec<String>,
    pub override_destination: SnifferBooleanFormValue,
    pub has_advanced_fields: bool,
}

impl From<&SnifferProtocolSettings> for SnifferProtocolViewModel {
    fn from(protocol: &SnifferProtocolSettings) -> Self {
        Self {
            name: protocol.name.clone(),
            ports: protocol.ports.clone(),
            override_destination: SnifferBooleanFormValue::from_option(
                protocol.override_destination,
            ),
            has_advanced_fields: !protocol.extensions.is_empty()
                || protocol
                    .passthrough
                    .as_ref()
                    .is_some_and(|value| !value.is_null()),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SnifferDomainRulesViewModel {
    pub force_domain: Vec<String>,
    pub skip_domain: Vec<String>,
    pub skip_src_address: Vec<String>,
    pub skip_dst_address: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SnifferLegacyViewModel {
    pub sniffing: Vec<String>,
    pub port_whitelist: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SnifferBooleanFormValue {
    pub value: bool,
    pub configured: bool,
}

impl SnifferBooleanFormValue {
    fn from_option(value: Option<bool>) -> Self {
        Self {
            value: value.unwrap_or_default(),
            configured: value.is_some(),
        }
    }
}

struct SnifferConfigValidator<'a> {
    settings: &'a SnifferConfigSettings,
    diagnostics: Vec<ConfigDiagnostic>,
}

impl<'a> SnifferConfigValidator<'a> {
    fn new(settings: &'a SnifferConfigSettings) -> Self {
        Self {
            settings,
            diagnostics: Vec::new(),
        }
    }

    fn validate(&mut self) {
        self.validate_protocols();
        self.validate_ports("sniffer.port-whitelist", &self.settings.port_whitelist);
        self.validate_domain_list("sniffer.force-domain", &self.settings.force_domain);
        self.validate_non_empty_list(
            "sniffer.skip-domain",
            &self.settings.skip_domain,
            "域名规则",
        );
        self.validate_cidr_list("sniffer.skip-src-address", &self.settings.skip_src_address);
        self.validate_cidr_list("sniffer.skip-dst-address", &self.settings.skip_dst_address);
        self.validate_legacy_fields();
    }

    fn validate_protocols(&mut self) {
        let mut seen = BTreeMap::<String, usize>::new();
        for (index, protocol) in self.settings.protocols.iter().enumerate() {
            let name = protocol.name.trim();
            let path = format!("sniffer.sniff[{index}]");
            if name.is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}.name"),
                    "嗅探协议名称不能为空",
                    Some("请填写 HTTP、TLS、QUIC 或 mihomo 支持的新协议名称。".to_string()),
                ));
            } else if !is_protocol_name(name) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}.name"),
                    format!("嗅探协议名称 `{name}` 格式无效"),
                    Some("协议名称只能包含字母、数字、下划线和短横线。".to_string()),
                ));
            } else if !WELL_KNOWN_SNIFFER_PROTOCOLS
                .iter()
                .any(|protocol| protocol.eq_ignore_ascii_case(name))
            {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}.name"),
                    format!("嗅探协议 `{name}` 不受支持"),
                    Some("当前仅支持 HTTP、TLS、QUIC。".to_string()),
                ));
            }

            let normalized = name.to_ascii_lowercase();
            if let Some(first_index) = seen.insert(normalized, index) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}.name"),
                    format!("嗅探协议 `{name}` 与第 {first_index} 项重复"),
                    Some("请合并重复协议，避免写回时互相覆盖。".to_string()),
                ));
            }

            self.validate_ports(&format!("{path}.ports"), &protocol.ports);
        }
    }

    fn validate_ports(&mut self, path: &str, values: &[String]) {
        for (index, value) in values.iter().enumerate() {
            let value = value.trim();
            if value.is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}[{index}]"),
                    "端口条目不能为空",
                    Some("请删除空条目，或填写 80、443、8080-8880 这类端口或范围。".to_string()),
                ));
                continue;
            }

            if let Err(message) = validate_port_range(value) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}[{index}]"),
                    message,
                    Some(
                        "端口必须位于 1-65535；范围写法为 start-end，且 start 不能大于 end。"
                            .to_string(),
                    ),
                ));
            }
        }
    }

    fn validate_domain_list(&mut self, path: &str, values: &[String]) {
        for (index, value) in values.iter().enumerate() {
            let value = value.trim();
            if value.is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}[{index}]"),
                    "域名规则不能为空",
                    Some(
                        "请删除空条目，或填写 example.com、*.example.com、+.example.com。"
                            .to_string(),
                    ),
                ));
                continue;
            }

            if let Err(message) = validate_domain_pattern(value) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}[{index}]"),
                    message,
                    Some(
                        "支持普通域名、*.example.com 和 +.example.com；不要包含协议、路径或空格。"
                            .to_string(),
                    ),
                ));
            }
        }
    }

    fn validate_non_empty_list(&mut self, path: &str, values: &[String], item_name: &str) {
        for (index, value) in values.iter().enumerate() {
            if value.trim().is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}[{index}]"),
                    format!("{item_name}不能为空"),
                    Some("请删除空条目，或填写有效内容。".to_string()),
                ));
            }
        }
    }

    fn validate_cidr_list(&mut self, path: &str, values: &[String]) {
        for (index, value) in values.iter().enumerate() {
            let value = value.trim();
            if value.is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}[{index}]"),
                    "IP 段不能为空",
                    Some("请删除空条目，或填写 192.168.0.3/32 这类 CIDR。".to_string()),
                ));
                continue;
            }
            if let Err(message) = validate_cidr(value) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}[{index}]"),
                    message,
                    Some("请填写合法的 IPv4/IPv6 CIDR。".to_string()),
                ));
            }
        }
    }

    fn validate_legacy_fields(&mut self) {
        if !self.settings.protocols.is_empty()
            && (!self.settings.sniffing.is_empty() || !self.settings.port_whitelist.is_empty())
        {
            self.diagnostics.push(ConfigDiagnostic::info(
                "sniffer.sniffing",
                "sniffing 和 port-whitelist 是旧字段，存在 sniffer.sniff 时通常会被 mihomo 忽略",
                Some("建议优先在 sniffer.sniff 中为每个协议配置 ports。".to_string()),
            ));
        }
    }
}

fn ports_from_value(value: &Value) -> Vec<String> {
    match value {
        Value::Sequence(values) => values.iter().filter_map(port_value_to_string).collect(),
        value => port_value_to_string(value).into_iter().collect(),
    }
}

fn port_value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number.to_string()),
        Value::String(value) => Some(value.trim().to_string()).filter(|value| !value.is_empty()),
        _ => None,
    }
}

fn validate_port_range(value: &str) -> Result<(), String> {
    let (start, end) = if let Some((start, end)) = value.split_once('-') {
        (parse_port(start, value)?, parse_port(end, value)?)
    } else {
        let port = parse_port(value, value)?;
        (port, port)
    };

    if start > end {
        return Err(format!("端口范围 `{value}` 的起始端口大于结束端口"));
    }
    Ok(())
}

fn parse_port(part: &str, original: &str) -> Result<u32, String> {
    let port = part
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("端口 `{original}` 不是合法整数或范围"))?;
    if (1..=65_535).contains(&port) {
        Ok(port)
    } else {
        Err(format!("端口 `{original}` 不在有效范围 1-65535 内"))
    }
}

fn validate_domain_pattern(value: &str) -> Result<(), String> {
    if value.contains("://") || value.contains('/') || value.chars().any(char::is_whitespace) {
        return Err(format!("域名规则 `{value}` 不能包含协议、路径或空白字符"));
    }

    let domain = value
        .strip_prefix("+.")
        .or_else(|| value.strip_prefix("*."))
        .or_else(|| value.strip_prefix('.'))
        .unwrap_or(value);

    if domain == "*" {
        return Ok(());
    }

    if domain.is_empty() || domain.contains("..") {
        return Err(format!("域名规则 `{value}` 的域名部分为空或包含连续点号"));
    }

    for label in domain.split('.') {
        if label.is_empty() {
            return Err(format!("域名规则 `{value}` 包含空标签"));
        }
        if label == "*" {
            continue;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(format!("域名规则 `{value}` 的标签不能以短横线开头或结尾"));
        }
        if !label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            return Err(format!("域名规则 `{value}` 包含不支持的字符"));
        }
    }

    if value.contains('*') && !(value == "*" || value.starts_with("*.") || domain == "*") {
        return Err(format!("域名规则 `{value}` 的通配符只能作为完整标签使用"));
    }

    Ok(())
}

fn validate_cidr(value: &str) -> Result<(), String> {
    let (ip, prefix) = value
        .split_once('/')
        .ok_or_else(|| format!("IP 段 `{value}` 缺少 CIDR 前缀"))?;
    let ip = ip
        .trim()
        .parse::<std::net::IpAddr>()
        .map_err(|_| format!("IP 段 `{value}` 的地址不是有效 IP"))?;
    let prefix = prefix
        .trim()
        .parse::<u8>()
        .map_err(|_| format!("IP 段 `{value}` 的前缀长度不是整数"))?;
    let max_prefix = if ip.is_ipv4() { 32 } else { 128 };
    if prefix > max_prefix {
        return Err(format!("IP 段 `{value}` 的前缀长度超过 {max_prefix}"));
    }
    Ok(())
}

fn is_protocol_name(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn normalized_strings(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn has_sniffer_error(diagnostics: &[ConfigDiagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ConfigDiagnosticSeverity::Error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use air_config::ConfigDocument;

    fn docs_document() -> ConfigDocument {
        ConfigDocument::parse(include_str!("../../../docs/config.yaml"))
            .expect("docs/config.yaml should parse")
    }

    fn has_diagnostic_at(
        diagnostics: &[ConfigDiagnostic],
        severity: ConfigDiagnosticSeverity,
        path: &str,
    ) -> bool {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == severity && diagnostic.path == path)
    }

    #[test]
    fn extracts_sniffer_fields_from_docs_config() {
        let document = docs_document();
        let settings = SnifferConfigSettings::from_document(&document.typed);

        assert_eq!(settings.enable, Some(false));
        assert_eq!(settings.override_destination, Some(false));
        assert_eq!(settings.force_domain, vec!["+.v2ex.com"]);
        assert_eq!(settings.sniffing, vec!["tls", "http"]);
        assert_eq!(settings.port_whitelist, vec!["80", "443"]);

        let http = settings
            .protocols
            .iter()
            .find(|protocol| protocol.name == "HTTP")
            .expect("sample should contain HTTP protocol");
        assert_eq!(http.ports, vec!["80", "8080-8880"]);
        assert_eq!(http.override_destination, Some(true));
    }

    #[test]
    fn preserves_extensible_protocols_and_protocol_extensions() {
        let document = ConfigDocument::parse(
            r#"
sniffer:
  enable: true
  sniff:
    FOO:
      ports: [1234]
      future-option: keep
    BAR:
"#,
        )
        .expect("sniffer config should parse");

        let settings = SnifferConfigSettings::from_document(&document.typed);
        let foo = settings
            .protocols
            .iter()
            .find(|protocol| protocol.name == "FOO")
            .expect("unknown protocol should be retained");
        assert_eq!(foo.ports, vec!["1234"]);
        assert_eq!(
            foo.extensions.get("future-option"),
            Some(&Value::String("keep".to_string()))
        );

        let mut written = document.typed.clone();
        settings.apply_to_document(&mut written);
        assert!(written.sniffer.unwrap().sniff.contains_key("BAR"));
    }

    #[test]
    fn writes_back_sniffer_without_touching_dns_or_tun() {
        let mut document = docs_document().typed;
        let original_dns = document.dns.clone();
        let original_tun = document.tun.clone();
        let original_extensions = document.sniffer.as_ref().unwrap().extensions.clone();

        let mut settings = SnifferConfigSettings::from_document(&document);
        settings.enable = Some(true);
        settings.force_domain.push("*.example.com".into());
        settings
            .protocols
            .push(SnifferProtocolSettings::new("CUSTOM"));
        settings.apply_to_document(&mut document);

        let sniffer = document
            .sniffer
            .as_ref()
            .expect("sniffer should remain present");
        assert_eq!(sniffer.enable, Some(true));
        assert!(sniffer.force_domain.contains(&"*.example.com".to_string()));
        assert!(sniffer.sniff.contains_key("CUSTOM"));
        assert_eq!(sniffer.extensions, original_extensions);
        assert_eq!(document.dns, original_dns);
        assert_eq!(document.tun, original_tun);
    }

    #[test]
    fn validates_ports_domains_and_protocol_names() {
        let settings = SnifferConfigSettings {
            protocols: vec![SnifferProtocolSettings {
                name: "BAD NAME".into(),
                ports: vec!["0".into(), "9000-8000".into()],
                ..Default::default()
            }],
            force_domain: vec!["https://example.com/path".into()],
            skip_domain: vec!["Mijia Cloud".into()],
            skip_src_address: vec!["192.168.0.999/32".into()],
            port_whitelist: vec!["65536".into()],
            ..Default::default()
        };

        let diagnostics = settings.validate();

        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "sniffer.sniff[0].name"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "sniffer.sniff[0].ports[0]"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "sniffer.sniff[0].ports[1]"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "sniffer.force-domain[0]"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "sniffer.skip-src-address[0]"
        ));
        assert!(!has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "sniffer.skip-domain[0]"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "sniffer.port-whitelist[0]"
        ));
        assert!(has_sniffer_error(&diagnostics));
    }

    #[test]
    fn prepares_grouped_form_view_model() {
        let settings = SnifferConfigSettings {
            enable: Some(true),
            override_destination: Some(true),
            protocols: vec![SnifferProtocolSettings {
                name: "HTTP".into(),
                ports: vec!["80".into()],
                override_destination: Some(false),
                ..Default::default()
            }],
            force_domain: vec!["+.v2ex.com".into()],
            port_whitelist: vec!["80".into()],
            ..Default::default()
        };

        let view_model = SnifferConfigFormViewModel::from(&settings);

        assert!(view_model.general.enable.value);
        assert!(view_model.general.override_destination.configured);
        assert_eq!(
            view_model.protocols.well_known_protocols,
            vec!["HTTP", "TLS", "QUIC"]
        );
        assert_eq!(view_model.protocols.items[0].ports, vec!["80"]);
        assert_eq!(view_model.domain_rules.force_domain, vec!["+.v2ex.com"]);
        assert!(has_diagnostic_at(
            &view_model.diagnostics,
            ConfigDiagnosticSeverity::Info,
            "sniffer.sniffing"
        ));
    }

    #[test]
    fn accepts_numeric_port_whitelist_entries() {
        let document = ConfigDocument::parse(
            r#"
sniffer:
  port-whitelist:
    - 80
    - 8080-8880
"#,
        )
        .expect("numeric port whitelist should parse");

        let settings = SnifferConfigSettings::from_document(&document.typed);

        assert_eq!(settings.port_whitelist, vec!["80", "8080-8880"]);
        assert!(!has_sniffer_error(&settings.validate()));
    }
}
