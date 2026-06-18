//! mihomo 配置文档的可扩展模型。
//!
//! 这里的结构只承担“安全读写 YAML 文档”的职责，不做运行态校验。mihomo 的配置格式演进很快，
//! 因此每一层都保留 `extensions`，让暂未建模的字段在反序列化和再次序列化时仍然存在。

pub mod common;
pub mod dns;
pub mod global;
pub mod inbound;
pub mod provider;
pub mod proxy;
pub mod rule;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

pub use common::{
    Credential, ExtensionMap, OneOrManyStrings, StringValueMap, empty_map_if_null,
    empty_vec_if_null, string_vec_if_null,
};
pub use dns::*;
pub use global::*;
pub use inbound::*;
pub use provider::*;
pub use proxy::*;
pub use rule::*;

/// 顶层 mihomo 配置文档。
///
/// 字段分组遵循后续 GUI 的编辑边界：常用全局开关和端口属于用户可编辑区；
/// TUN、DNS、节点、规则等子域交给对应页面编辑；`extensions` 则承载只读或高级字段。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct MihomoConfigDocument {
    #[serde(flatten)]
    pub global: GlobalConfig,
    pub tls: Option<TlsConfig>,
    pub profile: Option<ProfileConfig>,
    pub tun: Option<TunConfig>,
    pub sniffer: Option<SnifferConfig>,
    pub dns: Option<DnsConfig>,
    pub tunnels: Vec<TunnelConfig>,
    pub proxies: Vec<ProxyNode>,
    #[serde(rename = "proxy-groups")]
    pub proxy_groups: Vec<ProxyGroup>,
    #[serde(rename = "proxy-providers")]
    pub proxy_providers: BTreeMap<String, ProxyProvider>,
    #[serde(rename = "rule-providers")]
    pub rule_providers: BTreeMap<String, RuleProvider>,
    pub rules: Vec<RuleLine>,
    #[serde(rename = "sub-rules")]
    pub sub_rules: BTreeMap<String, Vec<RuleLine>>,
    pub listeners: Vec<ListenerConfig>,

    /// 顶层未知字段用于保留 mihomo 新版本字段、用户手写扩展和当前任务未覆盖的高级配置。
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

impl MihomoConfigDocument {
    /// 判断文档中是否包含配置页尚未建模的顶层字段，供后续 UI 标记“高级/只读配置”。
    pub fn has_top_level_extensions(&self) -> bool {
        !self.extensions.is_empty()
    }

    /// 从 YAML 值构建强类型文档。错误直接来自 serde_yaml，调用方可在上层转换为应用诊断。
    pub fn from_value(value: Value) -> serde_yaml::Result<Self> {
        serde_yaml::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_sample_config_structure() {
        let yaml = include_str!("../../../docs/config.yaml");
        let document: MihomoConfigDocument =
            serde_yaml::from_str(yaml).expect("docs/config.yaml should match document model");

        assert_eq!(document.global.mixed_port, Some(10801));
        assert!(document.tun.is_some());
        assert!(document.sniffer.is_some());
        assert!(document.dns.is_some());
        assert!(!document.proxies.is_empty());
        assert!(!document.proxy_groups.is_empty());
        assert!(document.proxy_providers.contains_key("provider1"));
        assert!(document.rule_providers.contains_key("rule1"));
        assert!(!document.rules.is_empty());
        assert!(!document.listeners.is_empty());

        let socks = document
            .proxies
            .iter()
            .find(|proxy| proxy.name == "socks")
            .expect("sample should contain socks proxy");
        assert_eq!(socks.kind, ProxyKind::Socks5);
        assert_eq!(socks.server, Some(Value::String("server".to_string())));
    }

    #[test]
    fn preserve_unknown_fields_at_multiple_levels() {
        let yaml = r#"
mixed-port: 7890
future-top:
  nested: true
dns:
  enable: true
  future-dns: 1
proxies:
  - name: demo
    type: new-proto
    server: example.com
    port: 443
    future-proxy-field:
      - keep
"#;

        let document: MihomoConfigDocument =
            serde_yaml::from_str(yaml).expect("future fields should deserialize");

        assert!(document.extensions.contains_key("future-top"));
        assert_eq!(
            document
                .dns
                .as_ref()
                .expect("dns should be present")
                .extensions["future-dns"],
            Value::Number(1.into())
        );
        assert_eq!(
            document.proxies[0].kind,
            ProxyKind::Other("new-proto".to_string())
        );
        assert!(
            document.proxies[0]
                .extensions
                .contains_key("future-proxy-field")
        );

        let value = serde_yaml::to_value(&document).expect("document should serialize");
        let mapping = value.as_mapping().expect("document should be a map");
        assert!(mapping.contains_key(Value::String("future-top".to_string())));
    }
}
