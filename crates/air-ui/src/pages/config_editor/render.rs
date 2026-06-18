use gpui::{Context, Entity, IntoElement, ParentElement, Styled, div, px};
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement;
use gpui_component::tab::{Tab, TabBar};

use air_config::DnsConfigSettings;
use air_ui::icons::Icon;
use air_ui::shell::{Shell, ShellPalette};

use super::form_helpers::*;
use super::state::*;
impl DnsFormState {
    pub(crate) fn from_settings(settings: DnsConfigSettings) -> Self {
        Self {
            enable: settings.enable,
            cache_algorithm: settings.cache_algorithm.unwrap_or_default(),
            prefer_h3: settings.prefer_h3,
            use_hosts: settings.use_hosts,
            use_system_hosts: settings.use_system_hosts,
            respect_rules: settings.respect_rules,
            listen: settings.listen.unwrap_or_default(),
            ipv6: settings.ipv6,
            enhanced_mode: settings.enhanced_mode.unwrap_or_default(),
            fake_ip_range: settings.fake_ip_range.unwrap_or_default(),
            fake_ip_range6: settings.fake_ip_range6.unwrap_or_default(),
            fake_ip_filter_mode: settings.fake_ip_filter_mode.unwrap_or_default(),
            fake_ip_filter: settings.fake_ip_filter.join("\n"),
            fake_ip_ttl: optional_u64(settings.fake_ip_ttl),
            default_nameserver: settings.default_nameserver.join("\n"),
            nameserver: settings.nameserver.join("\n"),
            fallback: settings.fallback.join("\n"),
            proxy_server_nameserver: settings.proxy_server_nameserver.join("\n"),
            proxy_server_nameserver_policy: settings
                .proxy_server_nameserver_policy
                .iter()
                .map(format_dns_policy_line)
                .collect::<Vec<_>>()
                .join("\n"),
            original_proxy_server_nameserver_policy: settings.proxy_server_nameserver_policy,
            direct_nameserver: settings.direct_nameserver.join("\n"),
            nameserver_policy: settings
                .nameserver_policy
                .iter()
                .map(format_dns_policy_line)
                .collect::<Vec<_>>()
                .join("\n"),
            original_nameserver_policy: settings.nameserver_policy,
            direct_nameserver_follow_policy: settings.direct_nameserver_follow_policy,
            fallback_geoip: settings
                .fallback_filter
                .as_ref()
                .and_then(|filter| filter.geoip),
            fallback_geoip_code: settings
                .fallback_filter
                .as_ref()
                .and_then(|filter| filter.geoip_code.clone())
                .unwrap_or_default(),
            fallback_geosite: settings
                .fallback_filter
                .as_ref()
                .map(|filter| filter.geosite.join("\n"))
                .unwrap_or_default(),
            fallback_ipcidr: settings
                .fallback_filter
                .as_ref()
                .map(|filter| filter.ipcidr.join("\n"))
                .unwrap_or_default(),
            fallback_domain: settings
                .fallback_filter
                .as_ref()
                .map(|filter| filter.domain.join("\n"))
                .unwrap_or_default(),
            original_fallback_filter: settings.fallback_filter,
        }
    }

    pub(crate) fn to_settings(&self) -> DnsConfigSettings {
        DnsConfigSettings {
            enable: self.enable,
            cache_algorithm: optional_text(&self.cache_algorithm),
            prefer_h3: self.prefer_h3,
            use_hosts: self.use_hosts,
            use_system_hosts: self.use_system_hosts,
            respect_rules: self.respect_rules,
            listen: optional_text(&self.listen),
            ipv6: self.ipv6,
            enhanced_mode: optional_text(&self.enhanced_mode),
            fake_ip_range: optional_text(&self.fake_ip_range),
            fake_ip_range6: optional_text(&self.fake_ip_range6),
            fake_ip_filter_mode: optional_text(&self.fake_ip_filter_mode),
            fake_ip_filter: split_lines(&self.fake_ip_filter),
            fake_ip_ttl: parse_optional_u64(&self.fake_ip_ttl),
            default_nameserver: split_lines(&self.default_nameserver),
            nameserver: split_lines(&self.nameserver),
            fallback: split_lines(&self.fallback),
            proxy_server_nameserver: split_lines(&self.proxy_server_nameserver),
            proxy_server_nameserver_policy: parse_policy_lines(
                &self.proxy_server_nameserver_policy,
                &self.original_proxy_server_nameserver_policy,
            ),
            direct_nameserver: split_lines(&self.direct_nameserver),
            nameserver_policy: parse_policy_lines(
                &self.nameserver_policy,
                &self.original_nameserver_policy,
            ),
            direct_nameserver_follow_policy: self.direct_nameserver_follow_policy,
            fallback_filter: fallback_filter_from_form(
                self.fallback_geoip,
                &self.fallback_geoip_code,
                &self.fallback_geosite,
                &self.fallback_ipcidr,
                &self.fallback_domain,
                self.original_fallback_filter.as_ref(),
            ),
            ..Default::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigTextField {
    GlobalMixedPort,
    GlobalHttpPort,
    GlobalSocksPort,
    GlobalRedirPort,
    GlobalTproxyPort,
    GlobalBindAddress,
    GlobalLanAllowedIps,
    GlobalLanDisallowedIps,
    GlobalMode,
    GlobalLogLevel,
    GlobalKeepAliveInterval,
    GlobalKeepAliveIdle,
    GlobalFindProcessMode,
    GlobalController,
    GlobalControllerCorsAllowOrigins,
    GlobalDohServer,
    GlobalSecret,
    GlobalAuthentication,
    GlobalSkipAuthPrefixes,
    GlobalInterfaceName,
    GlobalRoutingMark,
    GlobalGeodataLoader,
    GlobalGeoUpdateInterval,
    GlobalGeoxGeoip,
    GlobalGeoxGeosite,
    GlobalGeoxMmdb,
    GlobalGeoxAsn,
    GlobalUa,
    TunStack,
    TunDevice,
    TunDnsHijack,
    TunMtu,
    TunGsoMaxSize,
    TunInet6Address,
    TunUdpTimeout,
    TunIproute2TableIndex,
    TunIproute2RuleIndex,
    TunRouteAddressSet,
    TunRouteExcludeAddressSet,
    TunRouteAddress,
    TunRouteExclude,
    TunIncludeInterface,
    TunExcludeInterface,
    SnifferProtocols,
    SnifferForceDomain,
    SnifferSkipDomain,
    SnifferSkipSrcAddress,
    SnifferSkipDstAddress,
    DnsCacheAlgorithm,
    DnsListen,
    DnsEnhancedMode,
    DnsFakeIpRange,
    DnsFakeIpRange6,
    DnsFakeIpFilterMode,
    DnsFakeIpFilter,
    DnsFakeIpTtl,
    DnsDefaultNameserver,
    DnsNameserver,
    DnsFallback,
    DnsProxyServerNameserver,
    DnsProxyServerNameserverPolicy,
    DnsDirectNameserver,
    DnsNameserverPolicy,
    DnsFallbackGeoipCode,
    DnsFallbackGeosite,
    DnsFallbackIpcidr,
    DnsFallbackDomain,
}

impl ConfigTextField {
    pub(crate) fn group(self) -> ConfigEditorGroup {
        match self {
            Self::GlobalMixedPort
            | Self::GlobalHttpPort
            | Self::GlobalSocksPort
            | Self::GlobalRedirPort
            | Self::GlobalTproxyPort
            | Self::GlobalBindAddress
            | Self::GlobalLanAllowedIps
            | Self::GlobalLanDisallowedIps
            | Self::GlobalMode
            | Self::GlobalLogLevel
            | Self::GlobalKeepAliveInterval
            | Self::GlobalKeepAliveIdle
            | Self::GlobalFindProcessMode
            | Self::GlobalController
            | Self::GlobalControllerCorsAllowOrigins
            | Self::GlobalDohServer
            | Self::GlobalSecret
            | Self::GlobalAuthentication
            | Self::GlobalSkipAuthPrefixes
            | Self::GlobalInterfaceName
            | Self::GlobalRoutingMark
            | Self::GlobalGeodataLoader
            | Self::GlobalGeoUpdateInterval
            | Self::GlobalGeoxGeoip
            | Self::GlobalGeoxGeosite
            | Self::GlobalGeoxMmdb
            | Self::GlobalGeoxAsn
            | Self::GlobalUa => ConfigEditorGroup::Global,
            Self::TunStack
            | Self::TunDevice
            | Self::TunDnsHijack
            | Self::TunMtu
            | Self::TunGsoMaxSize
            | Self::TunInet6Address
            | Self::TunUdpTimeout
            | Self::TunIproute2TableIndex
            | Self::TunIproute2RuleIndex
            | Self::TunRouteAddressSet
            | Self::TunRouteExcludeAddressSet
            | Self::TunRouteAddress
            | Self::TunRouteExclude
            | Self::TunIncludeInterface
            | Self::TunExcludeInterface => ConfigEditorGroup::Tun,
            Self::SnifferProtocols
            | Self::SnifferForceDomain
            | Self::SnifferSkipDomain
            | Self::SnifferSkipSrcAddress
            | Self::SnifferSkipDstAddress => ConfigEditorGroup::Sniffer,
            Self::DnsCacheAlgorithm
            | Self::DnsListen
            | Self::DnsEnhancedMode
            | Self::DnsFakeIpRange
            | Self::DnsFakeIpRange6
            | Self::DnsFakeIpFilterMode
            | Self::DnsFakeIpFilter
            | Self::DnsFakeIpTtl
            | Self::DnsDefaultNameserver
            | Self::DnsNameserver
            | Self::DnsFallback
            | Self::DnsProxyServerNameserver
            | Self::DnsProxyServerNameserverPolicy
            | Self::DnsDirectNameserver
            | Self::DnsNameserverPolicy
            | Self::DnsFallbackGeoipCode
            | Self::DnsFallbackGeosite
            | Self::DnsFallbackIpcidr
            | Self::DnsFallbackDomain => ConfigEditorGroup::Dns,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigBoolField {
    GlobalAllowLan,
    GlobalIpv6,
    GlobalDisableKeepAlive,
    GlobalControllerCorsAllowPrivateNetwork,
    GlobalStoreSelected,
    GlobalStoreFakeIp,
    GlobalUnifiedDelay,
    GlobalTcpConcurrent,
    GlobalGeodataMode,
    GlobalGeoAutoUpdate,
    TunEnable,
    TunAutoDetectInterface,
    TunAutoRoute,
    TunAutoRedirect,
    TunStrictRoute,
    TunGso,
    TunEndpointIndependentNat,
    SnifferEnable,
    SnifferForceDnsMapping,
    SnifferParsePureIp,
    SnifferOverrideDestination,
    DnsEnable,
    DnsPreferH3,
    DnsUseHosts,
    DnsUseSystemHosts,
    DnsRespectRules,
    DnsIpv6,
    DnsDirectFollowPolicy,
    DnsFallbackGeoip,
}

impl ConfigBoolField {
    pub(crate) fn group(self) -> ConfigEditorGroup {
        match self {
            Self::GlobalAllowLan
            | Self::GlobalIpv6
            | Self::GlobalDisableKeepAlive
            | Self::GlobalControllerCorsAllowPrivateNetwork
            | Self::GlobalStoreSelected
            | Self::GlobalStoreFakeIp
            | Self::GlobalUnifiedDelay
            | Self::GlobalTcpConcurrent
            | Self::GlobalGeodataMode
            | Self::GlobalGeoAutoUpdate => ConfigEditorGroup::Global,
            Self::TunEnable
            | Self::TunAutoDetectInterface
            | Self::TunAutoRoute
            | Self::TunAutoRedirect
            | Self::TunStrictRoute
            | Self::TunGso
            | Self::TunEndpointIndependentNat => ConfigEditorGroup::Tun,
            Self::SnifferEnable
            | Self::SnifferForceDnsMapping
            | Self::SnifferParsePureIp
            | Self::SnifferOverrideDestination => ConfigEditorGroup::Sniffer,
            Self::DnsEnable
            | Self::DnsPreferH3
            | Self::DnsUseHosts
            | Self::DnsUseSystemHosts
            | Self::DnsRespectRules
            | Self::DnsIpv6
            | Self::DnsDirectFollowPolicy
            | Self::DnsFallbackGeoip => ConfigEditorGroup::Dns,
        }
    }
}

#[derive(Clone)]
pub(crate) struct ConfigEditorInputs {
    pub global_mixed_port: Entity<InputState>,
    pub global_port: Entity<InputState>,
    pub global_socks_port: Entity<InputState>,
    pub global_redir_port: Entity<InputState>,
    pub global_tproxy_port: Entity<InputState>,
    pub global_bind_address: Entity<InputState>,
    pub global_lan_allowed_ips: Entity<InputState>,
    pub global_lan_disallowed_ips: Entity<InputState>,
    pub global_mode: Entity<InputState>,
    pub global_log_level: Entity<InputState>,
    pub global_keep_alive_interval: Entity<InputState>,
    pub global_keep_alive_idle: Entity<InputState>,
    pub global_find_process_mode: Entity<InputState>,
    pub global_controller: Entity<InputState>,
    pub global_controller_cors_allow_origins: Entity<InputState>,
    pub global_doh_server: Entity<InputState>,
    pub global_secret: Entity<InputState>,
    pub global_authentication: Entity<InputState>,
    pub global_skip_auth_prefixes: Entity<InputState>,
    pub global_interface_name: Entity<InputState>,
    pub global_routing_mark: Entity<InputState>,
    pub global_geodata_loader: Entity<InputState>,
    pub global_geo_update_interval: Entity<InputState>,
    pub global_geox_geoip: Entity<InputState>,
    pub global_geox_geosite: Entity<InputState>,
    pub global_geox_mmdb: Entity<InputState>,
    pub global_geox_asn: Entity<InputState>,
    pub global_ua: Entity<InputState>,
    pub tun_stack: Entity<InputState>,
    pub tun_device: Entity<InputState>,
    pub tun_dns_hijack: Entity<InputState>,
    pub tun_mtu: Entity<InputState>,
    pub tun_gso_max_size: Entity<InputState>,
    pub tun_inet6_address: Entity<InputState>,
    pub tun_udp_timeout: Entity<InputState>,
    pub tun_iproute2_table_index: Entity<InputState>,
    pub tun_iproute2_rule_index: Entity<InputState>,
    pub tun_route_address_set: Entity<InputState>,
    pub tun_route_exclude_address_set: Entity<InputState>,
    pub tun_route_address: Entity<InputState>,
    pub tun_route_exclude: Entity<InputState>,
    pub tun_include_interface: Entity<InputState>,
    pub tun_exclude_interface: Entity<InputState>,
    pub sniffer_protocols: Entity<InputState>,
    pub sniffer_force_domain: Entity<InputState>,
    pub sniffer_skip_domain: Entity<InputState>,
    pub sniffer_skip_src_address: Entity<InputState>,
    pub sniffer_skip_dst_address: Entity<InputState>,
    pub dns_cache_algorithm: Entity<InputState>,
    pub dns_listen: Entity<InputState>,
    pub dns_enhanced_mode: Entity<InputState>,
    pub dns_fake_ip_range: Entity<InputState>,
    pub dns_fake_ip_range6: Entity<InputState>,
    pub dns_fake_ip_filter_mode: Entity<InputState>,
    pub dns_fake_ip_filter: Entity<InputState>,
    pub dns_fake_ip_ttl: Entity<InputState>,
    pub dns_default_nameserver: Entity<InputState>,
    pub dns_nameserver: Entity<InputState>,
    pub dns_fallback: Entity<InputState>,
    pub dns_proxy_server_nameserver: Entity<InputState>,
    pub dns_proxy_server_nameserver_policy: Entity<InputState>,
    pub dns_direct_nameserver: Entity<InputState>,
    pub dns_nameserver_policy: Entity<InputState>,
    pub dns_fallback_geoip_code: Entity<InputState>,
    pub dns_fallback_geosite: Entity<InputState>,
    pub dns_fallback_ipcidr: Entity<InputState>,
    pub dns_fallback_domain: Entity<InputState>,
}

pub(crate) fn render_config_editor_page(
    state: &ConfigEditorPageState,
    inputs: ConfigEditorInputs,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let view_model = state.view_model();

    div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.0))
        .gap_4()
        .child(render_config_tabs(&view_model, inputs, palette, cx))
}

fn render_config_tabs(
    view_model: &ConfigEditorViewModel,
    inputs: ConfigEditorInputs,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.0))
        .gap_3()
        .child(render_group_tab_bar(view_model, palette, cx))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.0))
                .gap_4()
                .overflow_y_scrollbar()
                .child(render_active_form(view_model, inputs, palette, cx)),
        )
}

fn render_group_tab_bar(
    view_model: &ConfigEditorViewModel,
    _palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    // 使用组件库 TabBar 作为配置域的唯一切换入口，诊断定位仍通过 active_group 复用同一套状态。
    ConfigEditorGroup::all().iter().fold(
        TabBar::new("config-editor-tabs")
            .w_full()
            .segmented()
            .selected_index(view_model.active_group.tab_index())
            .on_click(cx.listener(|shell, index: &usize, _, cx| {
                shell.set_config_group(ConfigEditorGroup::from_tab_index(*index));
                cx.notify();
            })),
        |tabs, group| tabs.child(Tab::new().flex_1().label(group.label())),
    )
}

fn render_active_form(
    view_model: &ConfigEditorViewModel,
    inputs: ConfigEditorInputs,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    match view_model.active_group {
        ConfigEditorGroup::Global => render_global_form(view_model, inputs, palette, cx),
        ConfigEditorGroup::Tun => render_tun_form(view_model, inputs, palette, cx),
        ConfigEditorGroup::Sniffer => render_sniffer_form(view_model, inputs, palette, cx),
        ConfigEditorGroup::Dns => render_dns_form(view_model, inputs, palette, cx),
    }
}

fn render_global_form(
    view_model: &ConfigEditorViewModel,
    inputs: ConfigEditorInputs,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> gpui::Div {
    let form = &view_model.draft.global;
    let advanced = view_model
        .advanced_open
        .contains(&ConfigEditorGroup::Global);

    section_panel(ConfigEditorGroup::Global, view_model, palette, cx)
        .child(
            div()
                .flex()
                .gap_2()
                .child(form_input("mixed-port", inputs.global_mixed_port, palette))
                .child(form_input("port", inputs.global_port, palette))
                .child(form_input("socks-port", inputs.global_socks_port, palette)),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(form_input("external-controller", inputs.global_controller, palette))
                .child(form_input(form.secret_label(), inputs.global_secret, palette)),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(bool_chip("allow-lan", form.allow_lan, ConfigBoolField::GlobalAllowLan, palette, cx))
                .child(bool_chip("ipv6", form.ipv6, ConfigBoolField::GlobalIpv6, palette, cx))
                .child(bool_chip(
                    "store-selected",
                    form.store_selected,
                    ConfigBoolField::GlobalStoreSelected,
                    palette,
                    cx,
                ))
                .child(bool_chip(
                    "store-fake-ip",
                    form.store_fake_ip,
                    ConfigBoolField::GlobalStoreFakeIp,
                    palette,
                    cx,
                )),
        )
        .child(advanced_toggle(ConfigEditorGroup::Global, advanced, palette, cx))
        .child(if advanced {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(form_input("bind-address", inputs.global_bind_address, palette))
                        .child(form_input("mode", inputs.global_mode, palette))
                        .child(form_input("log-level", inputs.global_log_level, palette)),
                )
                .child(form_input(
                    "authentication，每行 username:password",
                    inputs.global_authentication,
                    palette,
                ))
        } else {
            risk_note(
                "高级项包含监听地址、认证和运行模式；开放 LAN 或空 secret 可能暴露 external-controller，请只在可信网络中启用。",
                palette,
            )
        })
}

fn render_tun_form(
    view_model: &ConfigEditorViewModel,
    inputs: ConfigEditorInputs,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> gpui::Div {
    let form = &view_model.draft.tun;
    let advanced = view_model.advanced_open.contains(&ConfigEditorGroup::Tun);

    section_panel(ConfigEditorGroup::Tun, view_model, palette, cx)
        .child(
            div()
                .flex()
                .gap_2()
                .child(bool_chip_with_id(
                    "tun-enable",
                    "enable",
                    form.enable,
                    ConfigBoolField::TunEnable,
                    palette,
                    cx,
                ))
                .child(bool_chip(
                    "auto-route",
                    form.auto_route,
                    ConfigBoolField::TunAutoRoute,
                    palette,
                    cx,
                ))
                .child(bool_chip(
                    "auto-detect-interface",
                    form.auto_detect_interface,
                    ConfigBoolField::TunAutoDetectInterface,
                    palette,
                    cx,
                ))
                .child(bool_chip(
                    "strict-route",
                    form.strict_route,
                    ConfigBoolField::TunStrictRoute,
                    palette,
                    cx,
                )),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(form_input("stack: system / gvisor / mixed", inputs.tun_stack, palette))
                .child(form_input("mtu", inputs.tun_mtu, palette)),
        )
        .child(form_input("dns-hijack，每行一个监听目标", inputs.tun_dns_hijack, palette))
        .child(risk_note(
            "TUN 会接管系统路由和 DNS 流量；auto-route、auto-redirect、strict-route 通常需要管理员权限，错误配置可能导致断网或局域网不可达。",
            palette,
        ))
        .child(advanced_toggle(ConfigEditorGroup::Tun, advanced, palette, cx))
        .child(if advanced {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(bool_chip(
                    "auto-redirect",
                    form.auto_redirect,
                    ConfigBoolField::TunAutoRedirect,
                    palette,
                    cx,
                ))
                .child(form_input("route-address，每行 CIDR", inputs.tun_route_address, palette))
                .child(form_input(
                    "route-exclude-address，每行 CIDR",
                    inputs.tun_route_exclude,
                    palette,
                ))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(form_input(
                            "include-interface",
                            inputs.tun_include_interface,
                            palette,
                        ))
                        .child(form_input(
                            "exclude-interface",
                            inputs.tun_exclude_interface,
                            palette,
                        )),
                )
        } else {
            div()
        })
}

fn render_sniffer_form(
    view_model: &ConfigEditorViewModel,
    inputs: ConfigEditorInputs,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> gpui::Div {
    let form = &view_model.draft.sniffer;
    let advanced = view_model
        .advanced_open
        .contains(&ConfigEditorGroup::Sniffer);

    section_panel(ConfigEditorGroup::Sniffer, view_model, palette, cx)
        .child(
            div()
                .flex()
                .gap_2()
                .child(bool_chip_with_id(
                    "sniffer-enable",
                    "enable",
                    form.enable,
                    ConfigBoolField::SnifferEnable,
                    palette,
                    cx,
                ))
                .child(bool_chip(
                    "force-dns-mapping",
                    form.force_dns_mapping,
                    ConfigBoolField::SnifferForceDnsMapping,
                    palette,
                    cx,
                ))
                .child(bool_chip(
                    "override-destination",
                    form.override_destination,
                    ConfigBoolField::SnifferOverrideDestination,
                    palette,
                    cx,
                )),
        )
        .child(form_input(
            "sniff 协议，每行 HTTP: 80, 8080-8880",
            inputs.sniffer_protocols,
            palette,
        ))
        .child(form_input(
            "force-domain，每行一个域名规则",
            inputs.sniffer_force_domain,
            palette,
        ))
        .child(risk_note(
            "override-destination 会把连接目标改写为嗅探到的域名，可能影响透明代理路径；只在需要修正 SNI/Host 识别时启用。",
            palette,
        ))
        .child(advanced_toggle(ConfigEditorGroup::Sniffer, advanced, palette, cx))
        .child(if advanced {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(bool_chip(
                    "parse-pure-ip",
                    form.parse_pure_ip,
                    ConfigBoolField::SnifferParsePureIp,
                    palette,
                    cx,
                ))
                .child(form_input(
                    "skip-domain，每行一个域名规则",
                    inputs.sniffer_skip_domain,
                    palette,
                ))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(form_input(
                            "skip-src-address，每行 CIDR",
                            inputs.sniffer_skip_src_address,
                            palette,
                        ))
                        .child(form_input(
                            "skip-dst-address，每行 CIDR",
                            inputs.sniffer_skip_dst_address,
                            palette,
                        )),
                )
        } else {
            div()
        })
}

fn render_dns_form(
    view_model: &ConfigEditorViewModel,
    inputs: ConfigEditorInputs,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> gpui::Div {
    let form = &view_model.draft.dns;
    let advanced = view_model.advanced_open.contains(&ConfigEditorGroup::Dns);

    section_panel(ConfigEditorGroup::Dns, view_model, palette, cx)
        .child(
            div()
                .flex()
                .gap_2()
                .child(bool_chip_with_id(
                    "dns-enable",
                    "enable",
                    form.enable,
                    ConfigBoolField::DnsEnable,
                    palette,
                    cx,
                ))
                .child(bool_chip(
                    "respect-rules",
                    form.respect_rules,
                    ConfigBoolField::DnsRespectRules,
                    palette,
                    cx,
                ))
                .child(bool_chip(
                    "direct-follow-policy",
                    form.direct_nameserver_follow_policy,
                    ConfigBoolField::DnsDirectFollowPolicy,
                    palette,
                    cx,
                )),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(form_input("listen", inputs.dns_listen, palette))
                .child(form_input("enhanced-mode", inputs.dns_enhanced_mode, palette))
                .child(form_input("fake-ip-range", inputs.dns_fake_ip_range, palette)),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(form_input(
                    "fake-ip-filter-mode",
                    inputs.dns_fake_ip_filter_mode,
                    palette,
                ))
                .child(form_input(
                    "fake-ip-filter，每行一个条目",
                    inputs.dns_fake_ip_filter,
                    palette,
                )),
        )
        .child(form_input(
            "nameserver，每行一个上游 DNS",
            inputs.dns_nameserver,
            palette,
        ))
        .child(risk_note(
            "fake-ip-filter-mode: rule 会按规则顺序决定 fake-ip/real-ip，规则顺序错误可能导致直连域名污染或代理域名提前泄漏真实解析。",
            palette,
        ))
        .child(advanced_toggle(ConfigEditorGroup::Dns, advanced, palette, cx))
        .child(if advanced {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(form_input(
                    "default-nameserver，每行一个基础 DNS",
                    inputs.dns_default_nameserver,
                    palette,
                ))
                .child(form_input(
                    "fallback，每行一个兜底 DNS",
                    inputs.dns_fallback,
                    palette,
                ))
                .child(form_input(
                    "nameserver-policy，每行 matcher = dns1, dns2",
                    inputs.dns_nameserver_policy,
                    palette,
                ))
        } else {
            div()
        })
}

fn section_panel(
    group: ConfigEditorGroup,
    _view_model: &ConfigEditorViewModel,
    palette: ShellPalette,
    _cx: &mut Context<Shell>,
) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .gap_3()
        .p_4()
        .rounded_md()
        .border_1()
        .border_color(palette.border)
        .bg(palette.page)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(section_title(Icon::FileCog, group.title(), palette)),
        )
}

fn form_input(
    label: &'static str,
    input: Entity<InputState>,
    palette: ShellPalette,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .gap_1()
        .child(div().text_xs().text_color(palette.muted).child(label))
        .child(Input::new(&input))
}

fn bool_chip(
    label: &'static str,
    value: Option<bool>,
    field: ConfigBoolField,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    bool_chip_with_id(label, label, value, field, palette, cx)
}
