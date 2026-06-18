use super::*;

pub(super) fn create_log_runtime(window: &mut Window, cx: &mut Context<Shell>) -> LogPageRuntime {
    let monitor_search_input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("搜索日志级别或内容")
            .clean_on_escape()
    });
    let monitor_search_subscription = cx.subscribe_in(
        &monitor_search_input,
        window,
        |this, input, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.monitor
                    .set_search_query(input.read(cx).value().to_string());
                cx.notify();
            }
        },
    );

    LogPageRuntime {
        monitor_search_input,
        _monitor_search_subscription: monitor_search_subscription,
    }
}

pub(super) fn create_settings_inputs(
    settings: &AppSettings,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> settings::SettingsPageInputs {
    settings::SettingsPageInputs {
        proxy_delay_test_url: text_input(
            window,
            cx,
            "http://cp.cloudflare.com/generate_204",
            settings.normalized_proxy_delay_test_url(),
        ),
    }
}

pub(super) fn settings_input_subscriptions(
    inputs: &settings::SettingsPageInputs,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Vec<Subscription> {
    vec![settings_input_subscription(
        &inputs.proxy_delay_test_url,
        settings::SettingsTextField::ProxyDelayTestUrl,
        window,
        cx,
    )]
}

pub(super) fn settings_input_subscription(
    input: &Entity<InputState>,
    field: settings::SettingsTextField,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Subscription {
    cx.subscribe_in(input, window, move |this, input, event, _, cx| {
        if matches!(event, InputEvent::Change) {
            // 文本输入只修改应用偏好并立即持久化；真正测速时 app router 会重新读取落盘设置，
            // 保证后台命令不依赖页面是否仍然挂载。
            this.set_settings_text(field, input.read(cx).value().to_string());
            cx.notify();
        }
    })
}

pub(super) fn create_rules_proxy_runtime(
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> RulesProxyPageRuntime {
    let search_input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("过滤规则负载、类型或代理组")
            .clean_on_escape()
    });
    let subscriptions =
        vec![
            cx.subscribe_in(&search_input, window, |this, input, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.rules_proxy
                        .set_search_query(input.read(cx).value().to_string());
                    cx.notify();
                }
            }),
        ];

    RulesProxyPageRuntime {
        search_input,
        _subscriptions: subscriptions,
    }
}

pub(super) fn create_override_script_runtime(
    source: String,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> OverrideScriptPageRuntime {
    let editor = cx.new(|cx| code_editor_state(window, cx, "javascript", source));
    let preview_editor =
        cx.new(|cx| code_editor_state(window, cx, "yaml", "# 尚未生成覆写预览\n").soft_wrap(false));
    let subscriptions = vec![
        cx.subscribe_in(&editor, window, |this, input, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.override_script
                    .set_script(input.read(cx).value().to_string());
                cx.notify();
            }
        }),
    ];

    OverrideScriptPageRuntime {
        editor,
        preview_editor,
        _subscriptions: subscriptions,
    }
}

pub(super) fn create_group_runtime(
    state: &proxy_groups::GroupPageState,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> GroupPageRuntime {
    let view_model = state.view_model();
    let form = view_model.form;
    let search_input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("过滤代理组或节点")
            .default_value(view_model.search_query.clone())
    });
    let proxies_input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("每行一个 proxies 成员")
            .default_value(form.proxies.clone())
    });
    let providers_input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("每行一个 provider 名称")
            .default_value(form.providers.clone())
    });
    let filter_input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("filter")
            .default_value(form.filter.clone())
    });
    let exclude_filter_input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("exclude-filter")
            .default_value(form.exclude_filter.clone())
    });
    let subscriptions = vec![
        cx.subscribe_in(&search_input, window, |this, input, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.groups
                    .set_search_query(input.read(cx).value().to_string());
                cx.notify();
            }
        }),
        cx.subscribe_in(&proxies_input, window, |this, input, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.groups.update_form_field(
                    proxy_groups::GroupFormField::Proxies,
                    input.read(cx).value().to_string(),
                );
                cx.notify();
            }
        }),
        cx.subscribe_in(&providers_input, window, |this, input, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.groups.update_form_field(
                    proxy_groups::GroupFormField::Providers,
                    input.read(cx).value().to_string(),
                );
                cx.notify();
            }
        }),
        cx.subscribe_in(&filter_input, window, |this, input, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                this.groups.update_form_field(
                    proxy_groups::GroupFormField::Filter,
                    input.read(cx).value().to_string(),
                );
                cx.notify();
            }
        }),
        cx.subscribe_in(
            &exclude_filter_input,
            window,
            |this, input, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.groups.update_form_field(
                        proxy_groups::GroupFormField::ExcludeFilter,
                        input.read(cx).value().to_string(),
                    );
                    cx.notify();
                }
            },
        ),
    ];

    GroupPageRuntime {
        search_input,
        group_scroll_handle: ScrollHandle::default(),
        member_scroll_handle: ScrollHandle::default(),
        proxies_input,
        providers_input,
        filter_input,
        exclude_filter_input,
        _subscriptions: subscriptions,
    }
}

pub(super) fn create_connections_runtime(
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> ConnectionsPageRuntime {
    let inputs = connections::ConnectionsPageInputs {
        search: text_input(window, cx, "筛选应用、目标、类型、代理节点或网速", ""),
        status: cx.new(|cx| {
            SelectState::new(
                connections::CONNECTION_STATUS_OPTIONS.to_vec(),
                Some(gpui_component::IndexPath::default()),
                window,
                cx,
            )
        }),
        sort_field: cx.new(|cx| {
            SelectState::new(
                connections::CONNECTION_SORT_FIELD_OPTIONS.to_vec(),
                Some(gpui_component::IndexPath::default()),
                window,
                cx,
            )
        }),
    };
    let subscriptions = connection_input_subscriptions(&inputs, window, cx);
    let detail_editor = cx.new(|cx| code_editor_state(window, cx, "json", "{}").soft_wrap(false));

    ConnectionsPageRuntime {
        inputs,
        detail_editor,
        _subscriptions: subscriptions,
    }
}

pub(super) fn create_subscription_runtime(
    state: &subscriptions::SubscriptionPageState,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> SubscriptionPageRuntime {
    let view_model = state.view_model();
    let form = view_model.form;
    let config_form = view_model.config_form;
    let inputs = subscriptions::SubscriptionPageInputs {
        import_url: text_input(window, cx, "https://example.test/sub.yaml", ""),
        name: text_input(window, cx, "订阅名称", &form.name),
        // 订阅 URL 可能很长，这里允许自动换行，避免编辑弹窗出现横向滚动条。
        url: wrapping_multiline_text_input(window, cx, "https://example.test/sub.yaml", &form.url),
        interval_hours: text_input(window, cx, "24", &form.interval_hours),
        user_agent: text_input(window, cx, "留空则跟随当前内核版本", &form.user_agent),
        proxy: text_input(window, cx, "DIRECT", &form.proxy),
        request_headers: multiline_text_input(
            window,
            cx,
            "留空；每行 Name: Value",
            &form.request_headers,
        ),
        config_name: text_input(window, cx, "配置名称", &config_form.name),
        config_interval_hours: text_input(window, cx, "24", &config_form.interval_hours),
        config_proxy_count: text_input(window, cx, "0", &config_form.proxy_count),
        config_usage_used_gb: text_input(window, cx, "0.0", &config_form.usage_used_gb),
        config_usage_total_gb: text_input(window, cx, "0.0", &config_form.usage_total_gb),
        yaml_preview_editor: subscription_yaml_editor(window, cx),
    };
    let subscriptions = subscription_input_subscriptions(&inputs, window, cx);

    SubscriptionPageRuntime {
        inputs,
        _subscriptions: subscriptions,
    }
}

pub(super) fn create_config_runtime(
    state: &config_editor::ConfigEditorPageState,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> ConfigEditorPageRuntime {
    let draft = state.view_model().draft;
    let inputs = config_editor::ConfigEditorInputs {
        global_mixed_port: text_input(window, cx, "7892", &draft.global.mixed_port),
        global_port: text_input(window, cx, "7890", &draft.global.port),
        global_socks_port: text_input(window, cx, "7891", &draft.global.socks_port),
        global_redir_port: text_input(window, cx, "7893", &draft.global.redir_port),
        global_tproxy_port: text_input(window, cx, "7894", &draft.global.tproxy_port),
        global_bind_address: text_input(window, cx, "*", &draft.global.bind_address),
        global_lan_allowed_ips: multiline_text_input(
            window,
            cx,
            "0.0.0.0/0\n::/0",
            &draft.global.lan_allowed_ips,
        ),
        global_lan_disallowed_ips: multiline_text_input(
            window,
            cx,
            "默认空",
            &draft.global.lan_disallowed_ips,
        ),
        global_mode: text_input(window, cx, "rule", &draft.global.mode),
        global_log_level: text_input(window, cx, "info", &draft.global.log_level),
        global_keep_alive_interval: text_input(window, cx, "15", &draft.global.keep_alive_interval),
        global_keep_alive_idle: text_input(window, cx, "15", &draft.global.keep_alive_idle),
        global_find_process_mode: text_input(window, cx, "strict", &draft.global.find_process_mode),
        global_controller: text_input(
            window,
            cx,
            "127.0.0.1:9090",
            &draft.global.external_controller,
        ),
        global_controller_cors_allow_origins: multiline_text_input(
            window,
            cx,
            "*",
            &draft.global.external_controller_cors_allow_origins,
        ),
        global_doh_server: text_input(window, cx, "/dns-query", &draft.global.external_doh_server),
        global_secret: text_input(window, cx, "留空保留原 secret", &draft.global.secret),
        global_authentication: multiline_text_input(
            window,
            cx,
            "username:password",
            &draft.global.authentication,
        ),
        global_skip_auth_prefixes: multiline_text_input(
            window,
            cx,
            "127.0.0.1/8\n::1/128",
            &draft.global.skip_auth_prefixes,
        ),
        global_interface_name: text_input(window, cx, "en0", &draft.global.interface_name),
        global_routing_mark: text_input(window, cx, "6666", &draft.global.routing_mark),
        global_geodata_loader: text_input(
            window,
            cx,
            "memconservative",
            &draft.global.geodata_loader,
        ),
        global_geo_update_interval: text_input(window, cx, "24", &draft.global.geo_update_interval),
        global_geox_geoip: text_input(
            window,
            cx,
            "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geoip.dat",
            &draft.global.geox_geoip,
        ),
        global_geox_geosite: text_input(
            window,
            cx,
            "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/geosite.dat",
            &draft.global.geox_geosite,
        ),
        global_geox_mmdb: text_input(
            window,
            cx,
            "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@release/country.mmdb",
            &draft.global.geox_mmdb,
        ),
        global_geox_asn: text_input(
            window,
            cx,
            "https://github.com/xishang0128/geoip/releases/download/latest/GeoLite2-ASN.mmdb",
            &draft.global.geox_asn,
        ),
        global_ua: text_input(window, cx, "clash.meta", &draft.global.global_ua),
        tun_stack: text_input(window, cx, "gvisor", &draft.tun.stack),
        tun_device: text_input(window, cx, "utun0", &draft.tun.device),
        tun_dns_hijack: multiline_text_input(window, cx, "any:53", &draft.tun.dns_hijack),
        tun_mtu: text_input(window, cx, "1500", &draft.tun.mtu),
        tun_gso_max_size: text_input(window, cx, "65536", &draft.tun.gso_max_size),
        tun_inet6_address: text_input(
            window,
            cx,
            "fdfe:dcba:9876::1/126",
            &draft.tun.inet6_address,
        ),
        tun_udp_timeout: text_input(window, cx, "300", &draft.tun.udp_timeout),
        tun_iproute2_table_index: text_input(window, cx, "2022", &draft.tun.iproute2_table_index),
        tun_iproute2_rule_index: text_input(window, cx, "9000", &draft.tun.iproute2_rule_index),
        tun_route_address_set: multiline_text_input(
            window,
            cx,
            "ruleset-1",
            &draft.tun.route_address_set,
        ),
        tun_route_exclude_address_set: multiline_text_input(
            window,
            cx,
            "ruleset-2",
            &draft.tun.route_exclude_address_set,
        ),
        tun_route_address: multiline_text_input(window, cx, "0.0.0.0/1", &draft.tun.route_address),
        tun_route_exclude: multiline_text_input(
            window,
            cx,
            "192.168.0.0/16",
            &draft.tun.route_exclude_address,
        ),
        tun_include_interface: multiline_text_input(
            window,
            cx,
            "接口名称",
            &draft.tun.include_interface,
        ),
        tun_exclude_interface: multiline_text_input(
            window,
            cx,
            "接口名称",
            &draft.tun.exclude_interface,
        ),
        sniffer_protocols: multiline_text_input(
            window,
            cx,
            "HTTP: 80, 8080-8880; override-destination=true\nTLS: 443, 8443\nQUIC: 443, 8443",
            &draft.sniffer.protocols,
        ),
        sniffer_force_domain: multiline_text_input(
            window,
            cx,
            "+.v2ex.com",
            &draft.sniffer.force_domain,
        ),
        sniffer_skip_domain: multiline_text_input(
            window,
            cx,
            "Mijia Cloud",
            &draft.sniffer.skip_domain,
        ),
        sniffer_skip_src_address: multiline_text_input(
            window,
            cx,
            "192.168.0.3/32",
            &draft.sniffer.skip_src_address,
        ),
        sniffer_skip_dst_address: multiline_text_input(
            window,
            cx,
            "192.168.0.3/32",
            &draft.sniffer.skip_dst_address,
        ),
        dns_cache_algorithm: text_input(window, cx, "lru", &draft.dns.cache_algorithm),
        dns_listen: text_input(window, cx, "0.0.0.0:1053", &draft.dns.listen),
        dns_enhanced_mode: text_input(window, cx, "redir-host", &draft.dns.enhanced_mode),
        dns_fake_ip_range: text_input(window, cx, "198.18.0.1/16", &draft.dns.fake_ip_range),
        dns_fake_ip_range6: text_input(
            window,
            cx,
            "fdfe:dcba:9876::1/64",
            &draft.dns.fake_ip_range6,
        ),
        dns_fake_ip_filter_mode: text_input(
            window,
            cx,
            "blacklist",
            &draft.dns.fake_ip_filter_mode,
        ),
        dns_fake_ip_filter: multiline_text_input(window, cx, "*.lan", &draft.dns.fake_ip_filter),
        dns_fake_ip_ttl: text_input(window, cx, "1", &draft.dns.fake_ip_ttl),
        dns_default_nameserver: multiline_text_input(
            window,
            cx,
            "223.5.5.5",
            &draft.dns.default_nameserver,
        ),
        dns_nameserver: multiline_text_input(
            window,
            cx,
            "https://doh.pub/dns-query\nhttps://dns.alidns.com/dns-query",
            &draft.dns.nameserver,
        ),
        dns_fallback: multiline_text_input(
            window,
            cx,
            "tls://8.8.4.4\ntls://1.1.1.1",
            &draft.dns.fallback,
        ),
        dns_proxy_server_nameserver: multiline_text_input(
            window,
            cx,
            "https://doh.pub/dns-query",
            &draft.dns.proxy_server_nameserver,
        ),
        dns_proxy_server_nameserver_policy: multiline_text_input(
            window,
            cx,
            "www.yournode.com = 114.114.114.114",
            &draft.dns.proxy_server_nameserver_policy,
        ),
        dns_direct_nameserver: multiline_text_input(
            window,
            cx,
            "system",
            &draft.dns.direct_nameserver,
        ),
        dns_nameserver_policy: multiline_text_input(
            window,
            cx,
            "+.arpa = 10.0.0.1\nrule-set:cn = https://doh.pub/dns-query, https://dns.alidns.com/dns-query",
            &draft.dns.nameserver_policy,
        ),
        dns_fallback_geoip_code: text_input(window, cx, "CN", &draft.dns.fallback_geoip_code),
        dns_fallback_geosite: multiline_text_input(window, cx, "gfw", &draft.dns.fallback_geosite),
        dns_fallback_ipcidr: multiline_text_input(
            window,
            cx,
            "240.0.0.0/4",
            &draft.dns.fallback_ipcidr,
        ),
        dns_fallback_domain: multiline_text_input(
            window,
            cx,
            "+.google.com\n+.facebook.com\n+.youtube.com",
            &draft.dns.fallback_domain,
        ),
    };
    let subscriptions = config_input_subscriptions(&inputs, window, cx);

    ConfigEditorPageRuntime {
        inputs,
        _subscriptions: subscriptions,
    }
}
