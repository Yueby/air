use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;
/// 敏感 YAML 值包装。序列化仍保留真实值，Debug 和展示只暴露是否存在。
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SensitiveValue(Value);

impl SensitiveValue {
    pub fn new(value: Value) -> Self {
        Self(value)
    }

    pub fn expose_value(&self) -> &Value {
        &self.0
    }

    pub fn is_set(&self) -> bool {
        !matches!(self.0, Value::Null)
            && !matches!(&self.0, Value::String(value) if value.trim().is_empty())
    }

    pub fn redacted_label(&self) -> &'static str {
        if self.is_set() {
            "<redacted>"
        } else {
            "<empty>"
        }
    }
}

impl fmt::Debug for SensitiveValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.redacted_label())
    }
}

pub(super) fn sensitive(value: Option<Value>) -> Option<SensitiveValue> {
    value.map(SensitiveValue::new)
}

pub(super) fn sensitive_field_names(map: &BTreeMap<String, Value>) -> Vec<String> {
    map.keys()
        .filter(|key| is_sensitive_key(key))
        .cloned()
        .collect()
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|ch| *ch != '-' && *ch != '_')
        .collect::<String>()
        .to_ascii_lowercase();
    normalized.contains("password")
        || normalized.contains("privatekey")
        || normalized.contains("token")
        || normalized.contains("authstr")
        || normalized == "auth"
}
