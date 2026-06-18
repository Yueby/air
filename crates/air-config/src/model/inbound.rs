use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_yaml::Value;

use super::{Credential, ExtensionMap, StringValueMap};

/// 入站监听配置。
///
/// 不同监听类型的认证、传输层和 TLS 字段差异较大，公共字段类型化后，其余字段保存在
/// `extensions` 中，确保后续新增 inbound 类型不会破坏配置。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ListenerConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ListenerKind,
    pub port: Option<Value>,
    pub listen: Option<String>,
    pub rule: Option<String>,
    pub proxy: Option<String>,
    pub udp: Option<bool>,
    pub users: Option<Value>,
    pub certificate: Option<String>,
    pub private_key: Option<String>,
    pub client_auth_type: Option<String>,
    pub client_auth_cert: Option<String>,
    pub ech_key: Option<String>,
    pub network: Option<Value>,
    pub target: Option<String>,
    pub password: Option<String>,
    pub cipher: Option<String>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

/// 入站用户可能是普通用户名密码，也可能携带 uuid、alterId、flow 等协议字段。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ListenerUser {
    #[serde(flatten)]
    pub credential: Credential,
    pub uuid: Option<String>,
    #[serde(rename = "alterId")]
    pub alter_id: Option<u32>,
    pub flow: Option<String>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListenerKind {
    Socks,
    Http,
    Mixed,
    Redir,
    Tproxy,
    Shadowsocks,
    Vmess,
    Tuic,
    Tunnel,
    Vless,
    Other(String),
}

impl ListenerKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Socks => "socks",
            Self::Http => "http",
            Self::Mixed => "mixed",
            Self::Redir => "redir",
            Self::Tproxy => "tproxy",
            Self::Shadowsocks => "shadowsocks",
            Self::Vmess => "vmess",
            Self::Tuic => "tuic",
            Self::Tunnel => "tunnel",
            Self::Vless => "vless",
            Self::Other(value) => value,
        }
    }
}

impl Default for ListenerKind {
    fn default() -> Self {
        Self::Other(String::new())
    }
}

impl Serialize for ListenerKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ListenerKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "socks" => Self::Socks,
            "http" => Self::Http,
            "mixed" => Self::Mixed,
            "redir" => Self::Redir,
            "tproxy" => Self::Tproxy,
            "shadowsocks" => Self::Shadowsocks,
            "vmess" => Self::Vmess,
            "tuic" => Self::Tuic,
            "tunnel" => Self::Tunnel,
            "vless" => Self::Vless,
            _ => Self::Other(value),
        })
    }
}

/// 少量入站插件配置目前只作为 YAML 映射承载，后续任务再按具体页面拆分。
pub type ListenerPluginConfig = StringValueMap;
