use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

/// 统一的扩展字段容器。
///
/// 使用 `serde_yaml::Value` 而不是 `serde_json::Value`，是为了保留 YAML 原生的 null、序列、映射
/// 和数字表达；后续往返写回时不会因为当前版本未建模而丢字段。
pub type ExtensionMap = BTreeMap<String, Value>;

/// mihomo 中常见的“名称到任意 YAML 值”的映射，例如 hosts、headers、nameserver-policy。
pub type StringValueMap = BTreeMap<String, Value>;

/// mihomo 示例中常见 `key:` 后只放注释的写法，YAML 会把它解析成 null。
/// 对映射字段来说 null 与空映射语义一致，因此统一转换为空映射。
pub fn empty_map_if_null<'de, D>(deserializer: D) -> Result<StringValueMap, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<StringValueMap>::deserialize(deserializer)?.unwrap_or_default())
}

/// mihomo 示例中列表字段也可能写成 `key:` 后只放注释，此时 YAML 值为 null。
/// 对列表字段来说 null 与空列表语义一致，统一转换能避免表单加载阶段过早失败。
pub fn empty_vec_if_null<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
}

/// mihomo 的部分端口列表既可能写成字符串，也可能写成未加引号的数字。
/// 领域层会继续做范围校验；这里仅负责把 YAML 标量稳定转换为表单可编辑的字符串。
pub fn string_vec_if_null<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(value) = Option::<Value>::deserialize(deserializer)? else {
        return Ok(Vec::new());
    };

    match value {
        Value::Sequence(values) => values
            .into_iter()
            .map(string_from_yaml_scalar)
            .collect::<Result<Vec<_>, D::Error>>(),
        value => string_from_yaml_scalar(value).map(|value| vec![value]),
    }
}

fn string_from_yaml_scalar<E>(value: Value) -> Result<String, E>
where
    E: serde::de::Error,
{
    match value {
        Value::String(value) => Ok(value),
        Value::Number(value) => Ok(value.to_string()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Null => Ok(String::new()),
        _ => Err(E::custom("字段必须是字符串、数字或对应列表")),
    }
}

/// 一些配置项既接受单个字符串，也接受字符串列表。此类型保留原始形状，避免把用户格式强行改写。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrManyStrings {
    One(String),
    Many(Vec<String>),
}

/// 用户名密码对。入站监听与少量认证字段共享该结构。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Credential {
    pub username: Option<Value>,
    pub password: Option<Value>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}
