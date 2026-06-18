use serde::{Deserialize, Serialize};

use super::{ExtensionMap, StringValueMap, empty_map_if_null};

/// DNS 配置。
///
/// DNS 服务器条目允许 `IP`、`scheme://`、`rcode://`、带策略组后缀等多种字符串；策略值也可能是
/// 字符串或列表，所以保持为字符串列表和 YAML 值映射，避免过早收窄语法。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DnsConfig {
    pub enable: Option<bool>,
    pub listen: Option<String>,
    pub ipv6: Option<bool>,
    pub ipv6_timeout: Option<u64>,
    pub prefer_h3: Option<bool>,
    pub cache_algorithm: Option<String>,
    pub enhanced_mode: Option<String>,
    pub fake_ip_range: Option<String>,
    pub fake_ip_range6: Option<String>,
    pub fake_ip_filter: Vec<String>,
    pub fake_ip_filter_mode: Option<String>,
    pub fake_ip_ttl: Option<u64>,
    pub use_hosts: Option<bool>,
    pub use_system_hosts: Option<bool>,
    pub respect_rules: Option<bool>,
    pub default_nameserver: Vec<String>,
    pub nameserver: Vec<String>,
    pub fallback: Vec<String>,
    pub proxy_server_nameserver: Vec<String>,
    #[serde(deserialize_with = "empty_map_if_null")]
    pub proxy_server_nameserver_policy: StringValueMap,
    pub direct_nameserver: Vec<String>,
    pub direct_nameserver_follow_policy: Option<bool>,
    pub fallback_filter: Option<FallbackFilterConfig>,
    #[serde(deserialize_with = "empty_map_if_null")]
    pub nameserver_policy: StringValueMap,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct FallbackFilterConfig {
    pub geoip: Option<bool>,
    pub geoip_code: Option<String>,
    pub geosite: Vec<String>,
    pub ipcidr: Vec<String>,
    pub domain: Vec<String>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}
