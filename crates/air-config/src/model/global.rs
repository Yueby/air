use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use super::{
    ExtensionMap, StringValueMap, empty_map_if_null, empty_vec_if_null, string_vec_if_null,
};

/// 顶层全局配置。
///
/// 常用字段直接建模，GUI 可安全展示和编辑；实验性字段、平台相关字段和新版字段会进入
/// `extensions`，由高级配置编辑器或 YAML 编辑器处理。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct GlobalConfig {
    pub port: Option<u32>,
    pub socks_port: Option<u32>,
    pub mixed_port: Option<u32>,
    pub redir_port: Option<u32>,
    pub tproxy_port: Option<u32>,
    pub allow_lan: Option<bool>,
    pub bind_address: Option<String>,
    pub authentication: Vec<String>,
    pub skip_auth_prefixes: Vec<String>,
    pub lan_allowed_ips: Vec<String>,
    pub lan_disallowed_ips: Vec<String>,
    pub find_process_mode: Option<String>,
    pub mode: Option<String>,
    pub log_level: Option<String>,
    pub ipv6: Option<bool>,
    pub keep_alive_interval: Option<u64>,
    pub keep_alive_idle: Option<u64>,
    pub disable_keep_alive: Option<bool>,
    pub unified_delay: Option<bool>,
    pub tcp_concurrent: Option<bool>,
    pub geodata_mode: Option<bool>,
    pub geodata_loader: Option<String>,
    pub geox_url: Option<GeoxUrlConfig>,
    pub geo_auto_update: Option<bool>,
    pub geo_update_interval: Option<u64>,
    pub geosite_matcher: Option<String>,
    pub external_controller: Option<String>,
    pub external_controller_cors: Option<ExternalControllerCorsConfig>,
    pub secret: Option<String>,
    pub external_ui: Option<String>,
    pub external_ui_name: Option<String>,
    pub external_ui_url: Option<String>,
    pub external_doh_server: Option<String>,
    pub interface_name: Option<String>,
    pub routing_mark: Option<Value>,
    pub global_ua: Option<String>,
    #[serde(deserialize_with = "empty_map_if_null")]
    pub hosts: StringValueMap,
    #[serde(deserialize_with = "empty_map_if_null")]
    pub experimental: StringValueMap,
}

/// geodata 下载地址配置。字段较稳定，适合作为全局设置中的高级可编辑项。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct GeoxUrlConfig {
    pub geoip: Option<String>,
    pub geosite: Option<String>,
    pub mmdb: Option<String>,
    pub asn: Option<String>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

/// RESTful API CORS 配置。此项影响控制接口暴露范围，后续 UI 应作为高级/安全设置展示。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ExternalControllerCorsConfig {
    pub allow_origins: Vec<String>,
    pub allow_private_network: Option<bool>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

/// API 证书配置。密钥类字段只建模不记录日志，具体脱敏由 telemetry 层负责。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TlsConfig {
    pub certificate: Option<String>,
    pub private_key: Option<String>,
    pub client_auth_type: Option<String>,
    pub client_auth_cert: Option<String>,
    pub ech_key: Option<String>,
    pub custom_certifactes: Vec<String>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

/// 运行记录持久化配置。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProfileConfig {
    pub store_selected: Option<bool>,
    pub store_fake_ip: Option<bool>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

/// TUN 配置。Linux/Android 等平台差异字段很多，因此只把常用字段类型化，其余交给扩展映射。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TunConfig {
    pub enable: Option<bool>,
    pub stack: Option<String>,
    pub device: Option<String>,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub dns_hijack: Vec<String>,
    pub auto_detect_interface: Option<bool>,
    pub auto_route: Option<bool>,
    pub auto_redirect: Option<bool>,
    pub strict_route: Option<bool>,
    pub mtu: Option<u32>,
    pub gso: Option<bool>,
    pub gso_max_size: Option<u32>,
    pub inet6_address: Option<String>,
    pub udp_timeout: Option<u64>,
    pub iproute2_table_index: Option<u32>,
    pub iproute2_rule_index: Option<u32>,
    pub endpoint_independent_nat: Option<bool>,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub route_address: Vec<String>,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub route_exclude_address: Vec<String>,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub inet4_address: Vec<String>,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub route_address_set: Vec<String>,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub route_exclude_address_set: Vec<String>,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub include_interface: Vec<String>,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub exclude_interface: Vec<String>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

/// 域名嗅探配置。协议细节随 mihomo 版本扩展，`sniff` 内部保持 YAML 值映射。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SnifferConfig {
    pub enable: Option<bool>,
    pub force_dns_mapping: Option<bool>,
    pub parse_pure_ip: Option<bool>,
    pub override_destination: Option<bool>,
    #[serde(deserialize_with = "empty_map_if_null")]
    pub sniff: StringValueMap,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub force_domain: Vec<String>,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub skip_domain: Vec<String>,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub skip_src_address: Vec<String>,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub skip_dst_address: Vec<String>,
    #[serde(deserialize_with = "empty_vec_if_null")]
    pub sniffing: Vec<String>,
    #[serde(deserialize_with = "string_vec_if_null")]
    pub port_whitelist: Vec<String>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

/// tunnel 支持一行字符串和完整 YAML 两种写法，必须保留原始形状。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TunnelConfig {
    Shorthand(String),
    Structured(TunnelObject),
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self::Shorthand(String::new())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TunnelObject {
    pub network: Value,
    pub address: Option<String>,
    pub target: Option<String>,
    pub proxy: Option<String>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}
