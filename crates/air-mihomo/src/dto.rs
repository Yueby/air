use std::collections::BTreeMap;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VersionResponse {
    pub version: String,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigsResponse {
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProxyResponse {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub all: Vec<String>,
    #[serde(default)]
    pub now: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub history: Vec<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProxiesResponse {
    #[serde(default)]
    pub proxies: BTreeMap<String, ProxyResponse>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GroupsResponse {
    #[serde(default)]
    pub groups: BTreeMap<String, ProxyResponse>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl<'de> Deserialize<'de> for GroupsResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut object = match Value::deserialize(deserializer)? {
            Value::Object(object) => object,
            other => {
                return Err(D::Error::custom(format!(
                    "groups response must be object, got {other:?}"
                )));
            }
        };

        let groups = if let Some(value) = object.remove("groups") {
            serde_json::from_value(value).map_err(D::Error::custom)?
        } else if let Some(value) = object.remove("proxies") {
            let responses: Vec<ProxyResponse> =
                serde_json::from_value(value).map_err(D::Error::custom)?;
            responses
                .into_iter()
                .filter(|response| !response.name.trim().is_empty())
                .map(|response| (response.name.clone(), response))
                .collect()
        } else {
            BTreeMap::new()
        };

        Ok(Self {
            groups,
            extra: object.into_iter().collect(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RulesResponse {
    #[serde(default)]
    pub rules: Vec<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProvidersResponse {
    #[serde(default)]
    pub providers: BTreeMap<String, Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectionsResponse {
    #[serde(default, deserialize_with = "deserialize_nullable_vec")]
    pub connections: Vec<Value>,
    #[serde(default, rename = "uploadTotal", alias = "upload_total")]
    pub upload_total: u64,
    #[serde(default, rename = "downloadTotal", alias = "download_total")]
    pub download_total: u64,
    #[serde(default, alias = "inuse", alias = "in_use", alias = "mem")]
    pub memory: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn deserialize_nullable_vec<'de, D>(deserializer: D) -> Result<Vec<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    // mihomo 在没有活动连接时可能返回 `connections: null`；UI 语义上等同于空列表。
    Option::<Vec<Value>>::deserialize(deserializer)
        .map(|connections| connections.unwrap_or_default())
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DelayResponse {
    pub delay: u64,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl<'de> Deserialize<'de> for DelayResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Value::deserialize(deserializer)? {
            Value::Object(mut object) => {
                // mihomo 单节点测速的标准响应是 `{ "delay": 934 }`；这里显式取出 delay，
                // 其余字段继续保留到 extra，避免后续核心版本添加诊断字段时解析失败。
                let delay = object
                    .remove("delay")
                    .ok_or_else(|| D::Error::missing_field("delay"))
                    .and_then(|value| serde_json::from_value(value).map_err(D::Error::custom))?;
                Ok(Self {
                    delay,
                    extra: object.into_iter().collect(),
                })
            }
            Value::Number(number) => {
                // 部分兼容实现会直接返回数字，统一收敛成 DelayResponse，避免 UI 测速链路崩掉。
                let delay = number
                    .as_u64()
                    .ok_or_else(|| D::Error::custom("delay number must be unsigned integer"))?;
                Ok(Self {
                    delay,
                    extra: BTreeMap::new(),
                })
            }
            other => Err(D::Error::custom(format!(
                "delay response must be object or number, got {other:?}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GroupDelayResponse {
    pub delays: BTreeMap<String, u64>,
}

impl<'de> Deserialize<'de> for GroupDelayResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let delays = BTreeMap::<String, u64>::deserialize(deserializer)?;
        Ok(Self { delays })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SelectProxyRequest<'a> {
    pub name: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PathPayloadRequest<'a> {
    pub path: &'a str,
    pub payload: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_response_accepts_array_shape_from_group_endpoint() {
        let response: GroupsResponse = serde_json::from_str(
            r#"{"proxies":[{"name":"SSRDOG","all":["Auto","DIRECT"],"now":"Auto","type":"Selector"}]}"#,
        )
        .expect("array-shaped group response should parse");

        let group = response
            .groups
            .get("SSRDOG")
            .expect("group name should become map key");
        assert_eq!(group.now, "Auto");
        assert_eq!(group.kind, "Selector");
        assert_eq!(group.all, vec!["Auto", "DIRECT"]);
    }

    #[test]
    fn groups_response_keeps_legacy_map_shape() {
        let response: GroupsResponse = serde_json::from_str(
            r#"{"groups":{"Proxy":{"all":["DIRECT"],"now":"DIRECT","type":"Selector"}}}"#,
        )
        .expect("map-shaped group response should still parse");

        assert!(response.groups.contains_key("Proxy"));
    }

    #[test]
    fn connections_response_accepts_null_connections_as_empty_list() {
        let response = serde_json::from_str::<ConnectionsResponse>(
            r#"{
                "downloadTotal": 0,
                "uploadTotal": 0,
                "connections": null,
                "memory": 51138560
            }"#,
        )
        .expect("mihomo may return null connections when there are no active connections");

        assert!(response.connections.is_empty());
        assert_eq!(response.download_total, 0);
        assert_eq!(response.upload_total, 0);
        assert_eq!(response.memory, 51138560);
    }

    #[test]
    fn connections_response_accepts_memory_aliases_from_snapshot() {
        let inuse = serde_json::from_str::<ConnectionsResponse>(
            r#"{
                "downloadTotal": 0,
                "uploadTotal": 0,
                "connections": [],
                "inuse": 86085632
            }"#,
        )
        .expect("connections snapshot may use memory stream field naming");
        let in_use = serde_json::from_str::<ConnectionsResponse>(
            r#"{
                "downloadTotal": 0,
                "uploadTotal": 0,
                "connections": [],
                "in_use": 96085632
            }"#,
        )
        .expect("connections snapshot may use snake case memory naming");

        assert_eq!(inuse.memory, 86085632);
        assert_eq!(in_use.memory, 96085632);
    }

    #[test]
    fn group_delay_response_accepts_member_delay_map() {
        let response: GroupDelayResponse =
            serde_json::from_str(r#"{"Auto":248,"🇭🇰 Hong Kong丨01":244}"#)
                .expect("group delay map should parse");

        assert_eq!(response.delays.get("Auto"), Some(&248));
        assert_eq!(response.delays.get("🇭🇰 Hong Kong丨01"), Some(&244));
    }

    #[test]
    fn delay_response_accepts_mihomo_single_node_shape() {
        let response: DelayResponse =
            serde_json::from_str(r#"{"delay":934}"#).expect("single node delay should parse");

        assert_eq!(response.delay, 934);
    }

    #[test]
    fn delay_response_keeps_extra_fields_and_numeric_compatibility() {
        let response: DelayResponse = serde_json::from_str(r#"{"delay":934,"source":"cache"}"#)
            .expect("delay extra fields should be preserved");
        assert_eq!(response.delay, 934);
        assert_eq!(
            response.extra.get("source").and_then(Value::as_str),
            Some("cache")
        );

        let response: DelayResponse =
            serde_json::from_str("934").expect("numeric delay body should remain compatible");
        assert_eq!(response.delay, 934);
        assert!(response.extra.is_empty());
    }
}
