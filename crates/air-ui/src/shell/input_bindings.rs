use super::*;

pub(super) fn config_input_subscriptions(
    inputs: &config_editor::ConfigEditorInputs,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Vec<Subscription> {
    use config_editor::ConfigTextField as Field;

    vec![
        config_input_subscription(
            &inputs.global_mixed_port,
            Field::GlobalMixedPort,
            window,
            cx,
        ),
        config_input_subscription(&inputs.global_port, Field::GlobalHttpPort, window, cx),
        config_input_subscription(
            &inputs.global_socks_port,
            Field::GlobalSocksPort,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_redir_port,
            Field::GlobalRedirPort,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_tproxy_port,
            Field::GlobalTproxyPort,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_bind_address,
            Field::GlobalBindAddress,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_lan_allowed_ips,
            Field::GlobalLanAllowedIps,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_lan_disallowed_ips,
            Field::GlobalLanDisallowedIps,
            window,
            cx,
        ),
        config_input_subscription(&inputs.global_mode, Field::GlobalMode, window, cx),
        config_input_subscription(&inputs.global_log_level, Field::GlobalLogLevel, window, cx),
        config_input_subscription(
            &inputs.global_keep_alive_interval,
            Field::GlobalKeepAliveInterval,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_keep_alive_idle,
            Field::GlobalKeepAliveIdle,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_find_process_mode,
            Field::GlobalFindProcessMode,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_controller,
            Field::GlobalController,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_controller_cors_allow_origins,
            Field::GlobalControllerCorsAllowOrigins,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_doh_server,
            Field::GlobalDohServer,
            window,
            cx,
        ),
        config_input_subscription(&inputs.global_secret, Field::GlobalSecret, window, cx),
        config_input_subscription(
            &inputs.global_authentication,
            Field::GlobalAuthentication,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_skip_auth_prefixes,
            Field::GlobalSkipAuthPrefixes,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_interface_name,
            Field::GlobalInterfaceName,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_routing_mark,
            Field::GlobalRoutingMark,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_geodata_loader,
            Field::GlobalGeodataLoader,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_geo_update_interval,
            Field::GlobalGeoUpdateInterval,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_geox_geoip,
            Field::GlobalGeoxGeoip,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.global_geox_geosite,
            Field::GlobalGeoxGeosite,
            window,
            cx,
        ),
        config_input_subscription(&inputs.global_geox_mmdb, Field::GlobalGeoxMmdb, window, cx),
        config_input_subscription(&inputs.global_geox_asn, Field::GlobalGeoxAsn, window, cx),
        config_input_subscription(&inputs.global_ua, Field::GlobalUa, window, cx),
        config_input_subscription(&inputs.tun_stack, Field::TunStack, window, cx),
        config_input_subscription(&inputs.tun_device, Field::TunDevice, window, cx),
        config_input_subscription(&inputs.tun_dns_hijack, Field::TunDnsHijack, window, cx),
        config_input_subscription(&inputs.tun_mtu, Field::TunMtu, window, cx),
        config_input_subscription(&inputs.tun_gso_max_size, Field::TunGsoMaxSize, window, cx),
        config_input_subscription(
            &inputs.tun_inet6_address,
            Field::TunInet6Address,
            window,
            cx,
        ),
        config_input_subscription(&inputs.tun_udp_timeout, Field::TunUdpTimeout, window, cx),
        config_input_subscription(
            &inputs.tun_iproute2_table_index,
            Field::TunIproute2TableIndex,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.tun_iproute2_rule_index,
            Field::TunIproute2RuleIndex,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.tun_route_address_set,
            Field::TunRouteAddressSet,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.tun_route_exclude_address_set,
            Field::TunRouteExcludeAddressSet,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.tun_route_address,
            Field::TunRouteAddress,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.tun_route_exclude,
            Field::TunRouteExclude,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.tun_include_interface,
            Field::TunIncludeInterface,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.tun_exclude_interface,
            Field::TunExcludeInterface,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.sniffer_protocols,
            Field::SnifferProtocols,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.sniffer_force_domain,
            Field::SnifferForceDomain,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.sniffer_skip_domain,
            Field::SnifferSkipDomain,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.sniffer_skip_src_address,
            Field::SnifferSkipSrcAddress,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.sniffer_skip_dst_address,
            Field::SnifferSkipDstAddress,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.dns_cache_algorithm,
            Field::DnsCacheAlgorithm,
            window,
            cx,
        ),
        config_input_subscription(&inputs.dns_listen, Field::DnsListen, window, cx),
        config_input_subscription(
            &inputs.dns_enhanced_mode,
            Field::DnsEnhancedMode,
            window,
            cx,
        ),
        config_input_subscription(&inputs.dns_fake_ip_range, Field::DnsFakeIpRange, window, cx),
        config_input_subscription(
            &inputs.dns_fake_ip_range6,
            Field::DnsFakeIpRange6,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.dns_fake_ip_filter_mode,
            Field::DnsFakeIpFilterMode,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.dns_fake_ip_filter,
            Field::DnsFakeIpFilter,
            window,
            cx,
        ),
        config_input_subscription(&inputs.dns_fake_ip_ttl, Field::DnsFakeIpTtl, window, cx),
        config_input_subscription(
            &inputs.dns_default_nameserver,
            Field::DnsDefaultNameserver,
            window,
            cx,
        ),
        config_input_subscription(&inputs.dns_nameserver, Field::DnsNameserver, window, cx),
        config_input_subscription(&inputs.dns_fallback, Field::DnsFallback, window, cx),
        config_input_subscription(
            &inputs.dns_proxy_server_nameserver,
            Field::DnsProxyServerNameserver,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.dns_proxy_server_nameserver_policy,
            Field::DnsProxyServerNameserverPolicy,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.dns_direct_nameserver,
            Field::DnsDirectNameserver,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.dns_nameserver_policy,
            Field::DnsNameserverPolicy,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.dns_fallback_geoip_code,
            Field::DnsFallbackGeoipCode,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.dns_fallback_geosite,
            Field::DnsFallbackGeosite,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.dns_fallback_ipcidr,
            Field::DnsFallbackIpcidr,
            window,
            cx,
        ),
        config_input_subscription(
            &inputs.dns_fallback_domain,
            Field::DnsFallbackDomain,
            window,
            cx,
        ),
    ]
}

pub(super) fn config_input_subscription(
    input: &Entity<InputState>,
    field: config_editor::ConfigTextField,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Subscription {
    cx.subscribe_in(input, window, move |this, input, event, window, cx| {
        if matches!(event, InputEvent::Change) {
            if let Some(command) = this
                .config_editor
                .update_text(field, input.read(cx).value().to_string())
            {
                this.dispatch_command(command);
            }
            this.notify_config_result(window, cx);
            cx.notify();
        }
    })
}

#[derive(Clone, Copy)]
pub(super) enum ConnectionFilterField {
    Search,
}

pub(super) fn connection_input_subscriptions(
    inputs: &connections::ConnectionsPageInputs,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Vec<Subscription> {
    vec![
        connection_input_subscription(&inputs.search, ConnectionFilterField::Search, window, cx),
        cx.subscribe_in(&inputs.status, window, |this, _, event, _, cx| {
            let SelectEvent::Confirm(Some(label)) = event else {
                return;
            };
            if let Some(status) = connections::ConnectionStatusFilter::from_label(label) {
                this.set_connection_status_filter(status);
                cx.notify();
            }
        }),
        cx.subscribe_in(&inputs.sort_field, window, |this, _, event, _, cx| {
            let SelectEvent::Confirm(Some(label)) = event else {
                return;
            };
            if let Some(field) = connections::ConnectionSortField::from_label(label) {
                this.set_connection_sort_field(field);
                cx.notify();
            }
        }),
    ]
}

pub(super) fn connection_input_subscription(
    input: &Entity<InputState>,
    field: ConnectionFilterField,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Subscription {
    cx.subscribe_in(input, window, move |this, input, event, _, cx| {
        if matches!(event, InputEvent::Change) {
            let value = input.read(cx).value().to_string();
            match field {
                ConnectionFilterField::Search => this.connections.set_search_query(value),
            }
            cx.notify();
        }
    })
}

pub(super) fn subscription_input_subscriptions(
    inputs: &subscriptions::SubscriptionPageInputs,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Vec<Subscription> {
    use subscriptions::SubscriptionConfigFormField as ConfigField;
    use subscriptions::SubscriptionFormField as Field;

    vec![
        subscription_input_subscription(&inputs.import_url, Field::ImportUrl, window, cx),
        subscription_input_subscription(&inputs.name, Field::Name, window, cx),
        subscription_input_subscription(&inputs.url, Field::Url, window, cx),
        subscription_input_subscription(&inputs.interval_hours, Field::IntervalHours, window, cx),
        subscription_input_subscription(&inputs.user_agent, Field::UserAgent, window, cx),
        subscription_input_subscription(&inputs.proxy, Field::Proxy, window, cx),
        subscription_input_subscription(&inputs.request_headers, Field::RequestHeaders, window, cx),
        subscription_config_input_subscription(&inputs.config_name, ConfigField::Name, window, cx),
        subscription_config_input_subscription(
            &inputs.config_interval_hours,
            ConfigField::IntervalHours,
            window,
            cx,
        ),
        subscription_config_input_subscription(
            &inputs.config_proxy_count,
            ConfigField::ProxyCount,
            window,
            cx,
        ),
        subscription_config_input_subscription(
            &inputs.config_usage_used_gb,
            ConfigField::UsageUsedGb,
            window,
            cx,
        ),
        subscription_config_input_subscription(
            &inputs.config_usage_total_gb,
            ConfigField::UsageTotalGb,
            window,
            cx,
        ),
    ]
}

pub(super) fn subscription_input_subscription(
    input: &Entity<InputState>,
    field: subscriptions::SubscriptionFormField,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Subscription {
    cx.subscribe_in(input, window, move |this, input, event, _, cx| {
        if matches!(event, InputEvent::Change) {
            this.subscriptions
                .update_form_field(field, input.read(cx).value().to_string());
            cx.notify();
        }
    })
}

pub(super) fn subscription_config_input_subscription(
    input: &Entity<InputState>,
    field: subscriptions::SubscriptionConfigFormField,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Subscription {
    cx.subscribe_in(input, window, move |this, input, event, _, cx| {
        if matches!(event, InputEvent::Change) {
            this.subscriptions
                .update_config_form_field(field, input.read(cx).value().to_string());
            cx.notify();
        }
    })
}
