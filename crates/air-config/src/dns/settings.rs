use serde::{Deserialize, Serialize};

use crate::ConfigDiagnostic;
use crate::model::{DnsConfig, FallbackFilterConfig, MihomoConfigDocument};

use super::policy::{
    DnsNameserverPolicySettings, policy_settings_from_map, policy_settings_to_map,
};
use super::validator::validate_settings;
/// DNS 配置页负责编辑的字段集合。
///
/// 写回时只覆盖本结构明确建模的 DNS 字段；`DnsConfig.extensions`、`fallback-filter`
/// 等高级字段会继续保留在原文档中。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DnsConfigSettings {
    pub enable: Option<bool>,
    pub cache_algorithm: Option<String>,
    pub prefer_h3: Option<bool>,
    pub use_hosts: Option<bool>,
    pub use_system_hosts: Option<bool>,
    pub respect_rules: Option<bool>,
    pub listen: Option<String>,
    pub ipv6: Option<bool>,
    pub default_nameserver: Vec<String>,
    pub enhanced_mode: Option<String>,
    pub fake_ip_range: Option<String>,
    pub fake_ip_range6: Option<String>,
    pub fake_ip_filter: Vec<String>,
    pub fake_ip_filter_mode: Option<String>,
    pub fake_ip_ttl: Option<u64>,
    pub nameserver: Vec<String>,
    pub fallback: Vec<String>,
    pub proxy_server_nameserver: Vec<String>,
    pub proxy_server_nameserver_policy: Vec<DnsNameserverPolicySettings>,
    pub direct_nameserver: Vec<String>,
    pub direct_nameserver_follow_policy: Option<bool>,
    pub nameserver_policy: Vec<DnsNameserverPolicySettings>,
    pub fallback_filter: Option<FallbackFilterConfig>,
}

impl DnsConfigSettings {
    pub fn from_document(document: &MihomoConfigDocument) -> Self {
        document
            .dns
            .as_ref()
            .map(Self::from_config)
            .unwrap_or_default()
    }

    pub fn from_config(config: &DnsConfig) -> Self {
        Self {
            enable: config.enable,
            cache_algorithm: config.cache_algorithm.clone(),
            prefer_h3: config.prefer_h3,
            use_hosts: config.use_hosts,
            use_system_hosts: config.use_system_hosts,
            respect_rules: config.respect_rules,
            listen: config.listen.clone(),
            ipv6: config.ipv6,
            default_nameserver: config.default_nameserver.clone(),
            enhanced_mode: config.enhanced_mode.clone(),
            fake_ip_range: config.fake_ip_range.clone(),
            fake_ip_range6: config.fake_ip_range6.clone(),
            fake_ip_filter: config.fake_ip_filter.clone(),
            fake_ip_filter_mode: config.fake_ip_filter_mode.clone(),
            fake_ip_ttl: config.fake_ip_ttl,
            nameserver: config.nameserver.clone(),
            fallback: config.fallback.clone(),
            proxy_server_nameserver: config.proxy_server_nameserver.clone(),
            proxy_server_nameserver_policy: policy_settings_from_map(
                &config.proxy_server_nameserver_policy,
            ),
            direct_nameserver: config.direct_nameserver.clone(),
            direct_nameserver_follow_policy: config.direct_nameserver_follow_policy,
            nameserver_policy: policy_settings_from_map(&config.nameserver_policy),
            fallback_filter: config.fallback_filter.clone(),
        }
    }

    /// 将 DNS 设置写回完整文档，保留 DNS 内部未知扩展字段和未由当前任务编辑的高级字段。
    pub fn apply_to_document(&self, document: &mut MihomoConfigDocument) {
        let mut dns = document.dns.clone().unwrap_or_default();
        dns.enable = self.enable;
        dns.cache_algorithm = normalize_optional_string(self.cache_algorithm.as_deref());
        dns.prefer_h3 = self.prefer_h3;
        dns.use_hosts = self.use_hosts;
        dns.use_system_hosts = self.use_system_hosts;
        dns.respect_rules = self.respect_rules;
        dns.listen = normalize_optional_string(self.listen.as_deref());
        dns.ipv6 = self.ipv6;
        dns.default_nameserver = normalized_strings(&self.default_nameserver);
        dns.enhanced_mode = normalize_optional_string(self.enhanced_mode.as_deref());
        dns.fake_ip_range = normalize_optional_string(self.fake_ip_range.as_deref());
        dns.fake_ip_range6 = normalize_optional_string(self.fake_ip_range6.as_deref());
        dns.fake_ip_filter = normalized_strings(&self.fake_ip_filter);
        dns.fake_ip_filter_mode = normalize_optional_string(self.fake_ip_filter_mode.as_deref());
        dns.fake_ip_ttl = self.fake_ip_ttl;
        dns.nameserver = normalized_strings(&self.nameserver);
        dns.fallback = normalized_strings(&self.fallback);
        dns.proxy_server_nameserver = normalized_strings(&self.proxy_server_nameserver);
        dns.proxy_server_nameserver_policy =
            policy_settings_to_map(&self.proxy_server_nameserver_policy);
        dns.direct_nameserver = normalized_strings(&self.direct_nameserver);
        dns.direct_nameserver_follow_policy = self.direct_nameserver_follow_policy;
        dns.nameserver_policy = policy_settings_to_map(&self.nameserver_policy);
        dns.fallback_filter = self.fallback_filter.clone();

        if self.is_empty() && dns.extensions.is_empty() && dns.ipv6_timeout.is_none() {
            document.dns = None;
        } else {
            document.dns = Some(dns);
        }
    }

    pub fn validate(&self) -> Vec<ConfigDiagnostic> {
        validate_settings(self)
    }

    fn is_empty(&self) -> bool {
        self.enable.is_none()
            && self
                .cache_algorithm
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            && self.prefer_h3.is_none()
            && self.use_hosts.is_none()
            && self.use_system_hosts.is_none()
            && self.respect_rules.is_none()
            && self.listen.as_deref().unwrap_or_default().trim().is_empty()
            && self.ipv6.is_none()
            && self.default_nameserver.is_empty()
            && self
                .enhanced_mode
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            && self
                .fake_ip_range
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            && self
                .fake_ip_range6
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            && self.fake_ip_filter.is_empty()
            && self
                .fake_ip_filter_mode
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            && self.fake_ip_ttl.is_none()
            && self.nameserver.is_empty()
            && self.fallback.is_empty()
            && self.proxy_server_nameserver.is_empty()
            && self.proxy_server_nameserver_policy.is_empty()
            && self.direct_nameserver.is_empty()
            && self.direct_nameserver_follow_policy.is_none()
            && self.nameserver_policy.is_empty()
            && self.fallback_filter.is_none()
    }
}

pub(super) fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

pub(super) fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn normalized_strings(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
