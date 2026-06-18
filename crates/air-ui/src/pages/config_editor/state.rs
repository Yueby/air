use std::collections::BTreeSet;

use super::form_helpers::*;
use super::render::{ConfigBoolField, ConfigTextField};
use air_app::AppCommand;
#[cfg(test)]
use air_config::ConfigDocument;
use air_config::MihomoConfigDocument;
use air_config::model::FallbackFilterConfig;
use air_config::model::{ExternalControllerCorsConfig, GeoxUrlConfig};
use air_config::{
    DnsConfigSettings, DnsNameserverPolicySettings, SnifferConfigSettings, SnifferProtocolSettings,
    TunConfigSettings,
};
use air_mihomo::global_config::{AuthenticationCredential, GlobalConfigSettings, SecretValue};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ConfigEditorGroup {
    Global,
    Tun,
    Sniffer,
    Dns,
}

impl ConfigEditorGroup {
    const ALL: [Self; 4] = [Self::Global, Self::Tun, Self::Sniffer, Self::Dns];

    pub(crate) fn all() -> &'static [Self] {
        &Self::ALL
    }

    pub(crate) fn tab_index(self) -> usize {
        Self::ALL
            .iter()
            .position(|group| *group == self)
            .unwrap_or(0)
    }

    pub(crate) fn from_tab_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::Global)
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Global => "全局",
            Self::Tun => "TUN",
            Self::Sniffer => "嗅探",
            Self::Dns => "DNS",
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            Self::Global => "全局配置",
            Self::Tun => "TUN 入站",
            Self::Sniffer => "域名嗅探",
            Self::Dns => "DNS 解析",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConfigEditorPageState {
    pub(crate) document: MihomoConfigDocument,
    pub(crate) draft: ConfigEditorDraft,
    active_group: ConfigEditorGroup,
    dirty_groups: BTreeSet<ConfigEditorGroup>,
    advanced_open: BTreeSet<ConfigEditorGroup>,
    pub(crate) notice: Option<ConfigEditorNotice>,
}

impl ConfigEditorPageState {
    pub fn empty() -> Self {
        // 非配置页激活期间使用空文档占位，避免把 docs/config.yaml 测试夹具作为生产常驻状态。
        Self::from_document(MihomoConfigDocument::default())
    }

    #[cfg(test)]
    pub fn fake_for_test() -> Self {
        let document = ConfigDocument::parse(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/config.yaml"
        )))
        .expect("docs/config.yaml should remain a valid mihomo fixture")
        .typed;
        Self::from_document(document)
    }

    pub fn from_document(document: MihomoConfigDocument) -> Self {
        let draft = ConfigEditorDraft::from_document(&document);

        Self {
            document,
            draft,
            active_group: ConfigEditorGroup::Global,
            dirty_groups: BTreeSet::new(),
            advanced_open: BTreeSet::new(),
            notice: None,
        }
    }

    pub fn set_group(&mut self, group: ConfigEditorGroup) {
        self.active_group = group;
    }

    pub fn toggle_advanced(&mut self, group: ConfigEditorGroup) {
        if !self.advanced_open.remove(&group) {
            self.advanced_open.insert(group);
        }
    }

    pub fn update_text(
        &mut self,
        field: ConfigTextField,
        value: impl Into<String>,
    ) -> Option<AppCommand> {
        self.draft.update_text(field, value.into());
        self.dirty_groups.insert(field.group());
        self.notice = None;
        None
    }

    pub fn apply_persisted_runtime_mode(&mut self, mode: &str) {
        let had_global_dirty = self.dirty_groups.contains(&ConfigEditorGroup::Global);
        self.document.global.mode = Some(mode.to_string());
        self.draft.global.mode = mode.to_string();
        self.notice = None;

        if had_global_dirty {
            // 运行模式已经由状态栏保存；若切换前全局页还有其他草稿差异，继续保留未保存提示。
            self.dirty_groups.insert(ConfigEditorGroup::Global);
        } else {
            self.dirty_groups.remove(&ConfigEditorGroup::Global);
        }
    }

    pub fn apply_persisted_tun_enable(&mut self, enabled: bool) {
        let had_tun_dirty = self.dirty_groups.contains(&ConfigEditorGroup::Tun);
        self.document
            .tun
            .get_or_insert_with(Default::default)
            .enable = Some(enabled);
        self.draft.tun.enable = Some(enabled);
        self.notice = None;

        if had_tun_dirty {
            // 状态栏菜单已经把 enable 持久化；若 TUN 页还有其他草稿差异，继续保留未保存提示。
            self.dirty_groups.insert(ConfigEditorGroup::Tun);
        } else {
            self.dirty_groups.remove(&ConfigEditorGroup::Tun);
        }
    }

    pub fn cycle_bool(&mut self, field: ConfigBoolField) -> Option<AppCommand> {
        self.draft.cycle_bool(field);
        self.dirty_groups.insert(field.group());
        self.notice = None;
        None
    }

    pub fn set_bool(&mut self, field: ConfigBoolField, value: bool) -> Option<AppCommand> {
        self.draft.set_bool(field, value);
        self.dirty_groups.insert(field.group());
        self.notice = None;
        None
    }

    pub fn save_group(&mut self, group: ConfigEditorGroup) -> Option<AppCommand> {
        let document = self.preview_document();

        if document == self.document {
            self.notice = None;
            self.dirty_groups.remove(&group);
            return None;
        }

        self.document = document;
        self.draft = ConfigEditorDraft::from_document(&self.document);
        self.dirty_groups.clear();
        // 这里只标记“保存命令已提交”的本地状态；真正落盘和运行态重载成功后，
        // app router 会发出全局通知，避免设置页保存时出现两条成功 toast。
        self.notice = Some(ConfigEditorNotice::success("配置保存请求已提交"));
        match serde_yaml::to_string(&self.document) {
            Ok(profile) => Some(AppCommand::SaveConfig { profile }),
            Err(error) => {
                self.notice = Some(ConfigEditorNotice::error(format!(
                    "当前配置无法序列化为 YAML：{error}"
                )));
                None
            }
        }
    }

    pub fn view_model(&self) -> ConfigEditorViewModel {
        ConfigEditorViewModel {
            active_group: self.active_group,
            dirty_groups: self.dirty_groups.clone(),
            advanced_open: self.advanced_open.clone(),
            draft: self.draft.clone(),
            notice: self.notice.clone(),
        }
    }

    fn preview_document(&self) -> MihomoConfigDocument {
        let settings = self.draft.to_settings();
        let mut document = self.document.clone();
        settings.global.apply_to_document(&mut document);
        settings.tun.apply_to_document(&mut document);
        settings.sniffer.apply_to_document(&mut document);
        settings.dns.apply_to_document(&mut document);
        document
    }
}

#[derive(Clone, Debug)]
pub struct ConfigEditorViewModel {
    pub active_group: ConfigEditorGroup,
    pub dirty_groups: BTreeSet<ConfigEditorGroup>,
    pub advanced_open: BTreeSet<ConfigEditorGroup>,
    pub draft: ConfigEditorDraft,
    pub notice: Option<ConfigEditorNotice>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigEditorNotice {
    pub level: ConfigNoticeLevel,
    pub message: String,
}

impl ConfigEditorNotice {
    fn success(message: impl Into<String>) -> Self {
        Self {
            level: ConfigNoticeLevel::Success,
            message: message.into(),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            level: ConfigNoticeLevel::Error,
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigNoticeLevel {
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug)]
struct DraftSettings {
    global: GlobalConfigSettings,
    tun: TunConfigSettings,
    sniffer: SnifferConfigSettings,
    dns: DnsConfigSettings,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigEditorDraft {
    pub global: GlobalFormState,
    pub tun: TunFormState,
    pub sniffer: SnifferFormState,
    pub dns: DnsFormState,
}

impl ConfigEditorDraft {
    fn from_document(document: &MihomoConfigDocument) -> Self {
        Self {
            global: GlobalFormState::from_settings(GlobalConfigSettings::from_document(document)),
            tun: TunFormState::from_settings(TunConfigSettings::from_document(document)),
            sniffer: SnifferFormState::from_settings(SnifferConfigSettings::from_document(
                document,
            )),
            dns: DnsFormState::from_settings(DnsConfigSettings::from_document(document)),
        }
    }

    fn update_text(&mut self, field: ConfigTextField, value: String) {
        match field {
            ConfigTextField::GlobalMixedPort => self.global.mixed_port = value,
            ConfigTextField::GlobalHttpPort => self.global.port = value,
            ConfigTextField::GlobalSocksPort => self.global.socks_port = value,
            ConfigTextField::GlobalRedirPort => self.global.redir_port = value,
            ConfigTextField::GlobalTproxyPort => self.global.tproxy_port = value,
            ConfigTextField::GlobalBindAddress => self.global.bind_address = value,
            ConfigTextField::GlobalLanAllowedIps => self.global.lan_allowed_ips = value,
            ConfigTextField::GlobalLanDisallowedIps => self.global.lan_disallowed_ips = value,
            ConfigTextField::GlobalMode => self.global.mode = value,
            ConfigTextField::GlobalLogLevel => self.global.log_level = value,
            ConfigTextField::GlobalKeepAliveInterval => self.global.keep_alive_interval = value,
            ConfigTextField::GlobalKeepAliveIdle => self.global.keep_alive_idle = value,
            ConfigTextField::GlobalFindProcessMode => self.global.find_process_mode = value,
            ConfigTextField::GlobalController => self.global.external_controller = value,
            ConfigTextField::GlobalControllerCorsAllowOrigins => {
                self.global.external_controller_cors_allow_origins = value
            }
            ConfigTextField::GlobalDohServer => self.global.external_doh_server = value,
            ConfigTextField::GlobalSecret => self.global.secret = value,
            ConfigTextField::GlobalAuthentication => self.global.authentication = value,
            ConfigTextField::GlobalSkipAuthPrefixes => self.global.skip_auth_prefixes = value,
            ConfigTextField::GlobalInterfaceName => self.global.interface_name = value,
            ConfigTextField::GlobalRoutingMark => self.global.routing_mark = value,
            ConfigTextField::GlobalGeodataLoader => self.global.geodata_loader = value,
            ConfigTextField::GlobalGeoUpdateInterval => self.global.geo_update_interval = value,
            ConfigTextField::GlobalGeoxGeoip => self.global.geox_geoip = value,
            ConfigTextField::GlobalGeoxGeosite => self.global.geox_geosite = value,
            ConfigTextField::GlobalGeoxMmdb => self.global.geox_mmdb = value,
            ConfigTextField::GlobalGeoxAsn => self.global.geox_asn = value,
            ConfigTextField::GlobalUa => self.global.global_ua = value,
            ConfigTextField::TunStack => self.tun.stack = value,
            ConfigTextField::TunDevice => self.tun.device = value,
            ConfigTextField::TunDnsHijack => self.tun.dns_hijack = value,
            ConfigTextField::TunMtu => self.tun.mtu = value,
            ConfigTextField::TunGsoMaxSize => self.tun.gso_max_size = value,
            ConfigTextField::TunInet6Address => self.tun.inet6_address = value,
            ConfigTextField::TunUdpTimeout => self.tun.udp_timeout = value,
            ConfigTextField::TunIproute2TableIndex => self.tun.iproute2_table_index = value,
            ConfigTextField::TunIproute2RuleIndex => self.tun.iproute2_rule_index = value,
            ConfigTextField::TunRouteAddressSet => self.tun.route_address_set = value,
            ConfigTextField::TunRouteExcludeAddressSet => {
                self.tun.route_exclude_address_set = value
            }
            ConfigTextField::TunRouteAddress => self.tun.route_address = value,
            ConfigTextField::TunRouteExclude => self.tun.route_exclude_address = value,
            ConfigTextField::TunIncludeInterface => self.tun.include_interface = value,
            ConfigTextField::TunExcludeInterface => self.tun.exclude_interface = value,
            ConfigTextField::SnifferProtocols => self.sniffer.protocols = value,
            ConfigTextField::SnifferForceDomain => self.sniffer.force_domain = value,
            ConfigTextField::SnifferSkipDomain => self.sniffer.skip_domain = value,
            ConfigTextField::SnifferSkipSrcAddress => self.sniffer.skip_src_address = value,
            ConfigTextField::SnifferSkipDstAddress => self.sniffer.skip_dst_address = value,
            ConfigTextField::DnsCacheAlgorithm => self.dns.cache_algorithm = value,
            ConfigTextField::DnsListen => self.dns.listen = value,
            ConfigTextField::DnsFakeIpRange6 => self.dns.fake_ip_range6 = value,
            ConfigTextField::DnsFakeIpTtl => self.dns.fake_ip_ttl = value,
            ConfigTextField::DnsEnhancedMode => self.dns.enhanced_mode = value,
            ConfigTextField::DnsFakeIpRange => self.dns.fake_ip_range = value,
            ConfigTextField::DnsFakeIpFilterMode => self.dns.fake_ip_filter_mode = value,
            ConfigTextField::DnsFakeIpFilter => self.dns.fake_ip_filter = value,
            ConfigTextField::DnsDefaultNameserver => self.dns.default_nameserver = value,
            ConfigTextField::DnsNameserver => self.dns.nameserver = value,
            ConfigTextField::DnsFallback => self.dns.fallback = value,
            ConfigTextField::DnsProxyServerNameserver => self.dns.proxy_server_nameserver = value,
            ConfigTextField::DnsProxyServerNameserverPolicy => {
                self.dns.proxy_server_nameserver_policy = value
            }
            ConfigTextField::DnsDirectNameserver => self.dns.direct_nameserver = value,
            ConfigTextField::DnsNameserverPolicy => self.dns.nameserver_policy = value,
            ConfigTextField::DnsFallbackGeoipCode => self.dns.fallback_geoip_code = value,
            ConfigTextField::DnsFallbackGeosite => self.dns.fallback_geosite = value,
            ConfigTextField::DnsFallbackIpcidr => self.dns.fallback_ipcidr = value,
            ConfigTextField::DnsFallbackDomain => self.dns.fallback_domain = value,
        }
    }

    fn cycle_bool(&mut self, field: ConfigBoolField) {
        match field {
            ConfigBoolField::GlobalAllowLan => cycle_bool(&mut self.global.allow_lan),
            ConfigBoolField::GlobalIpv6 => cycle_bool(&mut self.global.ipv6),
            ConfigBoolField::GlobalDisableKeepAlive => {
                cycle_bool(&mut self.global.disable_keep_alive)
            }
            ConfigBoolField::GlobalControllerCorsAllowPrivateNetwork => {
                cycle_bool(&mut self.global.external_controller_cors_allow_private_network)
            }
            ConfigBoolField::GlobalStoreSelected => cycle_bool(&mut self.global.store_selected),
            ConfigBoolField::GlobalStoreFakeIp => cycle_bool(&mut self.global.store_fake_ip),
            ConfigBoolField::GlobalUnifiedDelay => cycle_bool(&mut self.global.unified_delay),
            ConfigBoolField::GlobalTcpConcurrent => cycle_bool(&mut self.global.tcp_concurrent),
            ConfigBoolField::GlobalGeodataMode => cycle_bool(&mut self.global.geodata_mode),
            ConfigBoolField::GlobalGeoAutoUpdate => cycle_bool(&mut self.global.geo_auto_update),
            ConfigBoolField::TunEnable => cycle_bool(&mut self.tun.enable),
            ConfigBoolField::TunAutoDetectInterface => {
                cycle_bool(&mut self.tun.auto_detect_interface)
            }
            ConfigBoolField::TunAutoRoute => cycle_bool(&mut self.tun.auto_route),
            ConfigBoolField::TunAutoRedirect => cycle_bool(&mut self.tun.auto_redirect),
            ConfigBoolField::TunStrictRoute => cycle_bool(&mut self.tun.strict_route),
            ConfigBoolField::TunGso => cycle_bool(&mut self.tun.gso),
            ConfigBoolField::TunEndpointIndependentNat => {
                cycle_bool(&mut self.tun.endpoint_independent_nat)
            }
            ConfigBoolField::SnifferEnable => cycle_bool(&mut self.sniffer.enable),
            ConfigBoolField::SnifferForceDnsMapping => {
                cycle_bool(&mut self.sniffer.force_dns_mapping)
            }
            ConfigBoolField::SnifferParsePureIp => cycle_bool(&mut self.sniffer.parse_pure_ip),
            ConfigBoolField::SnifferOverrideDestination => {
                cycle_bool(&mut self.sniffer.override_destination)
            }
            ConfigBoolField::DnsEnable => cycle_bool(&mut self.dns.enable),
            ConfigBoolField::DnsPreferH3 => cycle_bool(&mut self.dns.prefer_h3),
            ConfigBoolField::DnsUseHosts => cycle_bool(&mut self.dns.use_hosts),
            ConfigBoolField::DnsUseSystemHosts => cycle_bool(&mut self.dns.use_system_hosts),
            ConfigBoolField::DnsRespectRules => cycle_bool(&mut self.dns.respect_rules),
            ConfigBoolField::DnsIpv6 => cycle_bool(&mut self.dns.ipv6),
            ConfigBoolField::DnsDirectFollowPolicy => {
                cycle_bool(&mut self.dns.direct_nameserver_follow_policy)
            }
            ConfigBoolField::DnsFallbackGeoip => cycle_bool(&mut self.dns.fallback_geoip),
        }
    }

    pub(crate) fn set_bool(&mut self, field: ConfigBoolField, value: bool) {
        match field {
            ConfigBoolField::GlobalAllowLan => self.global.allow_lan = Some(value),
            ConfigBoolField::GlobalIpv6 => self.global.ipv6 = Some(value),
            ConfigBoolField::GlobalDisableKeepAlive => self.global.disable_keep_alive = Some(value),
            ConfigBoolField::GlobalControllerCorsAllowPrivateNetwork => {
                self.global.external_controller_cors_allow_private_network = Some(value)
            }
            ConfigBoolField::GlobalStoreSelected => self.global.store_selected = Some(value),
            ConfigBoolField::GlobalStoreFakeIp => self.global.store_fake_ip = Some(value),
            ConfigBoolField::GlobalUnifiedDelay => self.global.unified_delay = Some(value),
            ConfigBoolField::GlobalTcpConcurrent => self.global.tcp_concurrent = Some(value),
            ConfigBoolField::GlobalGeodataMode => self.global.geodata_mode = Some(value),
            ConfigBoolField::GlobalGeoAutoUpdate => self.global.geo_auto_update = Some(value),
            ConfigBoolField::TunEnable => self.tun.enable = Some(value),
            ConfigBoolField::TunAutoDetectInterface => self.tun.auto_detect_interface = Some(value),
            ConfigBoolField::TunAutoRoute => self.tun.auto_route = Some(value),
            ConfigBoolField::TunAutoRedirect => self.tun.auto_redirect = Some(value),
            ConfigBoolField::TunStrictRoute => self.tun.strict_route = Some(value),
            ConfigBoolField::TunGso => self.tun.gso = Some(value),
            ConfigBoolField::TunEndpointIndependentNat => {
                self.tun.endpoint_independent_nat = Some(value)
            }
            ConfigBoolField::SnifferEnable => self.sniffer.enable = Some(value),
            ConfigBoolField::SnifferForceDnsMapping => self.sniffer.force_dns_mapping = Some(value),
            ConfigBoolField::SnifferParsePureIp => self.sniffer.parse_pure_ip = Some(value),
            ConfigBoolField::SnifferOverrideDestination => {
                self.sniffer.override_destination = Some(value)
            }
            ConfigBoolField::DnsEnable => self.dns.enable = Some(value),
            ConfigBoolField::DnsPreferH3 => self.dns.prefer_h3 = Some(value),
            ConfigBoolField::DnsUseHosts => self.dns.use_hosts = Some(value),
            ConfigBoolField::DnsUseSystemHosts => self.dns.use_system_hosts = Some(value),
            ConfigBoolField::DnsRespectRules => self.dns.respect_rules = Some(value),
            ConfigBoolField::DnsIpv6 => self.dns.ipv6 = Some(value),
            ConfigBoolField::DnsDirectFollowPolicy => {
                self.dns.direct_nameserver_follow_policy = Some(value)
            }
            ConfigBoolField::DnsFallbackGeoip => self.dns.fallback_geoip = Some(value),
        }
    }

    fn to_settings(&self) -> DraftSettings {
        DraftSettings {
            global: self.global.to_settings(),
            tun: self.tun.to_settings(),
            sniffer: self.sniffer.to_settings(),
            dns: self.dns.to_settings(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlobalFormState {
    pub port: String,
    pub socks_port: String,
    pub mixed_port: String,
    pub redir_port: String,
    pub tproxy_port: String,
    pub allow_lan: Option<bool>,
    pub bind_address: String,
    pub lan_allowed_ips: String,
    pub lan_disallowed_ips: String,
    pub authentication: String,
    pub skip_auth_prefixes: String,
    pub mode: String,
    pub log_level: String,
    pub ipv6: Option<bool>,
    pub keep_alive_interval: String,
    pub keep_alive_idle: String,
    pub disable_keep_alive: Option<bool>,
    pub find_process_mode: String,
    pub external_controller: String,
    pub external_controller_cors_allow_origins: String,
    pub external_controller_cors_allow_private_network: Option<bool>,
    original_external_controller_cors: Option<ExternalControllerCorsConfig>,
    pub external_doh_server: String,
    pub secret: String,
    preserved_secret: Option<SecretValue>,
    pub store_selected: Option<bool>,
    pub store_fake_ip: Option<bool>,
    pub unified_delay: Option<bool>,
    pub tcp_concurrent: Option<bool>,
    pub interface_name: String,
    pub routing_mark: String,
    pub geodata_mode: Option<bool>,
    pub geodata_loader: String,
    pub geo_auto_update: Option<bool>,
    pub geo_update_interval: String,
    pub geox_geoip: String,
    pub geox_geosite: String,
    pub geox_mmdb: String,
    pub geox_asn: String,
    original_geox_url: Option<GeoxUrlConfig>,
    pub global_ua: String,
}

impl GlobalFormState {
    fn from_settings(settings: GlobalConfigSettings) -> Self {
        Self {
            port: optional_u32(settings.port),
            socks_port: optional_u32(settings.socks_port),
            mixed_port: optional_u32(settings.mixed_port),
            redir_port: optional_u32(settings.redir_port),
            tproxy_port: optional_u32(settings.tproxy_port),
            allow_lan: settings.allow_lan,
            bind_address: settings.bind_address.unwrap_or_default(),
            lan_allowed_ips: settings.lan_allowed_ips.join("\n"),
            lan_disallowed_ips: settings.lan_disallowed_ips.join("\n"),
            authentication: settings
                .authentication
                .iter()
                .map(AuthenticationCredential::as_config_value)
                .collect::<Vec<_>>()
                .join("\n"),
            skip_auth_prefixes: settings.skip_auth_prefixes.join("\n"),
            mode: settings.mode.unwrap_or_default(),
            log_level: settings.log_level.unwrap_or_default(),
            ipv6: settings.ipv6,
            keep_alive_interval: optional_u64(settings.keep_alive_interval),
            keep_alive_idle: optional_u64(settings.keep_alive_idle),
            disable_keep_alive: settings.disable_keep_alive,
            find_process_mode: settings.find_process_mode.unwrap_or_default(),
            external_controller: settings.external_controller.unwrap_or_default(),
            external_controller_cors_allow_origins: settings
                .external_controller_cors
                .as_ref()
                .map(|cors| cors.allow_origins.join("\n"))
                .unwrap_or_default(),
            external_controller_cors_allow_private_network: settings
                .external_controller_cors
                .as_ref()
                .and_then(|cors| cors.allow_private_network),
            original_external_controller_cors: settings.external_controller_cors.clone(),
            external_doh_server: settings.external_doh_server.unwrap_or_default(),
            secret: String::new(),
            preserved_secret: settings.secret,
            store_selected: settings.profile.store_selected,
            store_fake_ip: settings.profile.store_fake_ip,
            unified_delay: settings.unified_delay,
            tcp_concurrent: settings.tcp_concurrent,
            interface_name: settings.interface_name.unwrap_or_default(),
            routing_mark: optional_value(settings.routing_mark.as_ref()),
            geodata_mode: settings.geodata_mode,
            geodata_loader: settings.geodata_loader.unwrap_or_default(),
            geo_auto_update: settings.geo_auto_update,
            geo_update_interval: optional_u64(settings.geo_update_interval),
            geox_geoip: settings
                .geox_url
                .as_ref()
                .and_then(|geox| geox.geoip.clone())
                .unwrap_or_default(),
            geox_geosite: settings
                .geox_url
                .as_ref()
                .and_then(|geox| geox.geosite.clone())
                .unwrap_or_default(),
            geox_mmdb: settings
                .geox_url
                .as_ref()
                .and_then(|geox| geox.mmdb.clone())
                .unwrap_or_default(),
            geox_asn: settings
                .geox_url
                .as_ref()
                .and_then(|geox| geox.asn.clone())
                .unwrap_or_default(),
            original_geox_url: settings.geox_url.clone(),
            global_ua: settings.global_ua.unwrap_or_default(),
        }
    }

    fn to_settings(&self) -> GlobalConfigSettings {
        let secret = if self.secret.trim().is_empty() {
            self.preserved_secret.clone()
        } else {
            Some(SecretValue::new(self.secret.trim().to_string()))
        };

        GlobalConfigSettings {
            port: parse_optional_u32(&self.port),
            socks_port: parse_optional_u32(&self.socks_port),
            mixed_port: parse_optional_u32(&self.mixed_port),
            redir_port: parse_optional_u32(&self.redir_port),
            tproxy_port: parse_optional_u32(&self.tproxy_port),
            allow_lan: self.allow_lan,
            bind_address: optional_text(&self.bind_address),
            lan_allowed_ips: split_lines(&self.lan_allowed_ips),
            lan_disallowed_ips: split_lines(&self.lan_disallowed_ips),
            authentication: split_lines(&self.authentication)
                .into_iter()
                .map(AuthenticationCredential::new)
                .collect(),
            skip_auth_prefixes: split_lines(&self.skip_auth_prefixes),
            mode: optional_text(&self.mode),
            log_level: optional_text(&self.log_level),
            ipv6: self.ipv6,
            keep_alive_interval: parse_optional_u64(&self.keep_alive_interval),
            keep_alive_idle: parse_optional_u64(&self.keep_alive_idle),
            disable_keep_alive: self.disable_keep_alive,
            find_process_mode: optional_text(&self.find_process_mode),
            external_controller: optional_text(&self.external_controller),
            external_controller_cors: external_controller_cors_from_form(
                &self.external_controller_cors_allow_origins,
                self.external_controller_cors_allow_private_network,
                self.original_external_controller_cors.as_ref(),
            ),
            external_doh_server: optional_text(&self.external_doh_server),
            secret,
            unified_delay: self.unified_delay,
            tcp_concurrent: self.tcp_concurrent,
            interface_name: optional_text(&self.interface_name),
            routing_mark: optional_value_from_text(&self.routing_mark),
            geodata_mode: self.geodata_mode,
            geodata_loader: optional_text(&self.geodata_loader),
            geo_auto_update: self.geo_auto_update,
            geo_update_interval: parse_optional_u64(&self.geo_update_interval),
            geox_url: geox_url_from_form(
                &self.geox_geoip,
                &self.geox_geosite,
                &self.geox_mmdb,
                &self.geox_asn,
                self.original_geox_url.as_ref(),
            ),
            global_ua: optional_text(&self.global_ua),
            profile: air_mihomo::global_config::GlobalProfileSettings {
                store_selected: self.store_selected,
                store_fake_ip: self.store_fake_ip,
            },
            ..Default::default()
        }
    }

    pub(crate) fn secret_label(&self) -> &'static str {
        if self.preserved_secret.is_some() {
            "secret，留空保留原值"
        } else {
            "secret，API Bearer Token"
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TunFormState {
    pub enable: Option<bool>,
    pub stack: String,
    pub device: String,
    pub dns_hijack: String,
    pub auto_detect_interface: Option<bool>,
    pub auto_route: Option<bool>,
    pub auto_redirect: Option<bool>,
    pub strict_route: Option<bool>,
    pub mtu: String,
    pub gso: Option<bool>,
    pub gso_max_size: String,
    pub inet6_address: String,
    pub udp_timeout: String,
    pub iproute2_table_index: String,
    pub iproute2_rule_index: String,
    pub endpoint_independent_nat: Option<bool>,
    pub route_address_set: String,
    pub route_exclude_address_set: String,
    pub route_address: String,
    pub route_exclude_address: String,
    pub include_interface: String,
    pub exclude_interface: String,
}

impl TunFormState {
    fn from_settings(settings: TunConfigSettings) -> Self {
        Self {
            enable: settings.enable,
            stack: settings.stack.unwrap_or_default(),
            device: settings.device.unwrap_or_default(),
            dns_hijack: settings.dns_hijack.join("\n"),
            auto_detect_interface: settings.auto_detect_interface,
            auto_route: settings.auto_route,
            auto_redirect: settings.auto_redirect,
            strict_route: settings.strict_route,
            mtu: optional_u32(settings.mtu),
            gso: settings.gso,
            gso_max_size: optional_u32(settings.gso_max_size),
            inet6_address: settings.inet6_address.unwrap_or_default(),
            udp_timeout: optional_u64(settings.udp_timeout),
            iproute2_table_index: optional_u32(settings.iproute2_table_index),
            iproute2_rule_index: optional_u32(settings.iproute2_rule_index),
            endpoint_independent_nat: settings.endpoint_independent_nat,
            route_address_set: settings.route_address_set.join("\n"),
            route_exclude_address_set: settings.route_exclude_address_set.join("\n"),
            route_address: settings.route_address.join("\n"),
            route_exclude_address: settings.route_exclude_address.join("\n"),
            include_interface: settings.include_interface.join("\n"),
            exclude_interface: settings.exclude_interface.join("\n"),
        }
    }

    fn to_settings(&self) -> TunConfigSettings {
        TunConfigSettings {
            enable: self.enable,
            stack: optional_text(&self.stack),
            device: optional_text(&self.device),
            dns_hijack: split_lines(&self.dns_hijack),
            auto_detect_interface: self.auto_detect_interface,
            auto_route: self.auto_route,
            auto_redirect: self.auto_redirect,
            strict_route: self.strict_route,
            mtu: parse_optional_u32(&self.mtu),
            gso: self.gso,
            gso_max_size: parse_optional_u32(&self.gso_max_size),
            inet6_address: optional_text(&self.inet6_address),
            udp_timeout: parse_optional_u64(&self.udp_timeout),
            iproute2_table_index: parse_optional_u32(&self.iproute2_table_index),
            iproute2_rule_index: parse_optional_u32(&self.iproute2_rule_index),
            endpoint_independent_nat: self.endpoint_independent_nat,
            route_address_set: split_lines(&self.route_address_set),
            route_exclude_address_set: split_lines(&self.route_exclude_address_set),
            route_address: split_lines(&self.route_address),
            route_exclude_address: split_lines(&self.route_exclude_address),
            include_interface: split_lines(&self.include_interface),
            exclude_interface: split_lines(&self.exclude_interface),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SnifferFormState {
    pub enable: Option<bool>,
    pub force_dns_mapping: Option<bool>,
    pub parse_pure_ip: Option<bool>,
    pub override_destination: Option<bool>,
    pub protocols: String,
    original_protocols: Vec<SnifferProtocolSettings>,
    pub force_domain: String,
    pub skip_domain: String,
    pub skip_src_address: String,
    pub skip_dst_address: String,
}

impl SnifferFormState {
    fn from_settings(settings: SnifferConfigSettings) -> Self {
        let protocols = settings
            .protocols
            .iter()
            .map(format_sniffer_protocol_line)
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            enable: settings.enable,
            force_dns_mapping: settings.force_dns_mapping,
            parse_pure_ip: settings.parse_pure_ip,
            override_destination: settings.override_destination,
            protocols,
            original_protocols: settings.protocols,
            force_domain: settings.force_domain.join("\n"),
            skip_domain: settings.skip_domain.join("\n"),
            skip_src_address: settings.skip_src_address.join("\n"),
            skip_dst_address: settings.skip_dst_address.join("\n"),
        }
    }

    fn to_settings(&self) -> SnifferConfigSettings {
        SnifferConfigSettings {
            enable: self.enable,
            force_dns_mapping: self.force_dns_mapping,
            parse_pure_ip: self.parse_pure_ip,
            override_destination: self.override_destination,
            protocols: parse_protocol_lines(&self.protocols, &self.original_protocols),
            force_domain: split_lines(&self.force_domain),
            skip_domain: split_lines(&self.skip_domain),
            skip_src_address: split_lines(&self.skip_src_address),
            skip_dst_address: split_lines(&self.skip_dst_address),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DnsFormState {
    pub enable: Option<bool>,
    pub cache_algorithm: String,
    pub prefer_h3: Option<bool>,
    pub use_hosts: Option<bool>,
    pub use_system_hosts: Option<bool>,
    pub respect_rules: Option<bool>,
    pub listen: String,
    pub ipv6: Option<bool>,
    pub enhanced_mode: String,
    pub fake_ip_range: String,
    pub fake_ip_range6: String,
    pub fake_ip_filter_mode: String,
    pub fake_ip_filter: String,
    pub fake_ip_ttl: String,
    pub default_nameserver: String,
    pub nameserver: String,
    pub fallback: String,
    pub proxy_server_nameserver: String,
    pub proxy_server_nameserver_policy: String,
    pub(crate) original_proxy_server_nameserver_policy: Vec<DnsNameserverPolicySettings>,
    pub direct_nameserver: String,
    pub nameserver_policy: String,
    pub(crate) original_nameserver_policy: Vec<DnsNameserverPolicySettings>,
    pub direct_nameserver_follow_policy: Option<bool>,
    pub fallback_geoip: Option<bool>,
    pub fallback_geoip_code: String,
    pub fallback_geosite: String,
    pub fallback_ipcidr: String,
    pub fallback_domain: String,
    pub(crate) original_fallback_filter: Option<FallbackFilterConfig>,
}
