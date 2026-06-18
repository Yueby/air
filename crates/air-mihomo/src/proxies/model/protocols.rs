use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use air_config::model::{ProxyKind, ProxyNode, StringValueMap, empty_map_if_null};

use super::sensitive::{SensitiveValue, sensitive, sensitive_field_names};
/// 协议特有字段。每个分支只抽取后续编辑页高频需要的字段；完整 YAML 仍由 `raw` 保存。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "kebab-case")]
pub enum ProxyProtocolSettings {
    Direct(DirectProxySettings),
    Dns(DnsProxySettings),
    Snell(SnellProxySettings),
    Http(HttpProxySettings),
    Socks5(Socks5ProxySettings),
    Shadowsocks(ShadowsocksProxySettings),
    ShadowsocksR(ShadowsocksRProxySettings),
    Vmess(VmessProxySettings),
    Vless(VlessProxySettings),
    Trojan(TrojanProxySettings),
    Hysteria(HysteriaProxySettings),
    Hysteria2(Hysteria2ProxySettings),
    Wireguard(WireguardProxySettings),
    Tailscale(TailscaleProxySettings),
    Openvpn(OpenvpnProxySettings),
    Masque(MasqueProxySettings),
    Tuic(TuicProxySettings),
    Ssh(SshProxySettings),
    Mieru(MieruProxySettings),
    Sudoku(SudokuProxySettings),
    Anytls(AnytlsProxySettings),
    Trusttunnel(TrusttunnelProxySettings),
    GostRelay(GostRelayProxySettings),
    Raw(RawProxySettings),
}

impl ProxyProtocolSettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        match &node.kind {
            ProxyKind::Direct => Self::Direct(DirectProxySettings::from_config(node)),
            ProxyKind::Dns => Self::Dns(DnsProxySettings::from_config(node)),
            ProxyKind::Snell => Self::Snell(SnellProxySettings::from_config(node)),
            ProxyKind::Http => Self::Http(HttpProxySettings::from_config(node)),
            ProxyKind::Socks5 => Self::Socks5(Socks5ProxySettings::from_config(node)),
            ProxyKind::Shadowsocks => {
                Self::Shadowsocks(ShadowsocksProxySettings::from_config(node))
            }
            ProxyKind::ShadowsocksR => {
                Self::ShadowsocksR(ShadowsocksRProxySettings::from_config(node))
            }
            ProxyKind::Vmess => Self::Vmess(VmessProxySettings::from_config(node)),
            ProxyKind::Vless => Self::Vless(VlessProxySettings::from_config(node)),
            ProxyKind::Trojan => Self::Trojan(TrojanProxySettings::from_config(node)),
            ProxyKind::Hysteria => Self::Hysteria(HysteriaProxySettings::from_config(node)),
            ProxyKind::Hysteria2 => Self::Hysteria2(Hysteria2ProxySettings::from_config(node)),
            ProxyKind::Wireguard => Self::Wireguard(WireguardProxySettings::from_config(node)),
            ProxyKind::Tailscale => Self::Tailscale(TailscaleProxySettings::from_config(node)),
            ProxyKind::Openvpn => Self::Openvpn(OpenvpnProxySettings::from_config(node)),
            ProxyKind::Masque => Self::Masque(MasqueProxySettings::from_config(node)),
            ProxyKind::Tuic => Self::Tuic(TuicProxySettings::from_config(node)),
            ProxyKind::Ssh => Self::Ssh(SshProxySettings::from_config(node)),
            ProxyKind::Mieru => Self::Mieru(MieruProxySettings::from_config(node)),
            ProxyKind::Sudoku => Self::Sudoku(SudokuProxySettings::from_config(node)),
            ProxyKind::Anytls => Self::Anytls(AnytlsProxySettings::from_config(node)),
            ProxyKind::Trusttunnel => {
                Self::Trusttunnel(TrusttunnelProxySettings::from_config(node))
            }
            ProxyKind::GostRelay => Self::GostRelay(GostRelayProxySettings::from_config(node)),
            _ => Self::Raw(RawProxySettings::from_config(node)),
        }
    }

    pub fn protocol_name(&self) -> &str {
        match self {
            Self::Direct(_) => "direct",
            Self::Dns(_) => "dns",
            Self::Snell(_) => "snell",
            Self::Http(_) => "http",
            Self::Socks5(_) => "socks5",
            Self::Shadowsocks(_) => "ss",
            Self::ShadowsocksR(_) => "ssr",
            Self::Vmess(_) => "vmess",
            Self::Vless(_) => "vless",
            Self::Trojan(_) => "trojan",
            Self::Hysteria(_) => "hysteria",
            Self::Hysteria2(_) => "hysteria2",
            Self::Wireguard(_) => "wireguard",
            Self::Tailscale(_) => "tailscale",
            Self::Openvpn(_) => "openvpn",
            Self::Masque(_) => "masque",
            Self::Tuic(_) => "tuic",
            Self::Ssh(_) => "ssh",
            Self::Mieru(_) => "mieru",
            Self::Sudoku(_) => "sudoku",
            Self::Anytls(_) => "anytls",
            Self::Trusttunnel(_) => "trusttunnel",
            Self::GostRelay(_) => "gost-relay",
            Self::Raw(settings) => settings.kind.as_str(),
        }
    }
}

impl Default for ProxyProtocolSettings {
    fn default() -> Self {
        Self::Raw(RawProxySettings::default())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DirectProxySettings {
    pub interface_name: Option<String>,
    pub routing_mark: Option<Value>,
}

impl DirectProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            interface_name: node.interface_name.clone(),
            routing_mark: node.routing_mark.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct DnsProxySettings {
    pub target: Option<Value>,
}

impl DnsProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            target: extension_value(node, "target"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SnellProxySettings {
    pub psk: Option<SensitiveValue>,
    pub version: Option<Value>,
    pub obfs_opts: Option<Value>,
}

impl SnellProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            psk: sensitive(extension_value(node, "psk")),
            version: extension_value(node, "version"),
            obfs_opts: extension_value(node, "obfs-opts"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct HttpProxySettings {
    pub username: Option<Value>,
    pub password: Option<SensitiveValue>,
    pub tls: Option<bool>,
    pub sni: Option<String>,
    pub fingerprint: Option<String>,
}

impl HttpProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            username: node.username.clone(),
            password: sensitive(node.password.clone()),
            tls: node.tls,
            sni: first_string([node.sni.as_deref(), node.servername.as_deref()]),
            fingerprint: node.fingerprint.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Socks5ProxySettings {
    pub username: Option<Value>,
    pub password: Option<SensitiveValue>,
    pub tls: Option<bool>,
    pub fingerprint: Option<String>,
}

impl Socks5ProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            username: node.username.clone(),
            password: sensitive(node.password.clone()),
            tls: node.tls,
            fingerprint: node.fingerprint.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ShadowsocksProxySettings {
    pub cipher: Option<Value>,
    pub password: Option<SensitiveValue>,
    pub plugin: Option<String>,
    #[serde(deserialize_with = "empty_map_if_null")]
    pub plugin_opts: StringValueMap,
    pub client_fingerprint: Option<Value>,
}

impl ShadowsocksProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            cipher: node.cipher.clone(),
            password: sensitive(node.password.clone()),
            plugin: node.plugin.clone(),
            plugin_opts: node.plugin_opts.clone(),
            client_fingerprint: node.client_fingerprint.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ShadowsocksRProxySettings {
    pub cipher: Option<Value>,
    pub password: Option<SensitiveValue>,
    pub obfs: Option<Value>,
    pub protocol: Option<Value>,
    pub obfs_param: Option<Value>,
    pub protocol_param: Option<Value>,
}

impl ShadowsocksRProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            cipher: node.cipher.clone(),
            password: sensitive(node.password.clone()),
            obfs: extension_value(node, "obfs"),
            protocol: extension_value(node, "protocol"),
            obfs_param: extension_value(node, "obfs-param"),
            protocol_param: extension_value(node, "protocol-param"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct VmessProxySettings {
    pub uuid: Option<Value>,
    pub alter_id: Option<Value>,
    pub cipher: Option<Value>,
    pub tls: Option<bool>,
    pub network: Option<Value>,
    pub servername: Option<String>,
    pub client_fingerprint: Option<Value>,
}

impl VmessProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            uuid: node.uuid.clone(),
            alter_id: first_extension_value(node, &["alterId", "alter-id"]),
            cipher: node.cipher.clone(),
            tls: node.tls,
            network: node.network.clone(),
            servername: first_string([node.servername.as_deref(), node.sni.as_deref()]),
            client_fingerprint: node.client_fingerprint.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct VlessProxySettings {
    pub uuid: Option<Value>,
    pub flow: Option<Value>,
    pub tls: Option<bool>,
    pub network: Option<Value>,
    pub servername: Option<String>,
    pub client_fingerprint: Option<Value>,
    pub reality_opts: Option<Value>,
}

impl VlessProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            uuid: node.uuid.clone(),
            flow: extension_value(node, "flow"),
            tls: node.tls,
            network: node.network.clone(),
            servername: first_string([node.servername.as_deref(), node.sni.as_deref()]),
            client_fingerprint: node.client_fingerprint.clone(),
            reality_opts: extension_value(node, "reality-opts"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TrojanProxySettings {
    pub password: Option<SensitiveValue>,
    pub tls: Option<bool>,
    pub network: Option<Value>,
    pub sni: Option<String>,
    pub flow: Option<Value>,
    pub client_fingerprint: Option<Value>,
}

impl TrojanProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            password: sensitive(node.password.clone()),
            tls: node.tls,
            network: node.network.clone(),
            sni: first_string([node.sni.as_deref(), node.servername.as_deref()]),
            flow: extension_value(node, "flow"),
            client_fingerprint: node.client_fingerprint.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct HysteriaProxySettings {
    pub auth_str: Option<SensitiveValue>,
    pub auth: Option<SensitiveValue>,
    pub protocol: Option<Value>,
    pub obfs: Option<Value>,
    pub up: Option<Value>,
    pub down: Option<Value>,
}

impl HysteriaProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            auth_str: sensitive(extension_value(node, "auth-str")),
            auth: sensitive(extension_value(node, "auth")),
            protocol: extension_value(node, "protocol"),
            obfs: extension_value(node, "obfs"),
            up: extension_value(node, "up"),
            down: extension_value(node, "down"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Hysteria2ProxySettings {
    pub password: Option<SensitiveValue>,
    pub auth: Option<SensitiveValue>,
    pub obfs: Option<Value>,
    pub obfs_password: Option<SensitiveValue>,
    pub hop_interval: Option<Value>,
    pub up: Option<Value>,
    pub down: Option<Value>,
}

impl Hysteria2ProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            password: sensitive(node.password.clone()),
            auth: sensitive(extension_value(node, "auth")),
            obfs: extension_value(node, "obfs"),
            obfs_password: sensitive(extension_value(node, "obfs-password")),
            hop_interval: extension_value(node, "hop-interval"),
            up: extension_value(node, "up"),
            down: extension_value(node, "down"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct WireguardProxySettings {
    pub ip: Option<Value>,
    pub ipv6: Option<Value>,
    pub private_key: Option<SensitiveValue>,
    pub public_key: Option<Value>,
    pub pre_shared_key: Option<SensitiveValue>,
    pub dns: Option<Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TailscaleProxySettings {
    pub hostname: Option<Value>,
    pub auth_key: Option<SensitiveValue>,
    pub control_url: Option<Value>,
    pub state_dir: Option<Value>,
    pub exit_node: Option<Value>,
}

impl TailscaleProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            hostname: extension_value(node, "hostname"),
            auth_key: sensitive(extension_value(node, "auth-key")),
            control_url: extension_value(node, "control-url"),
            state_dir: extension_value(node, "state-dir"),
            exit_node: extension_value(node, "exit-node"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct OpenvpnProxySettings {
    pub proto: Option<Value>,
    pub ca: Option<SensitiveValue>,
    pub cert: Option<SensitiveValue>,
    pub key: Option<SensitiveValue>,
    pub tls_crypt: Option<SensitiveValue>,
}

impl OpenvpnProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            proto: extension_value(node, "proto"),
            ca: sensitive(extension_value(node, "ca")),
            cert: sensitive(extension_value(node, "cert")),
            key: sensitive(extension_value(node, "key")),
            tls_crypt: sensitive(extension_value(node, "tls-crypt")),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct MasqueProxySettings {
    pub private_key: Option<SensitiveValue>,
    pub public_key: Option<Value>,
    pub ip: Option<Value>,
    pub ipv6: Option<Value>,
    pub mtu: Option<Value>,
    pub network: Option<Value>,
}

impl MasqueProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            private_key: sensitive(extension_value(node, "private-key")),
            public_key: extension_value(node, "public-key"),
            ip: extension_value(node, "ip"),
            ipv6: extension_value(node, "ipv6"),
            mtu: extension_value(node, "mtu"),
            network: node.network.clone(),
        }
    }
}

impl WireguardProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            ip: extension_value(node, "ip"),
            ipv6: extension_value(node, "ipv6"),
            private_key: sensitive(extension_value(node, "private-key")),
            public_key: extension_value(node, "public-key"),
            pre_shared_key: sensitive(first_extension_value(
                node,
                &["pre-shared-key", "preshared-key"],
            )),
            dns: extension_value(node, "dns"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TuicProxySettings {
    pub uuid: Option<Value>,
    pub password: Option<SensitiveValue>,
    pub token: Option<SensitiveValue>,
    pub alpn: Vec<String>,
    pub congestion_controller: Option<Value>,
}

impl TuicProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            uuid: node.uuid.clone(),
            password: sensitive(node.password.clone()),
            token: sensitive(extension_value(node, "token")),
            alpn: node.alpn.clone(),
            congestion_controller: extension_value(node, "congestion-controller"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SshProxySettings {
    pub username: Option<Value>,
    pub password: Option<SensitiveValue>,
    pub private_key: Option<SensitiveValue>,
    pub private_key_passphrase: Option<SensitiveValue>,
}

impl SshProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            username: node.username.clone(),
            password: sensitive(node.password.clone()),
            private_key: sensitive(first_extension_value(node, &["privateKey", "private-key"])),
            private_key_passphrase: sensitive(first_extension_value(
                node,
                &["private-key-passphrase", "privateKeyPassphrase"],
            )),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct MieruProxySettings {
    pub username: Option<Value>,
    pub password: Option<SensitiveValue>,
    pub transport: Option<Value>,
    pub multiplexing: Option<Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct SudokuProxySettings {
    pub key: Option<SensitiveValue>,
    pub aead_method: Option<Value>,
    pub padding_min: Option<Value>,
    pub padding_max: Option<Value>,
    pub table_type: Option<Value>,
    pub httpmask: Option<Value>,
}

impl SudokuProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            key: sensitive(extension_value(node, "key")),
            aead_method: extension_value(node, "aead-method"),
            padding_min: extension_value(node, "padding-min"),
            padding_max: extension_value(node, "padding-max"),
            table_type: extension_value(node, "table-type"),
            httpmask: extension_value(node, "httpmask"),
        }
    }
}

impl MieruProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            username: node.username.clone(),
            password: sensitive(node.password.clone()),
            transport: extension_value(node, "transport"),
            multiplexing: extension_value(node, "multiplexing"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct AnytlsProxySettings {
    pub password: Option<SensitiveValue>,
    pub client_fingerprint: Option<Value>,
    pub idle_session_check_interval: Option<Value>,
    pub idle_session_timeout: Option<Value>,
    pub min_idle_session: Option<Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TrusttunnelProxySettings {
    pub username: Option<Value>,
    pub password: Option<SensitiveValue>,
    pub health_check: Option<Value>,
}

impl TrusttunnelProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            username: node.username.clone(),
            password: sensitive(node.password.clone()),
            health_check: extension_value(node, "health-check"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct GostRelayProxySettings {
    pub tls: Option<bool>,
    pub mux: Option<Value>,
}

impl GostRelayProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            tls: node.tls,
            mux: extension_value(node, "mux"),
        }
    }
}

impl AnytlsProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        Self {
            password: sensitive(node.password.clone()),
            client_fingerprint: node.client_fingerprint.clone(),
            idle_session_check_interval: extension_value(node, "idle-session-check-interval"),
            idle_session_timeout: extension_value(node, "idle-session-timeout"),
            min_idle_session: extension_value(node, "min-idle-session"),
        }
    }
}

/// 未覆盖协议的摘要。完整节点在 `ProxyNodeSettings::raw` 中保存，这里只暴露安全元数据。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct RawProxySettings {
    pub kind: String,
    pub extension_field_count: usize,
    pub sensitive_field_names: Vec<String>,
}

impl RawProxySettings {
    pub(super) fn from_config(node: &ProxyNode) -> Self {
        let mut sensitive_field_names = sensitive_field_names(&node.extensions);
        if node.password.is_some() {
            sensitive_field_names.push("password".to_string());
        }
        Self {
            kind: node.kind.as_str().to_string(),
            extension_field_count: node.extensions.len(),
            sensitive_field_names,
        }
    }
}

fn extension_value(node: &ProxyNode, key: &str) -> Option<Value> {
    node.extensions.get(key).cloned()
}

fn first_extension_value(node: &ProxyNode, keys: &[&str]) -> Option<Value> {
    keys.iter().find_map(|key| extension_value(node, key))
}

fn first_string<'a>(values: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
