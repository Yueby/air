//! 全局配置的领域编辑模型。
//!
//! `config::model::GlobalConfig` 负责 YAML 往返，本模块负责 GUI 编辑所需的业务语义：
//! 端口冲突、监听地址、鉴权格式，以及 external-controller 到 API 客户端端点的转换。

use std::collections::BTreeMap;
use std::fmt;
use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use air_config::ConfigDiagnostic;
#[cfg(test)]
use air_config::ConfigDiagnosticSeverity;
use air_config::model::{
    ExternalControllerCorsConfig, GeoxUrlConfig, MihomoConfigDocument, ProfileConfig,
};
use air_mihomo::MihomoEndpoint;

pub const MODES: &[&str] = &["rule", "global", "direct"];
pub const LOG_LEVELS: &[&str] = &["silent", "error", "warning", "info", "debug"];
pub const FIND_PROCESS_MODES: &[&str] = &["always", "strict", "off"];
pub const GEODATA_LOADERS: &[&str] = &["standard", "memconservative"];

/// 表单中需要编辑的全局配置。
///
/// 这里刻意不直接复用 YAML 模型：领域模型可以包含 `profile`、脱敏值和 UI 预览端点，
/// 同时写回时只覆盖明确归属全局设置页的字段，避免误伤 DNS、代理组、规则等其他 section。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct GlobalConfigSettings {
    pub port: Option<u32>,
    pub socks_port: Option<u32>,
    pub mixed_port: Option<u32>,
    pub redir_port: Option<u32>,
    pub tproxy_port: Option<u32>,
    pub allow_lan: Option<bool>,
    pub bind_address: Option<String>,
    pub lan_allowed_ips: Vec<String>,
    pub lan_disallowed_ips: Vec<String>,
    pub authentication: Vec<AuthenticationCredential>,
    pub skip_auth_prefixes: Vec<String>,
    pub mode: Option<String>,
    pub log_level: Option<String>,
    pub ipv6: Option<bool>,
    pub keep_alive_interval: Option<u64>,
    pub keep_alive_idle: Option<u64>,
    pub disable_keep_alive: Option<bool>,
    pub find_process_mode: Option<String>,
    /// mihomo REST API 的监听地址。API 客户端不能硬编码端口，必须由此字段和 `secret` 派生。
    pub external_controller: Option<String>,
    pub external_controller_cors: Option<ExternalControllerCorsConfig>,
    pub external_doh_server: Option<String>,
    /// `secret` 会作为 API 客户端的 Bearer Token，Debug 输出必须保持脱敏。
    pub secret: Option<SecretValue>,
    pub external_ui: Option<String>,
    pub external_ui_name: Option<String>,
    pub external_ui_url: Option<String>,
    pub unified_delay: Option<bool>,
    pub tcp_concurrent: Option<bool>,
    pub interface_name: Option<String>,
    pub routing_mark: Option<serde_yaml::Value>,
    pub geodata_mode: Option<bool>,
    pub geodata_loader: Option<String>,
    pub geo_auto_update: Option<bool>,
    pub geo_update_interval: Option<u64>,
    pub geox_url: Option<GeoxUrlConfig>,
    pub global_ua: Option<String>,
    pub profile: GlobalProfileSettings,
}

impl GlobalConfigSettings {
    /// 从完整配置文档提取全局设置，保留文档中其他 section 给各自领域模块处理。
    pub fn from_document(document: &MihomoConfigDocument) -> Self {
        let global = &document.global;
        Self {
            port: global.port,
            socks_port: global.socks_port,
            mixed_port: global.mixed_port,
            redir_port: global.redir_port,
            tproxy_port: global.tproxy_port,
            allow_lan: global.allow_lan,
            bind_address: global.bind_address.clone(),
            lan_allowed_ips: global.lan_allowed_ips.clone(),
            lan_disallowed_ips: global.lan_disallowed_ips.clone(),
            authentication: global
                .authentication
                .iter()
                .cloned()
                .map(AuthenticationCredential::new)
                .collect(),
            skip_auth_prefixes: global.skip_auth_prefixes.clone(),
            mode: global.mode.clone(),
            log_level: global.log_level.clone(),
            ipv6: global.ipv6,
            keep_alive_interval: global.keep_alive_interval,
            keep_alive_idle: global.keep_alive_idle,
            disable_keep_alive: global.disable_keep_alive,
            find_process_mode: global.find_process_mode.clone(),
            external_controller: global.external_controller.clone(),
            external_controller_cors: global.external_controller_cors.clone(),
            external_doh_server: global.external_doh_server.clone(),
            secret: global.secret.clone().map(SecretValue::new),
            external_ui: global.external_ui.clone(),
            external_ui_name: global.external_ui_name.clone(),
            external_ui_url: global.external_ui_url.clone(),
            unified_delay: global.unified_delay,
            tcp_concurrent: global.tcp_concurrent,
            interface_name: global.interface_name.clone(),
            routing_mark: global.routing_mark.clone(),
            geodata_mode: global.geodata_mode,
            geodata_loader: global.geodata_loader.clone(),
            geo_auto_update: global.geo_auto_update,
            geo_update_interval: global.geo_update_interval,
            geox_url: global.geox_url.clone(),
            global_ua: global.global_ua.clone(),
            profile: GlobalProfileSettings::from(document.profile.as_ref()),
        }
    }

    /// 将全局设置写回完整文档。未知顶层字段和非全局 section 不会被重建或清空。
    pub fn apply_to_document(&self, document: &mut MihomoConfigDocument) {
        let global = &mut document.global;
        global.port = self.port;
        global.socks_port = self.socks_port;
        global.mixed_port = self.mixed_port;
        global.redir_port = self.redir_port;
        global.tproxy_port = self.tproxy_port;
        global.allow_lan = self.allow_lan;
        global.bind_address = normalize_optional_string(self.bind_address.as_deref());
        global.lan_allowed_ips = self.lan_allowed_ips.clone();
        global.lan_disallowed_ips = self.lan_disallowed_ips.clone();
        global.authentication = self
            .authentication
            .iter()
            .map(AuthenticationCredential::as_config_value)
            .collect();
        global.skip_auth_prefixes = self.skip_auth_prefixes.clone();
        global.mode = normalize_optional_string(self.mode.as_deref());
        global.log_level = normalize_optional_string(self.log_level.as_deref());
        global.ipv6 = self.ipv6;
        global.keep_alive_interval = self.keep_alive_interval;
        global.keep_alive_idle = self.keep_alive_idle;
        global.disable_keep_alive = self.disable_keep_alive;
        global.find_process_mode = normalize_optional_string(self.find_process_mode.as_deref());
        global.external_controller = normalize_optional_string(self.external_controller.as_deref());
        global.external_controller_cors = self.external_controller_cors.clone();
        global.external_doh_server = normalize_optional_string(self.external_doh_server.as_deref());
        global.secret = self.secret.as_ref().and_then(SecretValue::non_empty_value);
        global.external_ui = normalize_optional_string(self.external_ui.as_deref());
        global.external_ui_name = normalize_optional_string(self.external_ui_name.as_deref());
        global.external_ui_url = normalize_optional_string(self.external_ui_url.as_deref());
        global.unified_delay = self.unified_delay;
        global.tcp_concurrent = self.tcp_concurrent;
        global.interface_name = normalize_optional_string(self.interface_name.as_deref());
        global.routing_mark = self.routing_mark.clone();
        global.geodata_mode = self.geodata_mode;
        global.geodata_loader = normalize_optional_string(self.geodata_loader.as_deref());
        global.geo_auto_update = self.geo_auto_update;
        global.geo_update_interval = self.geo_update_interval;
        global.geox_url = normalize_geox_url(self.geox_url.as_ref());
        global.global_ua = normalize_optional_string(self.global_ua.as_deref());

        let mut profile = document.profile.clone().unwrap_or_default();
        profile.store_selected = self.profile.store_selected;
        profile.store_fake_ip = self.profile.store_fake_ip;
        if profile.store_selected.is_none()
            && profile.store_fake_ip.is_none()
            && profile.extensions.is_empty()
        {
            document.profile = None;
        } else {
            document.profile = Some(profile);
        }
    }

    /// 执行不依赖 mihomo 进程的表单级校验。
    pub fn validate(&self) -> Vec<ConfigDiagnostic> {
        let mut validator = GlobalConfigValidator::new(self);
        validator.validate();
        validator.diagnostics
    }

    /// 为 HTTP API 客户端生成端点预览。监听通配地址会映射为本机回环地址，避免客户端连接 `0.0.0.0`。
    pub fn api_endpoint(&self) -> Option<MihomoEndpoint> {
        let value = self.external_controller.as_deref()?;

        controller_to_base_url("http", value).map(|base_url| MihomoEndpoint {
            base_url,
            secret: self.secret.as_ref().and_then(SecretValue::non_empty_value),
        })
    }
}

/// mihomo 的 profile 子配置属于顶层 section，但在 GUI 上通常和全局偏好一起编辑。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct GlobalProfileSettings {
    pub store_selected: Option<bool>,
    pub store_fake_ip: Option<bool>,
}

impl From<Option<&ProfileConfig>> for GlobalProfileSettings {
    fn from(value: Option<&ProfileConfig>) -> Self {
        match value {
            Some(profile) => Self {
                store_selected: profile.store_selected,
                store_fake_ip: profile.store_fake_ip,
            },
            None => Self::default(),
        }
    }
}

/// 鉴权条目的原始配置值。Debug 不输出密码，避免后续诊断或日志误带敏感信息。
#[derive(Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AuthenticationCredential(String);

impl AuthenticationCredential {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_config_value(&self) -> String {
        self.0.clone()
    }

    pub fn username(&self) -> Option<&str> {
        self.0.split_once(':').map(|(username, _)| username)
    }

    pub fn password_is_set(&self) -> bool {
        self.0
            .split_once(':')
            .map(|(_, password)| !password.is_empty())
            .unwrap_or(false)
    }
}

impl fmt::Debug for AuthenticationCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticationCredential")
            .field("username", &self.username())
            .field("password", &"<redacted>")
            .finish()
    }
}

/// 敏感字符串包装。序列化仍使用真实值，日志和调试输出固定脱敏。
#[derive(Clone, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    fn non_empty_value(&self) -> Option<String> {
        normalize_optional_string(Some(&self.0))
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.trim().is_empty() {
            formatter.write_str("SecretValue(<empty>)")
        } else {
            formatter.write_str("SecretValue(<redacted>)")
        }
    }
}

/// UI 表单使用字符串承载端口和路径，便于在用户输入非法内容时仍能显示原值。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct GlobalConfigFormViewModel {
    pub port: String,
    pub socks_port: String,
    pub mixed_port: String,
    pub redir_port: String,
    pub tproxy_port: String,
    pub allow_lan: BooleanFormValue,
    pub bind_address: String,
    pub authentication: Vec<AuthenticationCredentialForm>,
    pub mode: String,
    pub log_level: String,
    pub ipv6: BooleanFormValue,
    pub external_controller: String,
    pub secret_is_set: bool,
    pub external_ui: String,
    pub external_ui_name: String,
    pub external_ui_url: String,
    pub profile_store_selected: BooleanFormValue,
    pub profile_store_fake_ip: BooleanFormValue,
    pub api_endpoint_preview: Option<MihomoEndpoint>,
}

impl From<&GlobalConfigSettings> for GlobalConfigFormViewModel {
    fn from(settings: &GlobalConfigSettings) -> Self {
        Self {
            port: optional_port(settings.port),
            socks_port: optional_port(settings.socks_port),
            mixed_port: optional_port(settings.mixed_port),
            redir_port: optional_port(settings.redir_port),
            tproxy_port: optional_port(settings.tproxy_port),
            allow_lan: BooleanFormValue::from_option(settings.allow_lan),
            bind_address: settings.bind_address.clone().unwrap_or_default(),
            authentication: settings
                .authentication
                .iter()
                .map(AuthenticationCredentialForm::from)
                .collect(),
            mode: settings.mode.clone().unwrap_or_default(),
            log_level: settings.log_level.clone().unwrap_or_default(),
            ipv6: BooleanFormValue::from_option(settings.ipv6),
            external_controller: settings.external_controller.clone().unwrap_or_default(),
            secret_is_set: settings
                .secret
                .as_ref()
                .map(|secret| !secret.expose_secret().trim().is_empty())
                .unwrap_or(false),
            external_ui: settings.external_ui.clone().unwrap_or_default(),
            external_ui_name: settings.external_ui_name.clone().unwrap_or_default(),
            external_ui_url: settings.external_ui_url.clone().unwrap_or_default(),
            profile_store_selected: BooleanFormValue::from_option(settings.profile.store_selected),
            profile_store_fake_ip: BooleanFormValue::from_option(settings.profile.store_fake_ip),
            api_endpoint_preview: settings.api_endpoint(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct BooleanFormValue {
    pub value: bool,
    pub configured: bool,
}

impl BooleanFormValue {
    fn from_option(value: Option<bool>) -> Self {
        Self {
            value: value.unwrap_or_default(),
            configured: value.is_some(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct AuthenticationCredentialForm {
    pub username: String,
    pub password_is_set: bool,
}

impl From<&AuthenticationCredential> for AuthenticationCredentialForm {
    fn from(value: &AuthenticationCredential) -> Self {
        Self {
            username: value.username().unwrap_or_default().to_string(),
            password_is_set: value.password_is_set(),
        }
    }
}

struct GlobalConfigValidator<'a> {
    settings: &'a GlobalConfigSettings,
    diagnostics: Vec<ConfigDiagnostic>,
}

impl<'a> GlobalConfigValidator<'a> {
    fn new(settings: &'a GlobalConfigSettings) -> Self {
        Self {
            settings,
            diagnostics: Vec::new(),
        }
    }

    fn validate(&mut self) {
        self.validate_ports();
        self.validate_bind_address();
        self.validate_authentication();
        self.validate_known_values();
        self.validate_external_controllers();
    }

    fn validate_ports(&mut self) {
        let mut seen = BTreeMap::<u32, &'static str>::new();
        for (path, port) in [
            ("port", self.settings.port),
            ("socks-port", self.settings.socks_port),
            ("mixed-port", self.settings.mixed_port),
            ("redir-port", self.settings.redir_port),
            ("tproxy-port", self.settings.tproxy_port),
        ] {
            if let Some(port) = port {
                self.check_port(path, port);
                self.check_port_conflict(&mut seen, path, port);
            }
        }

        if let Some(endpoint) = self.settings.external_controller.as_deref() {
            if let Ok(parsed) = parse_listen_endpoint(endpoint) {
                self.check_port_conflict(&mut seen, "external-controller", parsed.port);
            }
        }
    }

    fn validate_bind_address(&mut self) {
        let Some(address) = self
            .settings
            .bind_address
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return;
        };
        if address != "*" && address.parse::<IpAddr>().is_err() {
            self.diagnostics.push(ConfigDiagnostic::error(
                "bind-address",
                format!("监听地址 `{address}` 不是有效 IP 地址"),
                Some(
                    "bind-address 只用于本机监听绑定，建议使用 `*`、`0.0.0.0` 或 `127.0.0.1`。"
                        .to_string(),
                ),
            ));
        }
    }

    fn validate_authentication(&mut self) {
        for (index, credential) in self.settings.authentication.iter().enumerate() {
            let path = format!("authentication[{index}]");
            let value = credential.as_config_value();
            if value.trim().is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    path,
                    "鉴权条目不能为空",
                    Some("请删除空条目，或填写 username:password。".to_string()),
                ));
                continue;
            }

            if value.chars().any(char::is_control) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    path,
                    "鉴权条目不能包含换行或控制字符",
                    Some("请使用单行 username:password 格式。".to_string()),
                ));
                continue;
            }

            match value.split_once(':') {
                Some((username, password)) if !username.is_empty() && !password.is_empty() => {}
                _ => self.diagnostics.push(ConfigDiagnostic::error(
                    path,
                    "鉴权条目必须使用 username:password 格式，用户名和密码都不能为空",
                    Some("请补全用户名和密码，或移除此鉴权条目。".to_string()),
                )),
            }
        }
    }

    fn validate_known_values(&mut self) {
        self.check_known_value("mode", self.settings.mode.as_deref(), MODES);
        self.check_known_value("log-level", self.settings.log_level.as_deref(), LOG_LEVELS);
        self.check_known_value(
            "find-process-mode",
            self.settings.find_process_mode.as_deref(),
            FIND_PROCESS_MODES,
        );
        self.check_known_value(
            "geodata-loader",
            self.settings.geodata_loader.as_deref(),
            GEODATA_LOADERS,
        );
    }

    fn validate_external_controllers(&mut self) {
        if let Some(endpoint) = self
            .settings
            .external_controller
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if let Err(message) = parse_listen_endpoint(endpoint) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    "external-controller",
                    message,
                    Some("请使用 host:port、:port 或 [IPv6]:port 格式。".to_string()),
                ));
            }
        }
    }

    fn check_port(&mut self, path: &str, port: u32) {
        if !is_valid_port(port) {
            self.diagnostics.push(ConfigDiagnostic::error(
                path,
                format!("端口 `{port}` 不在有效范围 1-65535 内"),
                Some("请填写 1 到 65535 之间且未被其他监听项使用的端口。".to_string()),
            ));
        }
    }

    fn check_port_conflict(
        &mut self,
        seen: &mut BTreeMap<u32, &'static str>,
        path: &'static str,
        port: u32,
    ) {
        if !is_valid_port(port) {
            return;
        }
        if let Some(first_path) = seen.insert(port, path) {
            self.diagnostics.push(ConfigDiagnostic::error(
                path,
                format!("端口 `{port}` 与 `{first_path}` 冲突"),
                Some("请为每个监听入口配置不同端口。".to_string()),
            ));
        }
    }

    fn check_known_value(&mut self, path: &str, value: Option<&str>, allowed: &[&str]) {
        let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct ListenEndpoint {
    host: String,
    port: u32,
}

fn parse_listen_endpoint(value: &str) -> Result<ListenEndpoint, String> {
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
    if !is_valid_port(port) {
        return Err(format!("监听地址 `{value}` 的端口不在 1-65535 内"));
    }
    validate_listen_host(host, value)?;
    Ok(ListenEndpoint {
        host: host.to_string(),
        port,
    })
}

fn validate_listen_host(host: &str, original: &str) -> Result<(), String> {
    if host.is_empty() || host == "*" || host == "localhost" || host.parse::<IpAddr>().is_ok() {
        return Ok(());
    }

    if host
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.'))
        && !host.starts_with('-')
        && !host.ends_with('-')
    {
        return Ok(());
    }

    Err(format!("监听地址 `{original}` 的主机部分格式无效"))
}

fn controller_to_base_url(scheme: &str, value: &str) -> Option<String> {
    let endpoint = parse_listen_endpoint(value).ok()?;
    let host = match endpoint.host.as_str() {
        "" | "*" | "0.0.0.0" => "127.0.0.1".to_string(),
        "::" | "::0" => "[::1]".to_string(),
        host if host
            .parse::<IpAddr>()
            .map(|ip| ip.is_ipv6())
            .unwrap_or(false) =>
        {
            format!("[{host}]")
        }
        host => host.to_string(),
    };
    Some(format!("{scheme}://{host}:{}", endpoint.port))
}

fn is_valid_port(port: u32) -> bool {
    (1..=65_535).contains(&port)
}

fn optional_port(port: Option<u32>) -> String {
    port.map(|port| port.to_string()).unwrap_or_default()
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_geox_url(value: Option<&GeoxUrlConfig>) -> Option<GeoxUrlConfig> {
    let value = value?;
    let normalized = GeoxUrlConfig {
        geoip: normalize_optional_string(value.geoip.as_deref()),
        geosite: normalize_optional_string(value.geosite.as_deref()),
        mmdb: normalize_optional_string(value.mmdb.as_deref()),
        asn: normalize_optional_string(value.asn.as_deref()),
        extensions: value.extensions.clone(),
    };
    // geox-url 的四个内置字段为空时不应落盘；若没有扩展字段，整个 section 也一并移除。
    if normalized.geoip.is_none()
        && normalized.geosite.is_none()
        && normalized.mmdb.is_none()
        && normalized.asn.is_none()
        && normalized.extensions.is_empty()
    {
        None
    } else {
        Some(normalized)
    }
}

#[cfg(test)]
fn has_error_at(diagnostics: &[ConfigDiagnostic], path: &str) -> bool {
    diagnostics.iter().any(|diagnostic| {
        diagnostic.severity == ConfigDiagnosticSeverity::Error && diagnostic.path == path
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use air_config::ConfigDocument;

    fn docs_document() -> ConfigDocument {
        ConfigDocument::parse(include_str!("../../../docs/config.yaml"))
            .expect("docs/config.yaml should parse")
    }

    #[test]
    fn extracts_global_fields_from_docs_config() {
        let document = docs_document();
        let settings = GlobalConfigSettings::from_document(&document.typed);

        assert_eq!(settings.mixed_port, Some(10801));
        assert_eq!(settings.allow_lan, Some(true));
        assert_eq!(settings.bind_address.as_deref(), Some("*"));
        assert_eq!(settings.authentication.len(), 1);
        assert_eq!(settings.mode.as_deref(), Some("rule"));
        assert_eq!(settings.log_level.as_deref(), Some("debug"));
        assert_eq!(settings.ipv6, Some(true));
        assert_eq!(
            settings.external_controller.as_deref(),
            Some("0.0.0.0:9090")
        );
        assert_eq!(settings.external_ui.as_deref(), Some("/path/to/ui/folder/"));
        assert_eq!(settings.profile.store_fake_ip, Some(true));
    }

    #[test]
    fn writes_back_global_fields_without_touching_other_sections() {
        let mut document = docs_document().typed;
        let original_dns = document.dns.clone();
        let original_proxies = document.proxies.clone();
        let original_rules = document.rules.clone();

        let mut settings = GlobalConfigSettings::from_document(&document);
        settings.mixed_port = Some(19090);
        settings.allow_lan = Some(false);
        settings.profile.store_selected = Some(true);
        settings.apply_to_document(&mut document);

        assert_eq!(document.global.mixed_port, Some(19090));
        assert_eq!(document.global.allow_lan, Some(false));
        assert_eq!(
            document
                .profile
                .as_ref()
                .and_then(|profile| profile.store_selected),
            Some(true)
        );
        assert_eq!(document.dns, original_dns);
        assert_eq!(document.proxies, original_proxies);
        assert_eq!(document.rules, original_rules);
    }

    #[test]
    fn writes_back_geox_url_without_blank_builtin_fields() {
        let mut document = docs_document().typed;
        let settings = GlobalConfigSettings {
            geox_url: Some(GeoxUrlConfig {
                geoip: Some("".into()),
                geosite: Some(" https://example.test/geosite.dat ".into()),
                mmdb: Some("   ".into()),
                asn: None,
                ..GeoxUrlConfig::default()
            }),
            ..Default::default()
        };

        settings.apply_to_document(&mut document);

        let geox_url = document.global.geox_url.unwrap();
        assert_eq!(
            geox_url.geosite.as_deref(),
            Some("https://example.test/geosite.dat")
        );
        assert!(geox_url.geoip.is_none());
        assert!(geox_url.mmdb.is_none());
        assert!(geox_url.asn.is_none());
    }

    #[test]
    fn validates_ports_addresses_and_authentication() {
        let settings = GlobalConfigSettings {
            port: Some(7890),
            socks_port: Some(7890),
            bind_address: Some("not an ip".into()),
            authentication: vec![AuthenticationCredential::new("missing-colon")],
            mode: Some("invalid".into()),
            external_controller: Some("0.0.0.0:7890".into()),
            ..Default::default()
        };

        let diagnostics = settings.validate();

        assert!(has_error_at(&diagnostics, "socks-port"));
        assert!(has_error_at(&diagnostics, "external-controller"));
        assert!(has_error_at(&diagnostics, "bind-address"));
        assert!(has_error_at(&diagnostics, "authentication[0]"));
        assert!(has_error_at(&diagnostics, "mode"));
    }

    #[test]
    fn optional_empty_listen_fields_are_not_required() {
        let settings = GlobalConfigSettings {
            bind_address: Some(" ".into()),
            external_controller: Some(String::new()),
            ..Default::default()
        };

        let diagnostics = settings.validate();

        assert!(!has_error_at(&diagnostics, "bind-address"));
        assert!(!has_error_at(&diagnostics, "external-controller"));
    }

    #[test]
    fn api_endpoint_uses_external_controller_and_secret() {
        let settings = GlobalConfigSettings {
            external_controller: Some("0.0.0.0:9093".into()),
            secret: Some(SecretValue::new("secret-value")),
            ..Default::default()
        };

        let endpoint = settings.api_endpoint().expect("endpoint should be derived");

        assert_eq!(endpoint.base_url, "http://127.0.0.1:9093");
        assert_eq!(endpoint.secret.as_deref(), Some("secret-value"));
    }

    #[test]
    fn prepares_form_view_model_without_exposing_secret_value() {
        let settings = GlobalConfigSettings {
            mixed_port: Some(7890),
            allow_lan: Some(true),
            authentication: vec![AuthenticationCredential::new("user:pass")],
            external_controller: Some(":9090".into()),
            secret: Some(SecretValue::new("secret-value")),
            ..Default::default()
        };

        let view_model = GlobalConfigFormViewModel::from(&settings);

        assert_eq!(view_model.mixed_port, "7890");
        assert_eq!(view_model.allow_lan.value, true);
        assert_eq!(view_model.allow_lan.configured, true);
        assert_eq!(view_model.authentication[0].username, "user");
        assert!(view_model.authentication[0].password_is_set);
        assert!(view_model.secret_is_set);
        assert_eq!(
            view_model.api_endpoint_preview.unwrap().base_url,
            "http://127.0.0.1:9090"
        );
        assert!(!format!("{settings:?}").contains("secret-value"));
        assert!(!format!("{settings:?}").contains("user:pass"));
    }
}
