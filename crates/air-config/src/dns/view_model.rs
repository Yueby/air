use serde::{Deserialize, Serialize};

use super::nameserver::{classify_nameserver, nameserver_view_models, route_hint};
use super::policy::{DnsNameserverPolicySettings, policy_value_preview};
use super::settings::DnsConfigSettings;
use crate::ConfigDiagnostic;
/// GUI 表单 view model：把 DNS 配置拆成常规、fake-ip、上游和策略四组。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DnsConfigFormViewModel {
    pub general: DnsGeneralViewModel,
    pub fake_ip: DnsFakeIpViewModel,
    pub upstream: DnsUpstreamViewModel,
    pub policies: DnsPolicyListViewModel,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

impl From<&DnsConfigSettings> for DnsConfigFormViewModel {
    fn from(settings: &DnsConfigSettings) -> Self {
        Self {
            general: DnsGeneralViewModel {
                enable: DnsBooleanFormValue::from_option(settings.enable),
                listen: settings.listen.clone().unwrap_or_default(),
                enhanced_mode: settings.enhanced_mode.clone().unwrap_or_default(),
                respect_rules: DnsBooleanFormValue::from_option(settings.respect_rules),
            },
            fake_ip: DnsFakeIpViewModel {
                fake_ip_range: settings.fake_ip_range.clone().unwrap_or_default(),
                fake_ip_range6: settings.fake_ip_range6.clone().unwrap_or_default(),
                fake_ip_filter: settings.fake_ip_filter.clone(),
                fake_ip_filter_mode: settings.fake_ip_filter_mode.clone().unwrap_or_default(),
                fake_ip_ttl: optional_u64(settings.fake_ip_ttl),
                rule_mode_risk: settings.fake_ip_filter_mode.as_deref() == Some("rule"),
            },
            upstream: DnsUpstreamViewModel {
                default_nameserver: nameserver_view_models(&settings.default_nameserver),
                nameserver: nameserver_view_models(&settings.nameserver),
                fallback: nameserver_view_models(&settings.fallback),
                proxy_server_nameserver: nameserver_view_models(&settings.proxy_server_nameserver),
                direct_nameserver: nameserver_view_models(&settings.direct_nameserver),
                direct_nameserver_follow_policy: DnsBooleanFormValue::from_option(
                    settings.direct_nameserver_follow_policy,
                ),
            },
            policies: DnsPolicyListViewModel {
                nameserver_policy: settings
                    .nameserver_policy
                    .iter()
                    .map(DnsPolicyRuleViewModel::from)
                    .collect(),
                proxy_server_nameserver_policy: settings
                    .proxy_server_nameserver_policy
                    .iter()
                    .map(DnsPolicyRuleViewModel::from)
                    .collect(),
            },
            diagnostics: settings.validate(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DnsGeneralViewModel {
    pub enable: DnsBooleanFormValue,
    pub listen: String,
    pub enhanced_mode: String,
    pub respect_rules: DnsBooleanFormValue,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DnsFakeIpViewModel {
    pub fake_ip_range: String,
    pub fake_ip_range6: String,
    pub fake_ip_filter: Vec<String>,
    pub fake_ip_filter_mode: String,
    pub fake_ip_ttl: String,
    pub rule_mode_risk: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DnsUpstreamViewModel {
    pub default_nameserver: Vec<DnsNameserverViewModel>,
    pub nameserver: Vec<DnsNameserverViewModel>,
    pub fallback: Vec<DnsNameserverViewModel>,
    pub proxy_server_nameserver: Vec<DnsNameserverViewModel>,
    pub direct_nameserver: Vec<DnsNameserverViewModel>,
    pub direct_nameserver_follow_policy: DnsBooleanFormValue,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DnsPolicyListViewModel {
    pub nameserver_policy: Vec<DnsPolicyRuleViewModel>,
    pub proxy_server_nameserver_policy: Vec<DnsPolicyRuleViewModel>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DnsPolicyRuleViewModel {
    pub matcher: String,
    pub nameservers: Vec<DnsNameserverViewModel>,
    pub value_preview: String,
    pub has_advanced_value: bool,
}

impl From<&DnsNameserverPolicySettings> for DnsPolicyRuleViewModel {
    fn from(policy: &DnsNameserverPolicySettings) -> Self {
        Self {
            matcher: policy.matcher.clone(),
            nameservers: nameserver_view_models(&policy.nameservers),
            value_preview: policy_value_preview(policy),
            has_advanced_value: policy.passthrough.is_some() && policy.nameservers.is_empty(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DnsNameserverViewModel {
    pub raw: String,
    pub protocol: DnsNameserverProtocol,
    pub has_route_hint: bool,
    pub force_h3: bool,
}

impl From<&str> for DnsNameserverViewModel {
    fn from(raw: &str) -> Self {
        let protocol = classify_nameserver(raw);
        Self {
            raw: raw.to_string(),
            protocol,
            has_route_hint: route_hint(raw).is_some(),
            force_h3: raw
                .split_once('#')
                .map(|(_, fragment)| fragment.to_ascii_lowercase().contains("h3=true"))
                .unwrap_or(false),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DnsNameserverProtocol {
    PlainIp,
    Udp,
    Tcp,
    Tls,
    Https,
    Quic,
    Dhcp,
    System,
    Rcode,
    #[default]
    Other,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DnsBooleanFormValue {
    pub value: bool,
    pub configured: bool,
}

impl DnsBooleanFormValue {
    fn from_option(value: Option<bool>) -> Self {
        Self {
            value: value.unwrap_or_default(),
            configured: value.is_some(),
        }
    }
}

fn optional_u64(value: Option<u64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}
