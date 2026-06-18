//! TUN 配置的领域编辑模型。
//!
//! YAML 模型只负责安全往返；本模块负责 GUI 表单需要的字段分组、格式校验和平台提示。
//! TUN 会改写系统流量入口、DNS 目标和路由表，其中 `auto-route`、`auto-redirect`、
//! `strict-route` 等选项属于高风险项：它们可能导致断网、局域网不可达或需要管理员权限。
//! 因此本模块只输出诊断，不执行权限提升，也不修改系统路由。

use std::net::{IpAddr, SocketAddr};

use serde::{Deserialize, Serialize};

use crate::{PlatformKind, current_platform_kind};

use super::model::{MihomoConfigDocument, TunConfig};
use super::{ConfigDiagnostic, ConfigDiagnosticSeverity};

pub const TUN_STACKS: &[&str] = &["system", "gvisor", "mixed"];

const REMOVED_TUN_FIELDS: &[&str] = &[
    "inet4-route-address",
    "inet6-route-address",
    "inet4-route-exclude-address",
    "inet6-route-exclude-address",
    "include-uid",
    "include-uid-range",
    "exclude-uid",
    "exclude-uid-range",
    "include-android-user",
    "include-package",
    "exclude-package",
];

/// TUN 选项的权限分级。GUI 可据此把普通表单项与需要管理员权限的高风险项分开展示。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TunOptionPrivilege {
    Normal,
    RequiresAdmin,
}

/// 单个 TUN 表单字段的展示元数据。
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TunOptionMetadata {
    pub field: String,
    pub privilege: TunOptionPrivilege,
    pub high_risk: bool,
}

impl Default for TunOptionMetadata {
    fn default() -> Self {
        Self {
            field: String::new(),
            privilege: TunOptionPrivilege::Normal,
            high_risk: false,
        }
    }
}

/// GUI 编辑 TUN 所需的领域模型。
///
/// 字段与 mihomo `tun` section 一一对应，但写回时只覆盖本模型明确负责的字段；
/// 未建模或未来版本新增字段仍留在 `TunConfig.extensions` 中。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TunConfigSettings {
    pub enable: Option<bool>,
    pub stack: Option<String>,
    pub device: Option<String>,
    pub dns_hijack: Vec<String>,
    pub auto_detect_interface: Option<bool>,
    pub auto_route: Option<bool>,
    pub auto_redirect: Option<bool>,
    pub strict_route: Option<bool>,
    pub mtu: Option<u32>,
    pub gso: Option<bool>,
    pub gso_max_size: Option<u32>,
    pub inet6_address: Option<String>,
    pub udp_timeout: Option<u64>,
    pub iproute2_table_index: Option<u32>,
    pub iproute2_rule_index: Option<u32>,
    pub endpoint_independent_nat: Option<bool>,
    pub route_address: Vec<String>,
    pub route_exclude_address: Vec<String>,
    pub inet4_address: Vec<String>,
    pub route_address_set: Vec<String>,
    pub route_exclude_address_set: Vec<String>,
    pub include_interface: Vec<String>,
    pub exclude_interface: Vec<String>,
}

impl TunConfigSettings {
    pub fn from_document(document: &MihomoConfigDocument) -> Self {
        document
            .tun
            .as_ref()
            .map(Self::from_config)
            .unwrap_or_default()
    }

    pub fn from_config(config: &TunConfig) -> Self {
        Self {
            enable: config.enable,
            stack: config.stack.clone(),
            device: config.device.clone(),
            dns_hijack: config.dns_hijack.clone(),
            auto_detect_interface: config.auto_detect_interface,
            auto_route: config.auto_route,
            auto_redirect: config.auto_redirect,
            strict_route: config.strict_route,
            mtu: config.mtu,
            gso: config.gso,
            gso_max_size: config.gso_max_size,
            inet6_address: config.inet6_address.clone(),
            udp_timeout: config.udp_timeout,
            iproute2_table_index: config.iproute2_table_index,
            iproute2_rule_index: config.iproute2_rule_index,
            endpoint_independent_nat: config.endpoint_independent_nat,
            route_address: config.route_address.clone(),
            route_exclude_address: config.route_exclude_address.clone(),
            inet4_address: config.inet4_address.clone(),
            route_address_set: config.route_address_set.clone(),
            route_exclude_address_set: config.route_exclude_address_set.clone(),
            include_interface: config.include_interface.clone(),
            exclude_interface: config.exclude_interface.clone(),
        }
    }

    /// 将 TUN 设置写回完整文档，保留 tun 内的未知扩展字段。
    pub fn apply_to_document(&self, document: &mut MihomoConfigDocument) {
        let mut tun = document.tun.clone().unwrap_or_default();
        tun.enable = self.enable;
        tun.stack = normalize_optional_string(self.stack.as_deref());
        tun.device = normalize_optional_string(self.device.as_deref());
        tun.dns_hijack = normalized_strings(&self.dns_hijack);
        tun.auto_detect_interface = self.auto_detect_interface;
        tun.auto_route = self.auto_route;
        tun.auto_redirect = self.auto_redirect;
        tun.strict_route = self.strict_route;
        tun.mtu = self.mtu;
        tun.gso = self.gso;
        tun.gso_max_size = self.gso_max_size;
        tun.inet6_address = normalize_optional_string(self.inet6_address.as_deref());
        tun.udp_timeout = self.udp_timeout;
        tun.iproute2_table_index = self.iproute2_table_index;
        tun.iproute2_rule_index = self.iproute2_rule_index;
        tun.endpoint_independent_nat = self.endpoint_independent_nat;
        tun.route_address = normalized_strings(&self.route_address);
        tun.route_exclude_address = normalized_strings(&self.route_exclude_address);
        tun.inet4_address = normalized_strings(&self.inet4_address);
        tun.route_address_set = normalized_strings(&self.route_address_set);
        tun.route_exclude_address_set = normalized_strings(&self.route_exclude_address_set);
        tun.include_interface = normalized_strings(&self.include_interface);
        tun.exclude_interface = normalized_strings(&self.exclude_interface);
        // 这些字段已从设置页和领域模型移除；写回时显式清理，避免旧配置经扩展字段继续保留。
        for removed_field in REMOVED_TUN_FIELDS {
            tun.extensions.remove(*removed_field);
        }

        if self.is_empty() && tun.extensions.is_empty() {
            document.tun = None;
        } else {
            document.tun = Some(tun);
        }
    }

    pub fn validate(&self) -> Vec<ConfigDiagnostic> {
        let mut validator = TunConfigValidator::new(self, current_platform_kind());
        validator.validate();
        validator.diagnostics
    }

    pub fn validate_for_platform(&self, platform: PlatformKind) -> Vec<ConfigDiagnostic> {
        let mut validator = TunConfigValidator::new(self, platform);
        validator.validate();
        validator.diagnostics
    }

    pub fn option_metadata() -> Vec<TunOptionMetadata> {
        [
            ("enable", TunOptionPrivilege::RequiresAdmin, true),
            ("stack", TunOptionPrivilege::Normal, false),
            ("device", TunOptionPrivilege::Normal, false),
            ("dns-hijack", TunOptionPrivilege::RequiresAdmin, true),
            (
                "auto-detect-interface",
                TunOptionPrivilege::RequiresAdmin,
                true,
            ),
            ("auto-route", TunOptionPrivilege::RequiresAdmin, true),
            ("auto-redirect", TunOptionPrivilege::RequiresAdmin, true),
            ("strict-route", TunOptionPrivilege::RequiresAdmin, true),
            ("mtu", TunOptionPrivilege::Normal, false),
            ("gso", TunOptionPrivilege::Normal, false),
            ("gso-max-size", TunOptionPrivilege::Normal, false),
            ("inet6-address", TunOptionPrivilege::RequiresAdmin, true),
            ("udp-timeout", TunOptionPrivilege::Normal, false),
            (
                "iproute2-table-index",
                TunOptionPrivilege::RequiresAdmin,
                true,
            ),
            (
                "iproute2-rule-index",
                TunOptionPrivilege::RequiresAdmin,
                true,
            ),
            (
                "endpoint-independent-nat",
                TunOptionPrivilege::Normal,
                false,
            ),
            ("route-address", TunOptionPrivilege::RequiresAdmin, true),
            (
                "route-exclude-address",
                TunOptionPrivilege::RequiresAdmin,
                true,
            ),
            ("route-address-set", TunOptionPrivilege::RequiresAdmin, true),
            (
                "route-exclude-address-set",
                TunOptionPrivilege::RequiresAdmin,
                true,
            ),
            ("include-interface", TunOptionPrivilege::RequiresAdmin, true),
            ("exclude-interface", TunOptionPrivilege::RequiresAdmin, true),
        ]
        .into_iter()
        .map(|(field, privilege, high_risk)| TunOptionMetadata {
            field: field.to_string(),
            privilege,
            high_risk,
        })
        .collect()
    }

    fn is_empty(&self) -> bool {
        self.enable.is_none()
            && self.stack.as_deref().unwrap_or_default().trim().is_empty()
            && self.device.as_deref().unwrap_or_default().trim().is_empty()
            && self.dns_hijack.is_empty()
            && self.auto_detect_interface.is_none()
            && self.auto_route.is_none()
            && self.auto_redirect.is_none()
            && self.strict_route.is_none()
            && self.mtu.is_none()
            && self.gso.is_none()
            && self.gso_max_size.is_none()
            && self
                .inet6_address
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            && self.udp_timeout.is_none()
            && self.iproute2_table_index.is_none()
            && self.iproute2_rule_index.is_none()
            && self.endpoint_independent_nat.is_none()
            && self.route_address.is_empty()
            && self.route_exclude_address.is_empty()
            && self.inet4_address.is_empty()
            && self.route_address_set.is_empty()
            && self.route_exclude_address_set.is_empty()
            && self.include_interface.is_empty()
            && self.exclude_interface.is_empty()
    }
}

/// TUN 表单 view model 使用字符串保存数值项，GUI 可以显示用户输入并把解析错误映射回字段。
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TunConfigFormViewModel {
    pub enable: TunBooleanFormValue,
    pub stack: String,
    pub device: String,
    pub dns_hijack: Vec<String>,
    pub auto_detect_interface: TunBooleanFormValue,
    pub auto_route: TunBooleanFormValue,
    pub auto_redirect: TunBooleanFormValue,
    pub strict_route: TunBooleanFormValue,
    pub mtu: String,
    pub gso: TunBooleanFormValue,
    pub gso_max_size: String,
    pub inet6_address: String,
    pub udp_timeout: String,
    pub iproute2_table_index: String,
    pub iproute2_rule_index: String,
    pub endpoint_independent_nat: TunBooleanFormValue,
    pub route_address: Vec<String>,
    pub route_exclude_address: Vec<String>,
    pub inet4_address: Vec<String>,
    pub route_address_set: Vec<String>,
    pub route_exclude_address_set: Vec<String>,
    pub include_interface: Vec<String>,
    pub exclude_interface: Vec<String>,
    pub option_metadata: Vec<TunOptionMetadata>,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

impl TunConfigFormViewModel {
    pub fn for_platform(settings: &TunConfigSettings, platform: PlatformKind) -> Self {
        Self {
            enable: TunBooleanFormValue::from_option(settings.enable),
            stack: settings.stack.clone().unwrap_or_default(),
            device: settings.device.clone().unwrap_or_default(),
            dns_hijack: settings.dns_hijack.clone(),
            auto_detect_interface: TunBooleanFormValue::from_option(settings.auto_detect_interface),
            auto_route: TunBooleanFormValue::from_option(settings.auto_route),
            auto_redirect: TunBooleanFormValue::from_option(settings.auto_redirect),
            strict_route: TunBooleanFormValue::from_option(settings.strict_route),
            mtu: optional_u32(settings.mtu),
            gso: TunBooleanFormValue::from_option(settings.gso),
            gso_max_size: optional_u32(settings.gso_max_size),
            inet6_address: settings.inet6_address.clone().unwrap_or_default(),
            udp_timeout: optional_u64(settings.udp_timeout),
            iproute2_table_index: optional_u32(settings.iproute2_table_index),
            iproute2_rule_index: optional_u32(settings.iproute2_rule_index),
            endpoint_independent_nat: TunBooleanFormValue::from_option(
                settings.endpoint_independent_nat,
            ),
            route_address: settings.route_address.clone(),
            route_exclude_address: settings.route_exclude_address.clone(),
            inet4_address: settings.inet4_address.clone(),
            route_address_set: settings.route_address_set.clone(),
            route_exclude_address_set: settings.route_exclude_address_set.clone(),
            include_interface: settings.include_interface.clone(),
            exclude_interface: settings.exclude_interface.clone(),
            option_metadata: TunConfigSettings::option_metadata(),
            diagnostics: settings.validate_for_platform(platform),
        }
    }
}

impl From<&TunConfigSettings> for TunConfigFormViewModel {
    fn from(settings: &TunConfigSettings) -> Self {
        Self::for_platform(settings, current_platform_kind())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct TunBooleanFormValue {
    pub value: bool,
    pub configured: bool,
}

impl TunBooleanFormValue {
    fn from_option(value: Option<bool>) -> Self {
        Self {
            value: value.unwrap_or_default(),
            configured: value.is_some(),
        }
    }
}

struct TunConfigValidator<'a> {
    settings: &'a TunConfigSettings,
    platform: PlatformKind,
    diagnostics: Vec<ConfigDiagnostic>,
}

impl<'a> TunConfigValidator<'a> {
    fn new(settings: &'a TunConfigSettings, platform: PlatformKind) -> Self {
        Self {
            settings,
            platform,
            diagnostics: Vec::new(),
        }
    }

    fn validate(&mut self) {
        self.validate_stack();
        self.validate_dns_hijack();
        self.validate_addresses();
        self.validate_numeric_values();
        self.validate_interface_filters();
        self.validate_platform_support();
    }

    fn validate_stack(&mut self) {
        let Some(stack) = self
            .settings
            .stack
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return;
        };

        if !TUN_STACKS.contains(&stack) {
            self.diagnostics.push(ConfigDiagnostic::error(
                "tun.stack",
                format!("TUN stack `{stack}` 不是支持的取值"),
                Some(format!("可选值: {}", TUN_STACKS.join(", "))),
            ));
        }
    }

    fn validate_dns_hijack(&mut self) {
        for (index, value) in self.settings.dns_hijack.iter().enumerate() {
            let path = format!("tun.dns-hijack[{index}]");
            let value = value.trim();
            if value.is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    path,
                    "DNS 劫持地址不能为空",
                    Some(
                        "请删除空条目，或填写 `0.0.0.0:53`、`[::]:53`、`any:53` 等地址。"
                            .to_string(),
                    ),
                ));
                continue;
            }

            if let Err(message) = validate_dns_hijack_endpoint(value) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    path,
                    message,
                    Some(
                        "请使用 IP:port、[IPv6]:port 或 any:port 格式，端口范围为 1-65535。"
                            .to_string(),
                    ),
                ));
            }
        }
    }

    fn validate_addresses(&mut self) {
        self.check_cidr_list("tun.route-address", &self.settings.route_address);
        self.check_cidr_list(
            "tun.route-exclude-address",
            &self.settings.route_exclude_address,
        );
        self.check_cidr_list("tun.inet4-address", &self.settings.inet4_address);
        if let Some(value) = self
            .settings
            .inet6_address
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if let Err(message) = validate_cidr(value) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    "tun.inet6-address",
                    message,
                    Some("请填写合法的 IPv6 CIDR，例如 fdfe:dcba:9876::1/126。".to_string()),
                ));
            } else if let Ok((ip, _)) = parse_cidr(value) {
                if !ip.is_ipv6() {
                    self.diagnostics.push(ConfigDiagnostic::error(
                        "tun.inet6-address",
                        format!("`{value}` 不是 IPv6 CIDR"),
                        Some("请填写 IPv6 CIDR。".to_string()),
                    ));
                }
            }
        }
        self.check_ip_family("tun.inet4-address", &self.settings.inet4_address);
    }

    fn validate_numeric_values(&mut self) {
        if let Some(mtu) = self.settings.mtu {
            if !(576..=65_535).contains(&mtu) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    "tun.mtu",
                    format!("MTU `{mtu}` 不在有效范围 576-65535 内"),
                    Some("请使用常见值 1500，或根据网络环境填写 576 到 65535。".to_string()),
                ));
            }
        }

        if let Some(gso_max_size) = self.settings.gso_max_size {
            if !(1..=65_535).contains(&gso_max_size) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    "tun.gso-max-size",
                    format!("GSO 最大包大小 `{gso_max_size}` 不在有效范围 1-65535 内"),
                    Some("请填写 1 到 65535 之间的整数。".to_string()),
                ));
            }
        }

        if let Some(udp_timeout) = self.settings.udp_timeout {
            if udp_timeout == 0 {
                self.diagnostics.push(ConfigDiagnostic::warning(
                    "tun.udp-timeout",
                    "UDP NAT 过期时间为 0，运行时可能导致 UDP 连接立即过期",
                    Some("通常建议使用 300 秒，或根据网络环境填写更大的非零值。".to_string()),
                ));
            }
        }
    }

    fn validate_interface_filters(&mut self) {
        if !self.settings.include_interface.is_empty()
            && !self.settings.exclude_interface.is_empty()
        {
            self.diagnostics.push(ConfigDiagnostic::warning(
                "tun.include-interface",
                "include-interface 与 exclude-interface 同时配置，路由范围可能难以判断",
                Some("建议只保留一种接口过滤方式。".to_string()),
            ));
        }
    }

    fn validate_platform_support(&mut self) {
        if !self.tun_enabled_or_high_risk_configured() {
            return;
        }

        self.diagnostics.push(ConfigDiagnostic::info(
            "tun.enable",
            "TUN 会接管系统流量入口，通常需要管理员权限或系统网络扩展权限",
            Some("启动前请通过平台能力检查确认权限；保存配置本身不会提升权限。".to_string()),
        ));

        if self.platform != PlatformKind::Linux && self.settings.auto_redirect == Some(true) {
            self.diagnostics.push(ConfigDiagnostic::warning(
                "tun.auto-redirect",
                "auto-redirect 仅在 Linux 平台受支持，当前平台保存后可能不会生效",
                Some("非 Linux 平台请关闭 auto-redirect，或仅保留为跨设备同步配置。".to_string()),
            ));
        }

        let linux_only_configured = self.settings.gso == Some(true)
            || self.settings.auto_redirect == Some(true)
            || self.settings.iproute2_table_index.is_some()
            || self.settings.iproute2_rule_index.is_some()
            || !self.settings.route_address_set.is_empty()
            || !self.settings.route_exclude_address_set.is_empty();
        if self.platform != PlatformKind::Linux && linux_only_configured {
            self.diagnostics.push(ConfigDiagnostic::warning(
                "tun",
                "当前配置包含 Linux 专用 TUN 选项，平台限制不会阻止保存，但运行时可能被 mihomo 忽略",
                Some("请在目标平台启动前查看运行诊断，确认内核、nftables 或权限能力。".to_string()),
            ));
        }

        if matches!(self.platform, PlatformKind::Unknown) {
            self.diagnostics.push(ConfigDiagnostic::warning(
                "tun",
                "无法识别当前平台的 TUN 支持矩阵",
                Some("保存前只做格式校验，运行前仍需平台能力检查。".to_string()),
            ));
        }

        if self.settings.auto_redirect == Some(true) && self.settings.auto_route != Some(true) {
            self.diagnostics.push(ConfigDiagnostic::warning(
                "tun.auto-redirect",
                "auto-redirect 通常需要同时启用 auto-route 才能正确接管 TCP 流量",
                Some("请启用 auto-route，或关闭 auto-redirect。".to_string()),
            ));
        }

        if (!self.settings.route_address_set.is_empty()
            || !self.settings.route_exclude_address_set.is_empty())
            && (self.settings.auto_route != Some(true) || self.settings.auto_redirect != Some(true))
        {
            self.diagnostics.push(ConfigDiagnostic::warning(
                "tun.route-address-set",
                "route-address-set 和 route-exclude-address-set 依赖 auto-route、auto-redirect 与 Linux nftables",
                Some("目标平台为 Linux 且启用 auto-route/auto-redirect 后，这些规则集才会参与防火墙路由。".to_string()),
            ));
        }
    }

    fn check_cidr_list(&mut self, path: &str, values: &[String]) {
        for (index, value) in values.iter().enumerate() {
            let value = value.trim();
            if value.is_empty() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}[{index}]"),
                    "CIDR 地址不能为空",
                    Some("请删除空条目，或填写 IPv4/IPv6 CIDR，例如 0.0.0.0/1。".to_string()),
                ));
                continue;
            }

            if let Err(message) = validate_cidr(value) {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}[{index}]"),
                    message,
                    Some(
                        "请填写合法的 IPv4/IPv6 CIDR，例如 198.19.0.1/30 或 fdfe:dcba::1/126。"
                            .to_string(),
                    ),
                ));
            }
        }
    }

    fn check_ip_family(&mut self, path: &str, values: &[String]) {
        for (index, value) in values.iter().enumerate() {
            let Ok((ip, _)) = parse_cidr(value.trim()) else {
                continue;
            };
            if !ip.is_ipv4() {
                self.diagnostics.push(ConfigDiagnostic::error(
                    format!("{path}[{index}]"),
                    format!("`{}` 与字段要求的 IP 版本不匹配", value.trim()),
                    Some("请填写 IPv4 CIDR。".to_string()),
                ));
            }
        }
    }

    fn tun_enabled_or_high_risk_configured(&self) -> bool {
        self.settings.enable == Some(true)
            || self.settings.auto_route == Some(true)
            || self.settings.auto_redirect == Some(true)
            || self.settings.strict_route == Some(true)
            || !self.settings.dns_hijack.is_empty()
            || !self.settings.route_address.is_empty()
            || !self.settings.route_exclude_address.is_empty()
            || !self.settings.inet4_address.is_empty()
            || self
                .settings
                .inet6_address
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty())
    }
}

fn validate_dns_hijack_endpoint(value: &str) -> Result<(), String> {
    if let Ok(socket) = value.parse::<SocketAddr>() {
        return validate_port(socket.port() as u32, value);
    }

    let (host, port) = if let Some(rest) = value.strip_prefix('[') {
        let (host, rest) = rest
            .split_once(']')
            .ok_or_else(|| format!("DNS 劫持地址 `{value}` 缺少 IPv6 右括号"))?;
        let port = rest
            .strip_prefix(':')
            .ok_or_else(|| format!("DNS 劫持地址 `{value}` 缺少端口分隔符"))?;
        (host, port)
    } else {
        let (host, port) = value
            .rsplit_once(':')
            .ok_or_else(|| format!("DNS 劫持地址 `{value}` 缺少端口"))?;
        if host.contains(':') {
            return Err(format!(
                "IPv6 DNS 劫持地址 `{value}` 需要使用 [addr]:port 格式"
            ));
        }
        (host, port)
    };

    let port = port
        .parse::<u32>()
        .map_err(|_| format!("DNS 劫持地址 `{value}` 的端口不是整数"))?;
    validate_port(port, value)?;

    if host.eq_ignore_ascii_case("any")
        || host == "*"
        || host.parse::<IpAddr>().is_ok()
        || is_dns_hijack_domain(host)
    {
        Ok(())
    } else {
        Err(format!("DNS 劫持地址 `{value}` 的主机部分格式无效"))
    }
}

fn validate_port(port: u32, value: &str) -> Result<(), String> {
    if (1..=65_535).contains(&port) {
        Ok(())
    } else {
        Err(format!("DNS 劫持地址 `{value}` 的端口不在 1-65535 内"))
    }
}

fn validate_cidr(value: &str) -> Result<(), String> {
    parse_cidr(value).map(|_| ())
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

fn is_dns_hijack_domain(host: &str) -> bool {
    !host.is_empty()
        && host
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '_' | '+' | '*'))
}

fn optional_u32(value: Option<u32>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn optional_u64(value: Option<u64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalized_strings(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub fn has_tun_error(diagnostics: &[ConfigDiagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == ConfigDiagnosticSeverity::Error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use air_config::ConfigDocument;

    fn docs_document() -> ConfigDocument {
        ConfigDocument::parse(include_str!("../../../docs/config.yaml"))
            .expect("docs/config.yaml should parse")
    }

    fn has_diagnostic_at(
        diagnostics: &[ConfigDiagnostic],
        severity: ConfigDiagnosticSeverity,
        path: &str,
    ) -> bool {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == severity && diagnostic.path == path)
    }

    #[test]
    fn extracts_tun_fields_from_docs_config() {
        let document = docs_document();
        let settings = TunConfigSettings::from_document(&document.typed);

        assert_eq!(settings.enable, Some(false));
        assert_eq!(settings.stack.as_deref(), Some("system"));
        assert_eq!(settings.dns_hijack, vec!["0.0.0.0:53"]);
        assert_eq!(settings.auto_redirect, Some(false));
        assert_eq!(
            settings.route_address,
            vec!["0.0.0.0/1", "128.0.0.0/1", "::/1", "8000::/1"]
        );
        assert_eq!(settings.route_address_set, vec!["ruleset-1", "ruleset-2"]);
        assert_eq!(
            settings.route_exclude_address_set,
            vec!["ruleset-3", "ruleset-4"]
        );
    }

    #[test]
    fn writes_back_tun_without_touching_other_sections_or_extensions() {
        let mut document = docs_document().typed;
        let original_dns = document.dns.clone();
        let original_proxies = document.proxies.clone();
        let original_extensions = document.tun.as_ref().unwrap().extensions.clone();

        let mut settings = TunConfigSettings::from_document(&document);
        settings.enable = Some(true);
        settings.stack = Some("mixed".into());
        settings.route_address.push("198.18.0.0/16".into());
        settings.apply_to_document(&mut document);

        let tun = document.tun.as_ref().expect("tun should remain present");
        assert_eq!(tun.enable, Some(true));
        assert_eq!(tun.stack.as_deref(), Some("mixed"));
        assert!(tun.route_address.contains(&"198.18.0.0/16".to_string()));
        assert_eq!(tun.extensions, original_extensions);
        assert_eq!(document.dns, original_dns);
        assert_eq!(document.proxies, original_proxies);
    }

    #[test]
    fn validates_tun_format_errors() {
        let settings = TunConfigSettings {
            stack: Some("bad-stack".into()),
            dns_hijack: vec!["0.0.0.0:70000".into()],
            route_address: vec!["0.0.0.0/33".into()],
            inet4_address: vec!["fdfe:dcba::1/126".into()],
            mtu: Some(10),
            ..Default::default()
        };

        let diagnostics = settings.validate_for_platform(PlatformKind::Linux);

        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "tun.stack"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "tun.dns-hijack[0]"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "tun.route-address[0]"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "tun.inet4-address[0]"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Error,
            "tun.mtu"
        ));
    }

    #[test]
    fn emits_platform_warnings_without_errors() {
        let settings = TunConfigSettings {
            enable: Some(true),
            auto_redirect: Some(true),
            route_address_set: vec!["ruleset".into()],
            ..Default::default()
        };

        let diagnostics = settings.validate_for_platform(PlatformKind::Windows);

        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Warning,
            "tun.auto-redirect"
        ));
        assert!(has_diagnostic_at(
            &diagnostics,
            ConfigDiagnosticSeverity::Warning,
            "tun"
        ));
        assert!(!has_tun_error(&diagnostics));
    }

    #[test]
    fn prepares_form_view_model_with_metadata_and_diagnostics() {
        let settings = TunConfigSettings {
            enable: Some(true),
            mtu: Some(1500),
            auto_route: Some(true),
            auto_redirect: Some(true),
            ..Default::default()
        };

        let view_model = TunConfigFormViewModel::for_platform(&settings, PlatformKind::Macos);

        assert_eq!(view_model.enable.value, true);
        assert_eq!(view_model.enable.configured, true);
        assert_eq!(view_model.mtu, "1500");
        assert!(view_model.option_metadata.iter().any(|metadata| {
            metadata.field == "auto-route"
                && metadata.privilege == TunOptionPrivilege::RequiresAdmin
                && metadata.high_risk
        }));
        assert!(has_diagnostic_at(
            &view_model.diagnostics,
            ConfigDiagnosticSeverity::Warning,
            "tun.auto-redirect"
        ));
    }
}
