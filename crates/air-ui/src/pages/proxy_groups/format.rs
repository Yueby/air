use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use air_config::model::ProxyGroupKind;
pub(crate) fn is_automatic_group(kind: &ProxyGroupKind) -> bool {
    matches!(
        kind,
        ProxyGroupKind::UrlTest | ProxyGroupKind::Fallback | ProxyGroupKind::LoadBalance
    )
}

#[cfg(test)]
pub(crate) fn group_sort_kind(kind: &str) -> u8 {
    match kind.trim().to_ascii_lowercase().as_str() {
        "select" | "selector" => 0,
        "url-test" | "urltest" => 1,
        "fallback" => 2,
        "load-balance" | "loadbalance" => 3,
        _ => 4,
    }
}

pub(crate) fn proxy_group_type_display_label(kind: &str) -> &str {
    match kind.trim().to_ascii_lowercase().as_str() {
        "select" | "selector" => "Selector",
        "fallback" => "Fallback",
        "load-balance" | "loadbalance" => "LoadBalance",
        "url-test" | "urltest" => "URLTest",
        "relay" => "Relay",
        _ => kind,
    }
}

pub(crate) fn proxy_type_display_label(kind: &str) -> &str {
    match kind.trim().to_ascii_lowercase().as_str() {
        "direct" => "DIRECT",
        "reject" => "REJECT",
        "reject-drop" | "rejectdrop" => "REJECT-DROP",
        "pass" => "PASS",
        "dns" => "DNS",
        "http" => "HTTP",
        "socks" | "socks5" => "SOCKS",
        "ss" | "shadowsocks" => "Shadowsocks",
        "ssr" | "shadowsocksr" => "ShadowsocksR",
        "snell" => "Snell",
        "vmess" => "VMess",
        "vless" => "VLESS",
        "trojan" => "Trojan",
        "anytls" | "any-tls" => "AnyTLS",
        "mieru" => "Mieru",
        "sudoku" => "Sudoku",
        "hysteria" => "Hysteria",
        "hysteria2" => "Hysteria2",
        "tuic" => "TUIC",
        "wireguard" | "wire-guard" => "WireGuard",
        "tailscale" => "Tailscale",
        "ssh" => "SSH",
        "masque" => "MASQUE",
        "trusttunnel" | "trust-tunnel" => "TrustTunnel",
        "openvpn" | "open-vpn" => "OpenVPN",
        _ => kind,
    }
}

pub(crate) fn element_id_fragment(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
        } else if ch == '-' || ch == '_' {
            normalized.push(ch);
        } else {
            normalized.push('-');
        }
    }
    let normalized = normalized.trim_matches('-');
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!(
        "{}-{:x}",
        if normalized.is_empty() {
            "item"
        } else {
            normalized
        },
        hasher.finish()
    )
}

pub(crate) fn split_member_lines(value: &str) -> Vec<String> {
    value
        .lines()
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn optional_form_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}
