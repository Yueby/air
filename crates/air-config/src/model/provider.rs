use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_yaml::Value;

use super::{ExtensionMap, ProxyNode, StringValueMap, empty_map_if_null};

/// 节点 Provider。http/file/inline 三类共享多数元数据，payload 只在 inline 中出现。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyProvider {
    #[serde(rename = "type")]
    pub kind: ProviderKind,
    pub url: Option<String>,
    pub interval: Option<u64>,
    pub path: Option<String>,
    pub proxy: Option<String>,
    pub size_limit: Option<u64>,
    #[serde(deserialize_with = "empty_map_if_null")]
    pub header: StringValueMap,
    pub health_check: Option<HealthCheckConfig>,
    pub override_config: Option<ProxyProviderOverride>,
    pub payload: Vec<ProxyNode>,
    pub dialer_proxy: Option<String>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct HealthCheckConfig {
    pub enable: Option<bool>,
    pub interval: Option<u64>,
    pub lazy: Option<bool>,
    pub url: Option<String>,
    pub expected_status: Option<Value>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

/// Provider 覆盖项会被应用到下载得到的节点上，字段集合与节点公共字段相近但允许继续扩展。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyProviderOverride {
    pub skip_cert_verify: Option<bool>,
    pub udp: Option<bool>,
    pub down: Option<String>,
    pub up: Option<String>,
    pub dialer_proxy: Option<String>,
    pub interface_name: Option<String>,
    pub routing_mark: Option<Value>,
    pub ip_version: Option<String>,
    pub additional_prefix: Option<String>,
    pub additional_suffix: Option<String>,
    pub proxy_name: Vec<NameRewriteRule>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct NameRewriteRule {
    pub pattern: String,
    pub target: String,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

/// 规则 Provider。inline 的 payload 和远程/文件 provider 的元数据共用同一模型。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct RuleProvider {
    #[serde(rename = "type")]
    pub kind: ProviderKind,
    pub behavior: Option<String>,
    pub format: Option<String>,
    pub interval: Option<u64>,
    pub path: Option<String>,
    pub url: Option<String>,
    pub proxy: Option<String>,
    pub size_limit: Option<u64>,
    pub payload: Vec<String>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    Http,
    File,
    Inline,
    Other(String),
}

impl ProviderKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Http => "http",
            Self::File => "file",
            Self::Inline => "inline",
            Self::Other(value) => value,
        }
    }
}

impl Default for ProviderKind {
    fn default() -> Self {
        Self::Other(String::new())
    }
}

impl Serialize for ProviderKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProviderKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "http" => Self::Http,
            "file" => Self::File,
            "inline" => Self::Inline,
            _ => Self::Other(value),
        })
    }
}

/// Provider 映射的类型别名，后续仓储层可沿用以表达名称索引。
pub type ProviderMap<T> = BTreeMap<String, T>;
