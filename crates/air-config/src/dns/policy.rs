use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use super::settings::normalized_strings;
/// nameserver-policy 的单条规则。`value_style` 用于避免把原本的单值写法强制改成列表。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DnsNameserverPolicySettings {
    pub matcher: String,
    pub nameservers: Vec<String>,
    pub value_style: DnsPolicyValueStyle,
    pub passthrough: Option<Value>,
}

impl Default for DnsNameserverPolicySettings {
    fn default() -> Self {
        Self {
            matcher: String::new(),
            nameservers: Vec::new(),
            value_style: DnsPolicyValueStyle::Sequence,
            passthrough: None,
        }
    }
}

impl DnsNameserverPolicySettings {
    pub fn new(matcher: impl Into<String>, nameservers: Vec<String>) -> Self {
        Self {
            matcher: matcher.into(),
            nameservers,
            value_style: DnsPolicyValueStyle::Sequence,
            passthrough: None,
        }
    }

    pub(super) fn from_yaml(matcher: &str, value: &Value) -> Self {
        match value {
            Value::String(value) => Self {
                matcher: matcher.to_string(),
                nameservers: vec![value.clone()],
                value_style: DnsPolicyValueStyle::Single,
                passthrough: None,
            },
            Value::Sequence(values) => {
                let nameservers = values
                    .iter()
                    .filter_map(yaml_scalar_to_string)
                    .collect::<Vec<_>>();
                if nameservers.len() == values.len() {
                    Self {
                        matcher: matcher.to_string(),
                        nameservers,
                        value_style: DnsPolicyValueStyle::Sequence,
                        passthrough: None,
                    }
                } else {
                    Self {
                        matcher: matcher.to_string(),
                        value_style: DnsPolicyValueStyle::Passthrough,
                        passthrough: Some(value.clone()),
                        ..Default::default()
                    }
                }
            }
            value => Self {
                matcher: matcher.to_string(),
                value_style: DnsPolicyValueStyle::Passthrough,
                passthrough: Some(value.clone()),
                ..Default::default()
            },
        }
    }

    pub(super) fn to_yaml_value(&self) -> Value {
        let nameservers = normalized_strings(&self.nameservers);
        if nameservers.is_empty()
            && let Some(value) = &self.passthrough
        {
            return value.clone();
        }

        if self.value_style == DnsPolicyValueStyle::Single && nameservers.len() == 1 {
            return Value::String(nameservers[0].clone());
        }

        Value::Sequence(nameservers.into_iter().map(Value::String).collect())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DnsPolicyValueStyle {
    Single,
    #[default]
    Sequence,
    Passthrough,
}

pub(super) fn yaml_scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn policy_settings_from_map(
    map: &BTreeMap<String, Value>,
) -> Vec<DnsNameserverPolicySettings> {
    map.iter()
        .map(|(matcher, value)| DnsNameserverPolicySettings::from_yaml(matcher, value))
        .collect()
}

pub(super) fn policy_settings_to_map(
    policies: &[DnsNameserverPolicySettings],
) -> BTreeMap<String, Value> {
    policies
        .iter()
        .filter_map(|policy| {
            let matcher = policy.matcher.trim();
            (!matcher.is_empty()).then(|| (matcher.to_string(), policy.to_yaml_value()))
        })
        .collect()
}

pub(super) fn policy_value_preview(policy: &DnsNameserverPolicySettings) -> String {
    if !policy.nameservers.is_empty() {
        return policy.nameservers.join(", ");
    }
    policy
        .passthrough
        .as_ref()
        .map(|value| format!("{value:?}"))
        .unwrap_or_default()
}
