use gpui::Entity;
use gpui_component::Icon as ComponentIcon;
use gpui_component::setting::{SettingGroup, SettingPage};

use air_ui::icons::Icon;
use air_ui::pages::config_editor::{self, ConfigBoolField, ConfigEditorGroup, ConfigTextField};
use air_ui::shell::{Shell, ShellPalette};

use super::application_pages::{UnifiedSettingsPage, hide_setting_page_header};
use super::controls::*;
pub(super) fn sniffer_page(
    shell: Entity<Shell>,
    model: config_editor::ConfigEditorViewModel,
    inputs: config_editor::ConfigEditorInputs,
    palette: ShellPalette,
) -> SettingPage {
    let form = model.draft.sniffer.clone();
    hide_setting_page_header(SettingPage::new(UnifiedSettingsPage::Sniffer.title()))
        .icon(ComponentIcon::new(
            UnifiedSettingsPage::Sniffer.sidebar_icon(),
        ))
        .description(UnifiedSettingsPage::Sniffer.description())
        .resettable(false)
        .groups(with_config_notice_group(
            ConfigEditorGroup::Sniffer,
            &model,
            palette,
            shell.clone(),
            vec![
                SettingGroup::new().title("基础").items(vec![
                    config_switch_item(
                        "启用嗅探",
                        "是否启用域名嗅探。",
                        form.enable,
                        false,
                        ConfigBoolField::SnifferEnable,
                        shell.clone(),
                        palette,
                    ),
                    config_switch_item(
                        "覆写目标地址",
                        "是否使用嗅探结果作为实际访问目标。",
                        form.override_destination,
                        true,
                        ConfigBoolField::SnifferOverrideDestination,
                        shell.clone(),
                        palette,
                    ),
                    config_switch_item(
                        "强制 DNS 映射",
                        "对 redir-host 类型识别的流量进行强制嗅探。",
                        form.force_dns_mapping,
                        true,
                        ConfigBoolField::SnifferForceDnsMapping,
                        shell.clone(),
                        palette,
                    ),
                    config_switch_item(
                        "解析纯 IP",
                        "对所有未获取到域名的流量进行强制嗅探。",
                        form.parse_pure_ip,
                        true,
                        ConfigBoolField::SnifferParsePureIp,
                        shell.clone(),
                        palette,
                    ),
                ]),
                SettingGroup::new().title("协议设置").items(vec![
                    textarea_item(
                        "协议设置",
                        inputs.sniffer_protocols,
                        "需要嗅探的协议设置，仅支持 HTTP、TLS、QUIC；每行一个协议，可追加 override-destination=true/false。",
                        palette,
                    ),
                ]),
                SettingGroup::new().title("规则").items(vec![
                    textarea_item(
                        "强制嗅探域名",
                        inputs.sniffer_force_domain,
                        "强制进行嗅探的域名列表，支持域名通配，每行一个值。",
                        palette,
                    ),
                    textarea_item(
                        "跳过域名",
                        inputs.sniffer_skip_domain,
                        "跳过嗅探的域名列表，支持域名通配，每行一个值。",
                        palette,
                    ),
                    textarea_item(
                        "跳过来源地址",
                        inputs.sniffer_skip_src_address,
                        "跳过嗅探的来源 IP 段列表，每行一个值。",
                        palette,
                    ),
                    textarea_item(
                        "跳过目标地址",
                        inputs.sniffer_skip_dst_address,
                        "跳过嗅探的目标 IP 段列表，每行一个值。",
                        palette,
                    ),
                ]),
            ],
        ))
}

pub(super) fn dns_page(
    shell: Entity<Shell>,
    model: config_editor::ConfigEditorViewModel,
    inputs: config_editor::ConfigEditorInputs,
    palette: ShellPalette,
) -> SettingPage {
    let form = model.draft.dns.clone();
    hide_setting_page_header(SettingPage::new(UnifiedSettingsPage::Dns.title()))
        .icon(ComponentIcon::new(UnifiedSettingsPage::Dns.sidebar_icon()))
        .description(UnifiedSettingsPage::Dns.description())
        .resettable(false)
        .groups(with_config_notice_group(
            ConfigEditorGroup::Dns,
            &model,
            palette,
            shell.clone(),
            vec![
                SettingGroup::new().title("基础").items(vec![
                    config_switch_item(
                        "启用 DNS",
                        "是否启用 DNS；关闭时使用系统 DNS 解析。",
                        form.enable,
                        true,
                        ConfigBoolField::DnsEnable,
                        shell.clone(),
                        palette,
                    ),
                    config_choice_item(
                        "缓存算法",
                        "DNS 缓存算法。",
                        form.cache_algorithm.as_str(),
                        "lru",
                        vec![
                            ("lru", "LRU", Icon::RefreshCw),
                            ("arc", "ARC", Icon::Layers),
                        ],
                        ConfigTextField::DnsCacheAlgorithm,
                        shell.clone(),
                        palette,
                    ),
                    config_switch_item(
                        "优先 HTTP/3",
                        "DOH 优先使用 HTTP/3。",
                        form.prefer_h3,
                        false,
                        ConfigBoolField::DnsPreferH3,
                        shell.clone(),
                        palette,
                    ),
                    config_switch_item(
                        "使用配置 hosts",
                        "是否回应配置中的 hosts。",
                        form.use_hosts,
                        true,
                        ConfigBoolField::DnsUseHosts,
                        shell.clone(),
                        palette,
                    ),
                    config_switch_item(
                        "使用系统 hosts",
                        "是否查询系统 hosts。",
                        form.use_system_hosts,
                        true,
                        ConfigBoolField::DnsUseSystemHosts,
                        shell.clone(),
                        palette,
                    ),
                    config_switch_item(
                        "遵循规则",
                        "DNS 连接遵守路由规则，需配置代理节点域名解析服务器。",
                        form.respect_rules,
                        false,
                        ConfigBoolField::DnsRespectRules,
                        shell.clone(),
                        palette,
                    ),
                    config_switch_item(
                        "解析 IPv6",
                        "是否解析 IPv6；关闭时回应 AAAA 空解析。",
                        form.ipv6,
                        false,
                        ConfigBoolField::DnsIpv6,
                        shell.clone(),
                        palette,
                    ),
                    input_item(
                        "监听地址",
                        inputs.dns_listen,
                        "DNS 服务监听地址，支持 UDP、TCP。",
                        palette,
                    ),
                ]),
                SettingGroup::new().title("Fake-IP").items(vec![
                    config_choice_item(
                        "增强模式",
                        "mihomo 的 DNS 处理模式。",
                        form.enhanced_mode.as_str(),
                        "redir-host",
                        vec![
                            ("fake-ip", "Fake-IP", Icon::Network),
                            ("redir-host", "Redir Host", Icon::Globe),
                        ],
                        ConfigTextField::DnsEnhancedMode,
                        shell.clone(),
                        palette,
                    ),
                    input_item(
                        "Fake-IP IPv4 地址段",
                        inputs.dns_fake_ip_range,
                        "Fake-IP 下的 IPv4 地址段设置。",
                        palette,
                    ),
                    input_item(
                        "Fake-IP IPv6 地址段",
                        inputs.dns_fake_ip_range6,
                        "Fake-IP 下的 IPv6 地址段设置。",
                        palette,
                    ),
                    config_choice_item(
                        "Fake-IP 过滤模式",
                        "Fake-IP 过滤规则模式。",
                        form.fake_ip_filter_mode.as_str(),
                        "blacklist",
                        vec![
                            ("blacklist", "黑名单", Icon::CircleOff),
                            ("whitelist", "白名单", Icon::CheckCircle),
                            ("rule", "规则", Icon::ListFilter),
                        ],
                        ConfigTextField::DnsFakeIpFilterMode,
                        shell.clone(),
                        palette,
                    ),
                    textarea_item(
                        "Fake-IP 过滤规则",
                        inputs.dns_fake_ip_filter,
                        "Fake-IP 过滤规则，每行一个值；rule 模式下按路由规则语法填写。",
                        palette,
                    ),
                    input_item(
                        "Fake-IP TTL",
                        inputs.dns_fake_ip_ttl,
                        "Fake-IP 查询返回的 TTL，非必要情况下请勿修改。",
                        palette,
                    ),
                ]),
                SettingGroup::new().title("上游").items(vec![
                    textarea_item(
                        "默认 DNS 服务器",
                        inputs.dns_default_nameserver,
                        "默认 DNS 服务器，每行一个值。",
                        palette,
                    ),
                    textarea_item(
                        "DNS 服务器",
                        inputs.dns_nameserver,
                        "DNS 服务器，每行一个值。",
                        palette,
                    ),
                    textarea_item(
                        "备用 DNS 服务器",
                        inputs.dns_fallback,
                        "备用 DNS 服务器，每行一个值。",
                        palette,
                    ),
                    textarea_item(
                        "代理节点 DNS",
                        inputs.dns_proxy_server_nameserver,
                        "代理节点域名解析服务器，每行一个值。",
                        palette,
                    ),
                    textarea_item(
                        "直连 DNS",
                        inputs.dns_direct_nameserver,
                        "用于 direct 出口域名解析的 DNS 服务器，每行一个值。",
                        palette,
                    ),
                    config_switch_item(
                        "直连 DNS 跟随策略",
                        "是否遵循 nameserver-policy，仅当直连 DNS 不为空时生效。",
                        form.direct_nameserver_follow_policy,
                        false,
                        ConfigBoolField::DnsDirectFollowPolicy,
                        shell.clone(),
                        palette,
                    ),
                ]),
                SettingGroup::new().title("策略").items(vec![
                    textarea_item(
                        "DNS 分流策略",
                        inputs.dns_nameserver_policy,
                        "指定域名查询的解析服务器，每行 matcher = dns1, dns2。",
                        palette,
                    ),
                    textarea_item(
                        "代理节点 DNS 策略",
                        inputs.dns_proxy_server_nameserver_policy,
                        "仅用于节点域名解析，每行 matcher = dns1, dns2。",
                        palette,
                    ),
                ]),
                SettingGroup::new().title("Fallback 筛选").items(vec![
                    config_switch_item(
                        "启用 GEOIP",
                        "是否启用 GEOIP 判断。",
                        form.fallback_geoip,
                        true,
                        ConfigBoolField::DnsFallbackGeoip,
                        shell.clone(),
                        palette,
                    ),
                    input_item(
                        "GEOIP 国家代码",
                        inputs.dns_fallback_geoip_code,
                        "国家缩写，默认 CN。",
                        palette,
                    ),
                    textarea_item(
                        "Geosite 集合",
                        inputs.dns_fallback_geosite,
                        "被视为已污染的 geosite 集合，每行一个值。",
                        palette,
                    ),
                    textarea_item(
                        "IP CIDR",
                        inputs.dns_fallback_ipcidr,
                        "被视为污染的 IP 网段，每行一个 CIDR。",
                        palette,
                    ),
                    textarea_item(
                        "污染域名",
                        inputs.dns_fallback_domain,
                        "直接使用 fallback 解析的域名，每行一个值。",
                        palette,
                    ),
                ]),
            ],
        ))
}
