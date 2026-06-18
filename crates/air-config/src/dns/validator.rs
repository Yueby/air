use std::collections::BTreeMap;
use std::net::IpAddr;

use super::nameserver::{
    NameserverValidation, is_plain_nameserver, plain_host, validate_nameserver_expression,
    validate_port,
};
use super::policy::DnsNameserverPolicySettings;
use super::settings::{DnsConfigSettings, non_empty};
use super::{DNS_CACHE_ALGORITHMS, DNS_ENHANCED_MODES, FAKE_IP_FILTER_MODES};
use crate::{ConfigDiagnostic, ConfigDiagnosticSeverity};
struct DnsConfigValidator<'a> {
    settings: &'a DnsConfigSettings,
    diagnostics: Vec<ConfigDiagnostic>,
}

impl<'a> DnsConfigValidator<'a> {
    fn new(settings: &'a DnsConfigSettings) -> Self {
        Self {
            settings,
            diagnostics: Vec::new(),
        }
    }

    fn validate(&mut self) {
        self.validate_known_values();
        self.validate_listen();
        self.validate_fake_ip();
        self.validate_nameservers();
        self.validate_policies();
        self.validate_interactions();
    }

    fn validate_known_values(&mut self) {
        self.check_known_value(
            "dns.cache-algorithm",
            self.settings.cache_algorithm.as_deref(),
            DNS_CACHE_ALGORITHMS,
        );
        self.check_known_value(
            "dns.enhanced-mode",
            self.settings.enhanced_mode.as_deref(),
            DNS_ENHANCED_MODES,
        );
        self.check_known_value(
            "dns.fake-ip-filter-mode",
            self.settings.fake_ip_filter_mode.as_deref(),
            FAKE_IP_FILTER_MODES,
        );
    }

    fn validate_listen(&mut self) {
        let Some(listen) = self.settings.listen.as_deref() else {
            return;
        };
        if let Err(message) = parse_listen_endpoint(listen) {
            self.diagnostics.push(ConfigDiagnostic::error(
                "dns.listen",
                message,
                Some(
                    "请使用 host:port、:port 或 [IPv6]:port 格式，端口范围为 1-65535。".to_string(),
                ),
            ));
        }
    }

    fn validate_fake_ip(&mut self) {
        if let Some(range) = non_empty(self.settings.fake_ip_range.as_deref()) {
            match parse_cidr(range) {
                Ok((ip, _)) if ip.is_ipv4() => {}
                Ok(_) => self.diagnostics.push(ConfigDiagnostic::error(
                    "dns.fake-ip-range",
                    format!("fake-ip-range `{range}` 必须是 IPv4 CIDR"),
                    Some("IPv6 地址池请使用 fake-ip-range6。".to_string()),
                )),
                Err(message) => self.diagnostics.push(ConfigDiagnostic::error(
                    "dns.fake-ip-range",
                    message,
                    Some("请填写合法 IPv4 CIDR，例如 198.18.0.1/16。".to_string()),
                )),
            }
        }

        if let Some(range) = non_empty(self.settings.fake_ip_range6.as_deref()) {
            match parse_cidr(range) {
                Ok((ip, _)) if ip.is_ipv6() => {}
                Ok(_) => self.diagnostics.push(ConfigDiagnostic::error(
                    "dns.fake-ip-range6",
                    format!("fake-ip-range6 `{range}` 必须是 IPv6 CIDR"),
                    Some("IPv4 地址池请使用 fake-ip-range。".to_string()),
                )),
                Err(message) => self.diagnostics.push(ConfigDiagnostic::error(
                    "dns.fake-ip-range6",
                    message,
                    Some("请填写合法 IPv6 CIDR，例如 fdfe:dcba:9876::1/64。".to_string()),
                )),
            }
        }

        for (index, value) in self.settings.fake_ip_filter.iter().enumerate() {
            let value = value.trim();
            if value.is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("dns.fake-ip-filter[{index}]"),
                    "fake-ip-filter 条目不能为空",
                    Some(
                        "请删除空条目，或填写域名、geosite、rule-set 或规则模式条目。".to_string(),
                    ),
                ));
            }
        }
    }

    fn validate_nameservers(&mut self) {
        self.check_nameserver_list(
            "dns.default-nameserver",
            &self.settings.default_nameserver,
            true,
        );
        self.check_nameserver_list("dns.nameserver", &self.settings.nameserver, false);
        self.check_nameserver_list("dns.fallback", &self.settings.fallback, false);
        self.check_nameserver_list(
            "dns.proxy-server-nameserver",
            &self.settings.proxy_server_nameserver,
            false,
        );
        self.check_nameserver_list(
            "dns.direct-nameserver",
            &self.settings.direct_nameserver,
            false,
        );
    }

    fn validate_policies(&mut self) {
        self.check_policy_list("dns.nameserver-policy", &self.settings.nameserver_policy);
        self.check_policy_list(
            "dns.proxy-server-nameserver-policy",
            &self.settings.proxy_server_nameserver_policy,
        );
    }

    fn validate_interactions(&mut self) {
        if self.settings.enhanced_mode.as_deref() == Some("fake-ip")
            && self
                .settings
                .fake_ip_range
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
        {
            self.diagnostics.push(ConfigDiagnostic::warning(
                "dns.fake-ip-range",
                "fake-ip 模式缺少 IPv4 地址池",
                Some("请配置 fake-ip-range，例如 198.18.0.1/16。".to_string()),
            ));
        }

        if self.settings.fake_ip_filter_mode.as_deref() == Some("rule") {
            self.diagnostics.push(ConfigDiagnostic::info(
                "dns.fake-ip-filter-mode",
                "fake-ip 规则模式会按 fake-ip-filter 顺序决定返回 fake-ip 或 real-ip",
                Some(
                    "请确认 MATCH 兜底和 rule-provider behavior，避免与路由 rules 的预期相反。"
                        .to_string(),
                ),
            ));
        }

        if self.settings.respect_rules == Some(true)
            && self.settings.proxy_server_nameserver.is_empty()
        {
            self.diagnostics.push(ConfigDiagnostic::warning(
                "dns.respect-rules",
                "respect-rules 需要配合 proxy-server-nameserver 使用才稳定",
                Some("请配置 proxy-server-nameserver，或关闭 respect-rules。".to_string()),
            ));
        }
    }

    fn check_nameserver_list(&mut self, path: &str, values: &[String], default_server: bool) {
        for (index, value) in values.iter().enumerate() {
            let path = format!("{path}[{index}]");
            let value = value.trim();
            if value.is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    path,
                    "DNS 服务器不能为空",
                    Some("请删除空条目，或填写 IP、udp://、tcp://、tls://、https:// 等服务器表达式。".to_string()),
                ));
                continue;
            }

            match validate_nameserver_expression(value) {
                NameserverValidation::Ok => {}
                NameserverValidation::Warning(message) => self
                    .diagnostics
                    .push(ConfigDiagnostic::warning(
                    path.clone(),
                    message,
                    Some(
                        "如果这是 mihomo 新增协议，可保留；否则请改为受支持的 DNS 服务器表达式。"
                            .to_string(),
                    ),
                )),
                NameserverValidation::Error(message) => self
                    .diagnostics
                    .push(ConfigDiagnostic::error(
                    path.clone(),
                    message,
                    Some(
                        "请填写 IP、IP:port、udp://、tcp://、tls://、https:// 或 rcode://success。"
                            .to_string(),
                    ),
                )),
            }

            if default_server
                && is_plain_nameserver(value)
                && plain_host(value).is_some_and(|host| host.parse::<IpAddr>().is_err())
            {
                self.diagnostics.push(ConfigDiagnostic::warning(
                    path,
                    "default-nameserver 使用普通域名可能造成启动阶段递归解析",
                    Some(
                        "建议 default-nameserver 使用纯 IP、system 或加密 DNS 的 IP 端点。"
                            .to_string(),
                    ),
                ));
            }
        }
    }

    fn check_policy_list(&mut self, path: &str, policies: &[DnsNameserverPolicySettings]) {
        let mut seen = BTreeMap::<String, usize>::new();
        for (index, policy) in policies.iter().enumerate() {
            let item_path = format!("{path}[{index}]");
            let matcher = policy.matcher.trim();
            if matcher.is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{item_path}.matcher"),
                    "DNS 策略匹配条件不能为空",
                    Some("请填写域名、+.example.com、geosite:cn 或 rule-set:name。".to_string()),
                ));
            } else if let Err(message) = validate_policy_matcher(matcher) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{item_path}.matcher"),
                    message,
                    Some(
                        "支持域名、通配域名、geosite:*、rule-set:* 和常见 DOMAIN 类匹配器。"
                            .to_string(),
                    ),
                ));
            }

            let normalized = matcher.to_ascii_lowercase();
            if !normalized.is_empty()
                && let Some(first_index) = seen.insert(normalized, index)
            {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{item_path}.matcher"),
                    format!("DNS 策略匹配条件 `{matcher}` 与第 {first_index} 项重复"),
                    Some("请合并重复策略，避免写回时互相覆盖。".to_string()),
                ));
            }

            if policy.passthrough.is_some() && policy.nameservers.is_empty() {
                continue;
            }

            self.check_nameserver_list(
                &format!("{item_path}.nameservers"),
                &policy.nameservers,
                false,
            );
        }
    }

    fn check_known_value(&mut self, path: &str, value: Option<&str>, allowed: &[&str]) {
        let Some(value) = non_empty(value) else {
            return;
        };
        if !allowed.contains(&value) {
            self.diagnostics.push(ConfigDiagnostic::error(
                path,
                format!("`{value}` 不是支持的取值"),
                Some(format!("可选值: {}", allowed.join(", "))),
            ));
        }
    }
}

fn parse_listen_endpoint(value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("监听地址不能为空".to_string());
    }

    let (host, port) = if let Some(rest) = value.strip_prefix('[') {
        let (host, rest) = rest
            .split_once(']')
            .ok_or_else(|| format!("监听地址 `{value}` 缺少 IPv6 右括号"))?;
        let port = rest
            .strip_prefix(':')
            .ok_or_else(|| format!("监听地址 `{value}` 缺少端口分隔符"))?;
        (host, port)
    } else {
        let (host, port) = value
            .rsplit_once(':')
            .ok_or_else(|| format!("监听地址 `{value}` 缺少端口"))?;
        if host.contains(':') {
            return Err(format!("IPv6 监听地址 `{value}` 需要使用 [addr]:port 格式"));
        }
        (host, port)
    };

    let port = port
        .parse::<u32>()
        .map_err(|_| format!("监听地址 `{value}` 的端口不是整数"))?;
    validate_port(port, value)?;

    if host.is_empty() || host == "*" || host == "localhost" || host.parse::<IpAddr>().is_ok() {
        Ok(())
    } else {
        Err(format!("监听地址 `{value}` 的主机部分格式无效"))
    }
}

fn validate_policy_matcher(value: &str) -> Result<(), String> {
    if value.chars().any(char::is_whitespace) {
        return Err(format!("DNS 策略匹配条件 `{value}` 不能包含空白字符"));
    }

    let lower = value.to_ascii_lowercase();
    for prefix in ["geosite:", "rule-set:"] {
        if lower.starts_with(prefix) {
            let payload = &value[prefix.len()..];
            return validate_named_matcher_payload(value, payload);
        }
    }

    for prefix in [
        "domain:",
        "domain-suffix:",
        "domain-keyword:",
        "domain-regex:",
    ] {
        if lower.starts_with(prefix) {
            let payload = &value[prefix.len()..];
            if payload.trim().is_empty() {
                return Err(format!("DNS 策略匹配条件 `{value}` 缺少匹配内容"));
            }
            return Ok(());
        }
    }

    if value.contains(':') {
        return Err(format!("DNS 策略匹配条件 `{value}` 使用了未知前缀"));
    }

    for token in value.split(',') {
        validate_domain_pattern(token.trim(), value)?;
    }
    Ok(())
}

fn validate_named_matcher_payload(original: &str, payload: &str) -> Result<(), String> {
    if payload.trim().is_empty() {
        return Err(format!("DNS 策略匹配条件 `{original}` 缺少名称"));
    }

    for name in payload.split(',') {
        let name = name.trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        {
            return Err(format!("DNS 策略匹配条件 `{original}` 包含无效名称"));
        }
    }
    Ok(())
}

fn validate_domain_pattern(value: &str, original: &str) -> Result<(), String> {
    let domain = value
        .strip_prefix("+.")
        .or_else(|| value.strip_prefix("*."))
        .or_else(|| value.strip_prefix('.'))
        .unwrap_or(value);

    if domain.is_empty() || domain.contains("..") {
        return Err(format!("DNS 策略匹配条件 `{original}` 包含无效域名"));
    }

    for label in domain.split('.') {
        if label.is_empty() {
            return Err(format!("DNS 策略匹配条件 `{original}` 包含空标签"));
        }
        if label == "*" {
            continue;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(format!(
                "DNS 策略匹配条件 `{original}` 的标签不能以短横线开头或结尾"
            ));
        }
        if !label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
        {
            return Err(format!("DNS 策略匹配条件 `{original}` 包含不支持的字符"));
        }
    }

    if value.contains('*') && !(value == "*" || value.starts_with("*.") || domain == "*") {
        return Err(format!(
            "DNS 策略匹配条件 `{original}` 的通配符只能作为完整标签使用"
        ));
    }
    Ok(())
}

fn parse_cidr(value: &str) -> Result<(IpAddr, u8), String> {
    let (ip, prefix) = value
        .split_once('/')
        .ok_or_else(|| format!("CIDR `{value}` 缺少前缀长度"))?;
    let ip = ip
        .parse::<IpAddr>()
        .map_err(|_| format!("CIDR `{value}` 的 IP 地址无效"))?;
    let prefix = prefix
        .parse::<u8>()
        .map_err(|_| format!("CIDR `{value}` 的前缀长度不是整数"))?;
    let max_prefix = if ip.is_ipv4() { 32 } else { 128 };
    if prefix > max_prefix {
        return Err(format!("CIDR `{value}` 的前缀长度超过 {max_prefix}"));
    }
    Ok((ip, prefix))
}

pub(super) fn validate_settings(settings: &DnsConfigSettings) -> Vec<ConfigDiagnostic> {
    let mut validator = DnsConfigValidator::new(settings);
    validator.validate();
    validator.diagnostics
}

pub fn has_dns_error(diagnostics: &[ConfigDiagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ConfigDiagnosticSeverity::Error)
}
