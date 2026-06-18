use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_yaml::Value;

use super::{ExtensionMap, StringValueMap, empty_map_if_null};

/// 出站节点协议类型。
///
/// 已知类型作为枚举值便于后续领域层分发；未知类型保留原字符串，避免 mihomo 新协议被丢弃。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProxyKind {
    Direct,
    Dns,
    Http,
    Socks5,
    Snell,
    Shadowsocks,
    ShadowsocksR,
    Vmess,
    Vless,
    Trojan,
    Hysteria,
    Hysteria2,
    Tuic,
    Wireguard,
    Tailscale,
    Openvpn,
    Masque,
    Ssh,
    Mieru,
    Sudoku,
    Anytls,
    Trusttunnel,
    GostRelay,
    Other(String),
}

impl ProxyKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Direct => "direct",
            Self::Dns => "dns",
            Self::Http => "http",
            Self::Socks5 => "socks5",
            Self::Snell => "snell",
            Self::Shadowsocks => "ss",
            Self::ShadowsocksR => "ssr",
            Self::Vmess => "vmess",
            Self::Vless => "vless",
            Self::Trojan => "trojan",
            Self::Hysteria => "hysteria",
            Self::Hysteria2 => "hysteria2",
            Self::Tuic => "tuic",
            Self::Wireguard => "wireguard",
            Self::Tailscale => "tailscale",
            Self::Openvpn => "openvpn",
            Self::Masque => "masque",
            Self::Ssh => "ssh",
            Self::Mieru => "mieru",
            Self::Sudoku => "sudoku",
            Self::Anytls => "anytls",
            Self::Trusttunnel => "trusttunnel",
            Self::GostRelay => "gost-relay",
            Self::Other(value) => value,
        }
    }
}

impl Default for ProxyKind {
    fn default() -> Self {
        Self::Other(String::new())
    }
}

impl Serialize for ProxyKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProxyKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let normalized = value.trim().to_ascii_lowercase();
        Ok(match normalized.as_str() {
            "direct" => Self::Direct,
            "dns" => Self::Dns,
            "http" => Self::Http,
            "socks" | "socks5" => Self::Socks5,
            "snell" => Self::Snell,
            "ss" | "shadowsocks" => Self::Shadowsocks,
            "ssr" | "shadowsocksr" => Self::ShadowsocksR,
            "vmess" => Self::Vmess,
            "vless" => Self::Vless,
            "trojan" => Self::Trojan,
            "hysteria" => Self::Hysteria,
            "hysteria2" => Self::Hysteria2,
            "tuic" => Self::Tuic,
            "wireguard" | "wire-guard" => Self::Wireguard,
            "tailscale" => Self::Tailscale,
            "openvpn" | "open-vpn" => Self::Openvpn,
            "masque" => Self::Masque,
            "ssh" => Self::Ssh,
            "mieru" => Self::Mieru,
            "sudoku" => Self::Sudoku,
            "anytls" | "any-tls" => Self::Anytls,
            "trusttunnel" | "trust-tunnel" => Self::Trusttunnel,
            "gost-relay" => Self::GostRelay,
            _ => Self::Other(value),
        })
    }
}

/// 出站代理节点。
///
/// 节点协议字段差异极大，本结构只抽取跨协议的可编辑字段；协议专属参数通过 `extensions`
/// 原样保留，例如 `reality-opts`、`plugin-opts`、`grpc-opts`、WireGuard peers 等。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyNode {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ProxyKind,
    pub server: Option<Value>,
    pub port: Option<Value>,
    pub username: Option<Value>,
    pub password: Option<Value>,
    pub uuid: Option<Value>,
    pub cipher: Option<Value>,
    pub udp: Option<bool>,
    pub tls: Option<bool>,
    pub network: Option<Value>,
    pub sni: Option<String>,
    pub servername: Option<String>,
    pub skip_cert_verify: Option<bool>,
    pub fingerprint: Option<String>,
    pub client_fingerprint: Option<Value>,
    pub alpn: Vec<String>,
    pub dialer_proxy: Option<String>,
    pub interface_name: Option<String>,
    pub routing_mark: Option<Value>,
    pub ip_version: Option<String>,
    pub plugin: Option<String>,
    #[serde(deserialize_with = "empty_map_if_null")]
    pub plugin_opts: StringValueMap,
    pub smux: Option<SmuxConfig>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SmuxConfig {
    pub enabled: Option<bool>,
    pub protocol: Option<String>,
    pub max_connections: Option<u32>,
    pub min_streams: Option<u32>,
    pub max_streams: Option<u32>,
    pub padding: Option<bool>,
    pub statistic: Option<bool>,
    pub only_tcp: Option<bool>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

/// 代理组。`proxies` 和 `use` 是普通用户常编辑字段，其余健康检查和筛选项保持类型化。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyGroup {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ProxyGroupKind,
    pub proxies: Vec<String>,
    #[serde(rename = "use")]
    pub use_providers: Vec<String>,
    pub filter: Option<String>,
    pub exclude_filter: Option<String>,
    pub url: Option<String>,
    pub interval: Option<u64>,
    pub tolerance: Option<u64>,
    pub lazy: Option<bool>,
    pub expected_status: Option<Value>,
    pub strategy: Option<String>,
    pub disable_udp: Option<bool>,
    #[serde(flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProxyGroupKind {
    Select,
    UrlTest,
    Fallback,
    LoadBalance,
    Relay,
    Other(String),
}

impl ProxyGroupKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Select => "select",
            Self::UrlTest => "url-test",
            Self::Fallback => "fallback",
            Self::LoadBalance => "load-balance",
            Self::Relay => "relay",
            Self::Other(value) => value,
        }
    }
}

impl Default for ProxyGroupKind {
    fn default() -> Self {
        Self::Other(String::new())
    }
}

impl Serialize for ProxyGroupKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProxyGroupKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let normalized = value.trim().to_ascii_lowercase();
        Ok(match normalized.as_str() {
            "select" | "selector" => Self::Select,
            "url-test" | "urltest" => Self::UrlTest,
            "fallback" => Self::Fallback,
            "load-balance" | "loadbalance" => Self::LoadBalance,
            "relay" => Self::Relay,
            _ => Self::Other(value),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proxy_kinds_case_insensitively() {
        let node: ProxyNode = serde_yaml::from_str(
            r#"
name: hk
type: VMess
server: example.com
port: 443
"#,
        )
        .expect("mixed-case proxy kind should parse");

        assert_eq!(node.kind, ProxyKind::Vmess);
    }

    #[test]
    fn parses_proxy_group_kinds_case_insensitively() {
        let group: ProxyGroup = serde_yaml::from_str(
            r#"
name: Auto
type: LoadBalance
proxies:
  - DIRECT
"#,
        )
        .expect("mixed-case proxy group kind should parse");

        assert_eq!(group.kind, ProxyGroupKind::LoadBalance);
    }
}
