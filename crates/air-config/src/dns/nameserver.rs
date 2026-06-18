use std::net::{IpAddr, SocketAddr};

use super::view_model::{DnsNameserverProtocol, DnsNameserverViewModel};

pub(super) enum NameserverValidation {
    Ok,
    Warning(String),
    Error(String),
}

pub(super) fn validate_nameserver_expression(value: &str) -> NameserverValidation {
    if value.chars().any(char::is_whitespace) {
        return NameserverValidation::Error(format!("DNS 服务器 `{value}` 不能包含空白字符"));
    }

    if matches!(value, "system" | "system://") {
        return NameserverValidation::Ok;
    }

    let base = value.split_once('#').map(|(base, _)| base).unwrap_or(value);
    if let Some((scheme, rest)) = base.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        return match scheme.as_str() {
            "https" => validate_https_nameserver(rest, value),
            "tls" | "tcp" | "udp" | "quic" => validate_network_nameserver(rest, value),
            "dhcp" | "system" => {
                if rest.trim().is_empty() && scheme != "system" {
                    NameserverValidation::Error(format!("DNS 服务器 `{value}` 缺少目标"))
                } else {
                    NameserverValidation::Ok
                }
            }
            "rcode" => {
                if rest.trim().is_empty() {
                    NameserverValidation::Error(format!("DNS 服务器 `{value}` 缺少 rcode 值"))
                } else {
                    NameserverValidation::Ok
                }
            }
            _ => NameserverValidation::Warning(format!(
                "DNS 服务器 `{value}` 使用了未识别协议 `{scheme}`"
            )),
        };
    }

    validate_plain_nameserver(base, value)
}

fn validate_https_nameserver(rest: &str, original: &str) -> NameserverValidation {
    let host = rest.split('/').next().unwrap_or_default();
    if host.trim().is_empty() {
        return NameserverValidation::Error(format!("DoH 服务器 `{original}` 缺少主机名"));
    }
    validate_host_port(host, original)
}

fn validate_network_nameserver(rest: &str, original: &str) -> NameserverValidation {
    if rest.trim().is_empty() {
        return NameserverValidation::Error(format!("DNS 服务器 `{original}` 缺少目标"));
    }
    validate_host_port(rest, original)
}

fn validate_plain_nameserver(base: &str, original: &str) -> NameserverValidation {
    let host = if let Ok(socket) = base.parse::<SocketAddr>() {
        return validate_port(socket.port() as u32, original)
            .map_or_else(NameserverValidation::Error, |_| NameserverValidation::Ok);
    } else {
        match split_host_port(base) {
            Some((host, port)) => {
                let Ok(port) = port.parse::<u32>() else {
                    return NameserverValidation::Error(format!(
                        "DNS 服务器 `{original}` 的端口不是整数"
                    ));
                };
                if let Err(message) = validate_port(port, original) {
                    return NameserverValidation::Error(message);
                }
                host
            }
            None => base,
        }
    };

    if host.parse::<IpAddr>().is_ok() || is_domain_like(host) {
        NameserverValidation::Ok
    } else {
        NameserverValidation::Error(format!("DNS 服务器 `{original}` 的主机部分格式无效"))
    }
}

fn validate_host_port(value: &str, original: &str) -> NameserverValidation {
    let host = if let Some((host, port)) = split_host_port(value) {
        let Ok(port) = port.parse::<u32>() else {
            return NameserverValidation::Error(format!("DNS 服务器 `{original}` 的端口不是整数"));
        };
        if let Err(message) = validate_port(port, original) {
            return NameserverValidation::Error(message);
        }
        host
    } else {
        strip_bracketed_ipv6(value).unwrap_or(value)
    };

    if host.parse::<IpAddr>().is_ok() || is_domain_like(host) {
        NameserverValidation::Ok
    } else {
        NameserverValidation::Error(format!("DNS 服务器 `{original}` 的主机部分格式无效"))
    }
}

pub(super) fn validate_port(port: u32, value: &str) -> Result<(), String> {
    if (1..=65_535).contains(&port) {
        Ok(())
    } else {
        Err(format!("`{value}` 的端口不在 1-65535 内"))
    }
}

pub(super) fn classify_nameserver(value: &str) -> DnsNameserverProtocol {
    let base = value.split_once('#').map(|(base, _)| base).unwrap_or(value);
    if matches!(base, "system" | "system://") {
        return DnsNameserverProtocol::System;
    }
    if let Some((scheme, _)) = base.split_once("://") {
        return match scheme.to_ascii_lowercase().as_str() {
            "udp" => DnsNameserverProtocol::Udp,
            "tcp" => DnsNameserverProtocol::Tcp,
            "tls" => DnsNameserverProtocol::Tls,
            "https" => DnsNameserverProtocol::Https,
            "quic" => DnsNameserverProtocol::Quic,
            "dhcp" => DnsNameserverProtocol::Dhcp,
            "system" => DnsNameserverProtocol::System,
            "rcode" => DnsNameserverProtocol::Rcode,
            _ => DnsNameserverProtocol::Other,
        };
    }
    plain_host(base)
        .and_then(|host| host.parse::<IpAddr>().ok())
        .map(|_| DnsNameserverProtocol::PlainIp)
        .unwrap_or(DnsNameserverProtocol::Other)
}

pub(super) fn plain_host(value: &str) -> Option<&str> {
    if value.parse::<IpAddr>().is_ok() {
        return Some(value);
    }
    if let Ok(socket) = value.parse::<SocketAddr>() {
        return Some(match socket.ip() {
            IpAddr::V4(_) => value.split(':').next().unwrap_or(value),
            IpAddr::V6(_) => strip_bracketed_ipv6(value).unwrap_or(value),
        });
    }
    split_host_port(value).map(|(host, _)| host).or(Some(value))
}

fn split_host_port(value: &str) -> Option<(&str, &str)> {
    if let Some(rest) = value.strip_prefix('[') {
        let (host, rest) = rest.split_once(']')?;
        let port = rest.strip_prefix(':')?;
        return Some((host, port));
    }

    value
        .rsplit_once(':')
        .filter(|(host, _)| !host.contains(':') && !host.is_empty())
}

fn strip_bracketed_ipv6(value: &str) -> Option<&str> {
    value
        .strip_prefix('[')
        .and_then(|rest| rest.split_once(']').map(|(host, _)| host))
}

pub(super) fn is_plain_nameserver(value: &str) -> bool {
    !matches!(value, "system" | "system://") && !value.contains("://")
}

pub(super) fn route_hint(value: &str) -> Option<&str> {
    value
        .split_once('#')
        .map(|(_, fragment)| fragment.trim())
        .filter(|fragment| !fragment.is_empty())
}

fn is_domain_like(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '_'))
}

pub(super) fn nameserver_view_models(values: &[String]) -> Vec<DnsNameserverViewModel> {
    values
        .iter()
        .map(|value| DnsNameserverViewModel::from(value.as_str()))
        .collect()
}
