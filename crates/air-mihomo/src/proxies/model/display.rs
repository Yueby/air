use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use super::collection::ProxyNodeSettings;
use super::preview::yaml_value_preview;
use super::protocols::ProxyProtocolSettings;
use super::sensitive::SensitiveValue;
/// 给 UI 列表或诊断使用的安全展示结构。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyNodeDisplay {
    pub name: String,
    pub protocol: String,
    pub endpoint: String,
    pub udp: Option<bool>,
    pub dialer_proxy: Option<String>,
    pub skip_cert_verify: Option<bool>,
    pub fields: Vec<ProxyFieldPreview>,
}

impl From<&ProxyNodeSettings> for ProxyNodeDisplay {
    fn from(node: &ProxyNodeSettings) -> Self {
        let mut fields = Vec::new();
        push_protocol_preview(&mut fields, &node.protocol);
        Self {
            name: node.common.name.clone(),
            protocol: node.protocol.protocol_name().to_string(),
            endpoint: node.common.endpoint_preview(),
            udp: node.common.udp,
            dialer_proxy: node.common.dialer_proxy.clone(),
            skip_cert_verify: node.common.skip_cert_verify,
            fields,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ProxyFieldPreview {
    pub name: String,
    pub value: String,
    pub sensitive: bool,
}

fn push_protocol_preview(fields: &mut Vec<ProxyFieldPreview>, protocol: &ProxyProtocolSettings) {
    match protocol {
        ProxyProtocolSettings::Direct(settings) => {
            push_plain(fields, "interface-name", settings.interface_name.as_deref());
            push_value(fields, "routing-mark", settings.routing_mark.as_ref());
        }
        ProxyProtocolSettings::Dns(settings) => {
            push_value(fields, "target", settings.target.as_ref())
        }
        ProxyProtocolSettings::Snell(settings) => {
            push_secret(fields, "psk", settings.psk.as_ref());
            push_value(fields, "version", settings.version.as_ref());
            push_value(fields, "obfs-opts", settings.obfs_opts.as_ref());
        }
        ProxyProtocolSettings::Http(settings) => {
            push_value(fields, "username", settings.username.as_ref());
            push_secret(fields, "password", settings.password.as_ref());
            push_bool(fields, "tls", settings.tls);
            push_plain(fields, "sni", settings.sni.as_deref());
        }
        ProxyProtocolSettings::Socks5(settings) => {
            push_value(fields, "username", settings.username.as_ref());
            push_secret(fields, "password", settings.password.as_ref());
            push_bool(fields, "tls", settings.tls);
        }
        ProxyProtocolSettings::Shadowsocks(settings) => {
            push_value(fields, "cipher", settings.cipher.as_ref());
            push_secret(fields, "password", settings.password.as_ref());
            push_plain(fields, "plugin", settings.plugin.as_deref());
        }
        ProxyProtocolSettings::ShadowsocksR(settings) => {
            push_value(fields, "cipher", settings.cipher.as_ref());
            push_secret(fields, "password", settings.password.as_ref());
            push_value(fields, "obfs", settings.obfs.as_ref());
            push_value(fields, "protocol", settings.protocol.as_ref());
        }
        ProxyProtocolSettings::Vmess(settings) => {
            push_value(fields, "uuid", settings.uuid.as_ref());
            push_value(fields, "alterId", settings.alter_id.as_ref());
            push_value(fields, "network", settings.network.as_ref());
            push_bool(fields, "tls", settings.tls);
        }
        ProxyProtocolSettings::Vless(settings) => {
            push_value(fields, "uuid", settings.uuid.as_ref());
            push_value(fields, "flow", settings.flow.as_ref());
            push_value(fields, "network", settings.network.as_ref());
            push_bool(fields, "tls", settings.tls);
        }
        ProxyProtocolSettings::Trojan(settings) => {
            push_secret(fields, "password", settings.password.as_ref());
            push_value(fields, "network", settings.network.as_ref());
            push_plain(fields, "sni", settings.sni.as_deref());
        }
        ProxyProtocolSettings::Hysteria(settings) => {
            push_secret(fields, "auth-str", settings.auth_str.as_ref());
            push_secret(fields, "auth", settings.auth.as_ref());
            push_value(fields, "protocol", settings.protocol.as_ref());
            push_value(fields, "obfs", settings.obfs.as_ref());
        }
        ProxyProtocolSettings::Hysteria2(settings) => {
            push_secret(fields, "password", settings.password.as_ref());
            push_secret(fields, "auth", settings.auth.as_ref());
            push_value(fields, "obfs", settings.obfs.as_ref());
            push_secret(fields, "obfs-password", settings.obfs_password.as_ref());
        }
        ProxyProtocolSettings::Wireguard(settings) => {
            push_value(fields, "ip", settings.ip.as_ref());
            push_value(fields, "ipv6", settings.ipv6.as_ref());
            push_value(fields, "public-key", settings.public_key.as_ref());
            push_secret(fields, "private-key", settings.private_key.as_ref());
            push_secret(fields, "pre-shared-key", settings.pre_shared_key.as_ref());
        }
        ProxyProtocolSettings::Tailscale(settings) => {
            push_value(fields, "hostname", settings.hostname.as_ref());
            push_secret(fields, "auth-key", settings.auth_key.as_ref());
            push_value(fields, "control-url", settings.control_url.as_ref());
            push_value(fields, "exit-node", settings.exit_node.as_ref());
        }
        ProxyProtocolSettings::Openvpn(settings) => {
            push_value(fields, "proto", settings.proto.as_ref());
            push_secret(fields, "ca", settings.ca.as_ref());
            push_secret(fields, "cert", settings.cert.as_ref());
            push_secret(fields, "key", settings.key.as_ref());
        }
        ProxyProtocolSettings::Masque(settings) => {
            push_value(fields, "ip", settings.ip.as_ref());
            push_value(fields, "ipv6", settings.ipv6.as_ref());
            push_value(fields, "public-key", settings.public_key.as_ref());
            push_secret(fields, "private-key", settings.private_key.as_ref());
            push_value(fields, "network", settings.network.as_ref());
        }
        ProxyProtocolSettings::Tuic(settings) => {
            push_value(fields, "uuid", settings.uuid.as_ref());
            push_secret(fields, "password", settings.password.as_ref());
            push_secret(fields, "token", settings.token.as_ref());
            push_value(
                fields,
                "congestion-controller",
                settings.congestion_controller.as_ref(),
            );
        }
        ProxyProtocolSettings::Ssh(settings) => {
            push_value(fields, "username", settings.username.as_ref());
            push_secret(fields, "password", settings.password.as_ref());
            push_secret(fields, "privateKey", settings.private_key.as_ref());
        }
        ProxyProtocolSettings::Mieru(settings) => {
            push_value(fields, "username", settings.username.as_ref());
            push_secret(fields, "password", settings.password.as_ref());
            push_value(fields, "transport", settings.transport.as_ref());
        }
        ProxyProtocolSettings::Sudoku(settings) => {
            push_secret(fields, "key", settings.key.as_ref());
            push_value(fields, "aead-method", settings.aead_method.as_ref());
            push_value(fields, "table-type", settings.table_type.as_ref());
        }
        ProxyProtocolSettings::Anytls(settings) => {
            push_secret(fields, "password", settings.password.as_ref());
            push_value(
                fields,
                "client-fingerprint",
                settings.client_fingerprint.as_ref(),
            );
        }
        ProxyProtocolSettings::Trusttunnel(settings) => {
            push_value(fields, "username", settings.username.as_ref());
            push_secret(fields, "password", settings.password.as_ref());
            push_value(fields, "health-check", settings.health_check.as_ref());
        }
        ProxyProtocolSettings::GostRelay(settings) => {
            push_bool(fields, "tls", settings.tls);
            push_value(fields, "mux", settings.mux.as_ref());
        }
        ProxyProtocolSettings::Raw(settings) => {
            push_plain(fields, "raw-type", Some(settings.kind.as_str()));
            if settings.extension_field_count > 0 {
                fields.push(ProxyFieldPreview {
                    name: "extension-fields".to_string(),
                    value: settings.extension_field_count.to_string(),
                    sensitive: false,
                });
            }
        }
    }
}

fn push_plain(fields: &mut Vec<ProxyFieldPreview>, name: &str, value: Option<&str>) {
    let Some(value) = value.filter(|value| !value.trim().is_empty()) else {
        return;
    };
    fields.push(ProxyFieldPreview {
        name: name.to_string(),
        value: value.to_string(),
        sensitive: false,
    });
}

fn push_value(fields: &mut Vec<ProxyFieldPreview>, name: &str, value: Option<&Value>) {
    let Some(value) = value else {
        return;
    };
    fields.push(ProxyFieldPreview {
        name: name.to_string(),
        value: yaml_value_preview(value),
        sensitive: false,
    });
}

fn push_secret(fields: &mut Vec<ProxyFieldPreview>, name: &str, value: Option<&SensitiveValue>) {
    let Some(value) = value else {
        return;
    };
    fields.push(ProxyFieldPreview {
        name: name.to_string(),
        value: value.redacted_label().to_string(),
        sensitive: true,
    });
}

fn push_bool(fields: &mut Vec<ProxyFieldPreview>, name: &str, value: Option<bool>) {
    let Some(value) = value else {
        return;
    };
    fields.push(ProxyFieldPreview {
        name: name.to_string(),
        value: value.to_string(),
        sensitive: false,
    });
}
