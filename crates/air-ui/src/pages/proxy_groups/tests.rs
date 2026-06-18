use super::*;
use std::collections::BTreeMap;

use air_app::AppCommand;
use air_config::ConfigDocument;
use air_config::model::{MihomoConfigDocument, ProxyGroupKind};
use air_mihomo::groups::{
    ProxyGroupMemberSource, ProxyGroupRuntimeMember, ProxyGroupRuntimeProjection,
    ProxyGroupSelectionState,
};
use air_mihomo::proxies::ProxyDelayStatus;
use gpui::rgb;

use crate::shell::ShellPalette;

fn expanded_item(mut state: GroupPageState, name: &str) -> GroupListItem {
    state.select_group(name);
    state
        .view_model()
        .items
        .into_iter()
        .find(|item| item.name == name)
        .expect("expanded group item should exist")
}

fn test_palette() -> ShellPalette {
    ShellPalette {
        background: rgb(0x000000).into(),
        surface: rgb(0x111111).into(),
        page: rgb(0x222222).into(),
        border: rgb(0x333333).into(),
        text: rgb(0x444444).into(),
        muted: rgb(0x555555).into(),
        subtle: rgb(0x666666).into(),
        hover: rgb(0x777777).into(),
        active: rgb(0x3fa0fe).into(),
        active_hover: rgb(0x238cf0).into(),
        active_text: rgb(0xffffff).into(),
        warning: rgb(0xfbbf24).into(),
        danger: rgb(0xfb7185).into(),
    }
}

#[test]
fn view_model_lists_group_summary_and_members() {
    let state = GroupPageState::fake_for_test();
    let model = state.view_model();

    assert!(model.items.iter().any(|item| item.name == "Proxy"));
    let detail = model
        .selected
        .expect("fake state should select first group");
    assert!(!detail.proxies.is_empty() || !detail.runtime_members.is_empty());
}

#[test]
fn config_only_groups_stay_empty_without_runtime_projection() {
    let document = ConfigDocument::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/config.yaml"
    )))
    .expect("docs config should parse")
    .typed;
    let state = GroupPageState::from_document(document);

    assert!(state.view_model().items.is_empty());
}

#[test]
fn view_model_keeps_config_group_order() {
    let document = ConfigDocument::parse(
        r#"
proxy-groups:
  - name: auto-first
    type: url-test
    proxies:
      - DIRECT
  - name: proxy-second
    type: select
    proxies:
      - DIRECT
"#,
    )
    .expect("group order fixture should parse")
    .typed;
    let mut state = GroupPageState::from_document(document);
    state.apply_runtime_projection(ProxyGroupRuntimeProjection {
        states: vec![
            ProxyGroupSelectionState {
                group_name: "auto-first".to_string(),
                configured_kind: ProxyGroupKind::UrlTest,
                api_kind: Some("URLTest".to_string()),
                selected: Some("DIRECT".to_string()),
                members: vec![ProxyGroupRuntimeMember {
                    name: "DIRECT".to_string(),
                    source: ProxyGroupMemberSource::BuiltInPolicy,
                    protocol: None,
                    selected: true,
                    configured: true,
                    index_in_api: 0,
                    history: Vec::new(),
                }],
                history: Vec::new(),
            },
            ProxyGroupSelectionState {
                group_name: "proxy-second".to_string(),
                configured_kind: ProxyGroupKind::Select,
                api_kind: Some("Selector".to_string()),
                selected: Some("DIRECT".to_string()),
                members: vec![ProxyGroupRuntimeMember {
                    name: "DIRECT".to_string(),
                    source: ProxyGroupMemberSource::BuiltInPolicy,
                    protocol: None,
                    selected: true,
                    configured: true,
                    index_in_api: 0,
                    history: Vec::new(),
                }],
                history: Vec::new(),
            },
        ],
    });
    let model = state.view_model();
    let names = model
        .items
        .iter()
        .map(|item| item.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, vec!["auto-first", "proxy-second"]);
}

#[test]
fn proxy_type_labels_match_mihomo_api_and_config_names() {
    let pairs = [
        ("DIRECT", "direct"),
        ("REJECT", "reject"),
        ("PASS", "pass"),
        ("DNS", "dns"),
        ("HTTP", "http"),
        ("SOCKS", "socks5"),
        ("Shadowsocks", "ss"),
        ("ShadowsocksR", "ssr"),
        ("Snell", "snell"),
        ("VMess", "vmess"),
        ("VLESS", "vless"),
        ("Trojan", "trojan"),
        ("AnyTLS", "anytls"),
        ("Mieru", "mieru"),
        ("Sudoku", "sudoku"),
        ("Hysteria", "hysteria"),
        ("Hysteria2", "hysteria2"),
        ("TUIC", "tuic"),
        ("WireGuard", "wireguard"),
        ("Tailscale", "tailscale"),
        ("SSH", "ssh"),
        ("MASQUE", "masque"),
        ("TrustTunnel", "trusttunnel"),
        ("OpenVPN", "openvpn"),
    ];

    for (api_kind, config_kind) in pairs {
        assert_eq!(proxy_type_display_label(api_kind), api_kind);
        assert_eq!(proxy_type_display_label(config_kind), api_kind);
    }
}

#[test]
fn proxy_group_type_labels_match_mihomo_api_and_config_names() {
    let pairs = [
        ("Selector", "select", 0),
        ("URLTest", "url-test", 1),
        ("Fallback", "fallback", 2),
        ("LoadBalance", "load-balance", 3),
    ];

    for (api_kind, config_kind, sort_kind) in pairs {
        assert_eq!(proxy_group_type_display_label(api_kind), api_kind);
        assert_eq!(proxy_group_type_display_label(config_kind), api_kind);
        assert_eq!(group_sort_kind(api_kind), sort_kind);
        assert_eq!(group_sort_kind(config_kind), sort_kind);
    }
}

#[test]
fn selecting_select_group_member_returns_runtime_api_command() {
    let mut state = GroupPageState::fake_for_test();
    let command = state.select_member("Proxy", "ss1");

    assert!(matches!(
        command,
        Some(AppCommand::SelectProxy { ref group, ref proxy }) if group == "Proxy" && proxy == "ss1"
    ));
    let detail = state
        .view_model()
        .selected
        .or_else(|| {
            state.select_group("Proxy");
            state.view_model().selected
        })
        .unwrap();
    assert_eq!(detail.current.as_deref(), Some("ss1"));
    assert!(state.notice.is_none());
}

#[test]
fn non_select_group_member_selection_is_rejected() {
    let mut state = GroupPageState::fake_for_test();
    let command = state.select_member("auto", "ss1");

    assert!(command.is_none());
    assert_eq!(
        state.notice.as_ref().map(|notice| notice.level),
        Some(GroupNoticeLevel::Warning)
    );
}

#[test]
fn healthcheck_and_fixed_clear_emit_group_commands() {
    let mut state = GroupPageState::fake_for_test();

    let delay = state.test_group_delay("Proxy");
    assert!(matches!(
        delay,
        Some(AppCommand::TestProxyGroupDelay { ref name, .. }) if name == "Proxy"
    ));
    assert!(state.notice.is_none());
    let item = expanded_item(state.clone(), "Proxy");
    assert!(
        item.members
            .iter()
            .all(|member| matches!(member.delay_status, ProxyDelayStatus::Testing))
    );
    state.apply_group_delay_result(
        "Proxy",
        BTreeMap::from([("ss1".to_string(), 120), ("ss2".to_string(), 180)]),
    );
    let item = state
        .view_model()
        .items
        .into_iter()
        .find(|item| item.name == "Proxy")
        .unwrap();
    assert_eq!(item.delay_status, ProxyDelayStatus::Available);
    assert_eq!(item.delay_ms, Some(120));

    let fixed = state.clear_fixed_for("auto");
    assert!(matches!(
        fixed,
        Some(AppCommand::ClearProxyGroupFixed { ref name }) if name == "auto"
    ));
}

#[test]
fn group_members_sort_by_lowest_delay_after_group_healthcheck() {
    let mut state = GroupPageState::fake_for_test();
    state.test_group_delay("Proxy");

    state.apply_group_delay_result(
        "Proxy",
        BTreeMap::from([("ss1".to_string(), 260), ("ss2".to_string(), 80)]),
    );
    state.toggle_delay_sort();

    let model = state.view_model();
    let item = model
        .items
        .iter()
        .find(|item| item.name == "Proxy")
        .expect("Proxy group should exist");
    let names = item
        .members
        .iter()
        .map(|member| member.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names.get(0), Some(&"ss2"));
    assert_eq!(names.get(1), Some(&"ss1"));

    let detail = model
        .selected
        .expect("selected group detail should be available");
    let detail_names = detail
        .runtime_members
        .iter()
        .map(|member| member.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(detail_names.get(0), Some(&"ss2"));
    assert_eq!(detail_names.get(1), Some(&"ss1"));
}

#[test]
fn global_search_filters_groups_and_members() {
    let mut state = GroupPageState::fake_for_test();
    state.select_group("Proxy");
    state.set_search_query("ss1");

    let model = state.view_model();
    let item = model
        .items
        .iter()
        .find(|item| item.name == "Proxy")
        .expect("Proxy group should exist");

    assert!(item.expanded);
    assert_eq!(item.filter_query, "ss1");
    assert!(item.total_member_count >= item.member_count);
    assert!(
        item.members
            .iter()
            .all(|member| member.name.contains("ss1"))
    );
}

#[test]
fn selected_group_controls_right_panel_members() {
    let mut state = GroupPageState::fake_for_test();
    state.select_group("Proxy");
    let selected = state
        .view_model()
        .items
        .into_iter()
        .find(|item| item.name == "Proxy")
        .expect("Proxy group should exist");

    assert!(selected.expanded);
    assert_eq!(selected.members.len(), selected.member_count);
    assert!(selected.member_count > 0);
}

#[test]
fn member_delay_action_uses_proxy_delay_command() {
    let mut state = GroupPageState::fake_for_test();
    let command = state.test_member_delay("Proxy", "ss1");

    assert!(matches!(
        command,
        Some(AppCommand::TestProxyDelay { ref name, .. }) if name == "ss1"
    ));
    let item = expanded_item(state.clone(), "Proxy");
    let member = item
        .members
        .into_iter()
        .find(|member| member.name == "ss1")
        .unwrap();
    assert_eq!(member.delay_status, ProxyDelayStatus::Testing);
    state.apply_proxy_delay_result("ss1", 88);
    let item = expanded_item(state.clone(), "Proxy");
    let member = item
        .members
        .into_iter()
        .find(|member| member.name == "ss1")
        .unwrap();
    assert_eq!(member.delay_status, ProxyDelayStatus::Available);
    assert_eq!(member.delay_ms, Some(88));
    assert_eq!(member.protocol, "Shadowsocks");
    assert!(state.notice.is_none());
}

#[test]
fn delay_color_uses_latency_buckets() {
    let palette = test_palette();

    assert_eq!(
        delay_color(ProxyDelayStatus::Available, Some(99), palette),
        rgb(0x16a34a).into()
    );
    assert_eq!(
        delay_color(ProxyDelayStatus::Available, Some(100), palette),
        palette.active
    );
    assert_eq!(
        delay_color(ProxyDelayStatus::Available, Some(299), palette),
        palette.active
    );
    assert_eq!(
        delay_color(ProxyDelayStatus::Available, Some(300), palette),
        palette.warning
    );
    assert_eq!(
        delay_color(ProxyDelayStatus::Available, Some(799), palette),
        palette.warning
    );
    assert_eq!(
        delay_color(ProxyDelayStatus::Available, Some(800), palette),
        palette.danger
    );
    assert_eq!(
        delay_color(ProxyDelayStatus::Unknown, None, palette),
        palette.muted
    );
}

#[test]
fn runtime_member_delay_falls_back_to_history_until_manual_test_overrides_it() {
    let document = ConfigDocument::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/config.yaml"
    )))
    .expect("docs config should parse")
    .typed;
    let mut state = GroupPageState::from_document(document);
    state.apply_runtime_projection(ProxyGroupRuntimeProjection {
        states: vec![ProxyGroupSelectionState {
            group_name: "Proxy".to_string(),
            configured_kind: ProxyGroupKind::Select,
            api_kind: Some("Selector".to_string()),
            selected: Some("ss1".to_string()),
            members: vec![ProxyGroupRuntimeMember {
                name: "ss1".to_string(),
                source: ProxyGroupMemberSource::ProxyNode,
                protocol: Some("ss".to_string()),
                selected: true,
                configured: true,
                index_in_api: 0,
                history: vec![serde_json::json!({
                    "time": "2026-05-29T10:00:00+08:00",
                    "delay": 166,
                })],
            }],
            history: Vec::new(),
        }],
    });

    let item = expanded_item(state.clone(), "Proxy");
    let member = item
        .members
        .into_iter()
        .find(|member| member.name == "ss1")
        .expect("runtime member should exist");
    let display_delay = displayed_member_delay(&member);
    assert_eq!(display_delay.status, ProxyDelayStatus::Available);
    assert_eq!(display_delay.delay_ms, Some(166));

    state.apply_proxy_delay_result("ss1", 88);
    let item = expanded_item(state, "Proxy");
    let member = item
        .members
        .into_iter()
        .find(|member| member.name == "ss1")
        .expect("runtime member should exist after delay update");
    let display_delay = displayed_member_delay(&member);
    assert_eq!(display_delay.status, ProxyDelayStatus::Available);
    assert_eq!(display_delay.delay_ms, Some(88));
}

#[test]
fn selected_proxy_delay_action_uses_current_runtime_member() {
    let mut state = GroupPageState::fake_for_test();
    let target = state
        .selected_proxy_delay_target()
        .expect("fake runtime should expose a selected proxy target");
    let command = state.test_selected_proxy_delay();

    assert!(matches!(
        command,
        Some(AppCommand::TestProxyDelay { ref name, .. }) if name == &target.proxy
    ));
    let item = expanded_item(state, &target.group);
    let member = item
        .members
        .into_iter()
        .find(|member| member.name == target.proxy)
        .expect("selected runtime proxy should remain visible");
    assert_eq!(member.delay_status, ProxyDelayStatus::Testing);
}

#[test]
fn selected_proxy_delay_resolves_nested_active_proxy_group() {
    let document = ConfigDocument::parse(
        r#"
proxies:
  - name: hk-01
    type: ss
    server: example.test
    port: 443
    cipher: aes-128-gcm
    password: pass
  - name: sg-01
    type: ss
    server: example.test
    port: 443
    cipher: aes-128-gcm
    password: pass
proxy-groups:
  - name: Proxy
    type: select
    proxies:
      - Auto
      - DIRECT
  - name: Auto
    type: url-test
    proxies:
      - hk-01
      - sg-01
"#,
    )
    .expect("nested proxy group fixture should parse")
    .typed;
    let mut state = GroupPageState::from_document(document);
    state.apply_runtime_projection(ProxyGroupRuntimeProjection {
        states: vec![
            ProxyGroupSelectionState {
                group_name: "Proxy".to_string(),
                configured_kind: ProxyGroupKind::Select,
                api_kind: Some("Selector".to_string()),
                selected: Some("Auto".to_string()),
                members: vec![
                    ProxyGroupRuntimeMember {
                        name: "Auto".to_string(),
                        source: ProxyGroupMemberSource::ProxyGroup,
                        protocol: None,
                        selected: true,
                        configured: true,
                        index_in_api: 0,
                        history: Vec::new(),
                    },
                    ProxyGroupRuntimeMember {
                        name: "DIRECT".to_string(),
                        source: ProxyGroupMemberSource::BuiltInPolicy,
                        protocol: None,
                        selected: false,
                        configured: true,
                        index_in_api: 1,
                        history: Vec::new(),
                    },
                ],
                history: Vec::new(),
            },
            ProxyGroupSelectionState {
                group_name: "Auto".to_string(),
                configured_kind: ProxyGroupKind::UrlTest,
                api_kind: Some("URLTest".to_string()),
                selected: Some("hk-01".to_string()),
                members: vec![
                    ProxyGroupRuntimeMember {
                        name: "hk-01".to_string(),
                        source: ProxyGroupMemberSource::ProxyNode,
                        protocol: Some("ss".to_string()),
                        selected: true,
                        configured: true,
                        index_in_api: 0,
                        history: Vec::new(),
                    },
                    ProxyGroupRuntimeMember {
                        name: "sg-01".to_string(),
                        source: ProxyGroupMemberSource::ProxyNode,
                        protocol: Some("ss".to_string()),
                        selected: false,
                        configured: true,
                        index_in_api: 1,
                        history: Vec::new(),
                    },
                ],
                history: Vec::new(),
            },
        ],
    });

    let target = state
        .selected_proxy_delay_target()
        .expect("nested group should resolve to active leaf proxy");
    assert_eq!(target.group, "Auto");
    assert_eq!(target.proxy, "hk-01");

    let command = state.test_selected_proxy_delay();
    assert!(matches!(
        command,
        Some(AppCommand::TestProxyDelay { ref name, .. }) if name == "hk-01"
    ));
}

#[test]
fn selected_proxy_delay_ignores_group_page_filter() {
    let mut state = GroupPageState::fake_for_test();
    let expected = state
        .selected_proxy_delay_target()
        .expect("fake runtime should expose selected proxy");

    state.set_search_query("__no_matching_group__");

    assert!(state.view_model().items.is_empty());
    assert_eq!(state.selected_proxy_delay_target(), Some(expected.clone()));
    assert!(matches!(
        state.test_selected_proxy_delay(),
        Some(AppCommand::TestProxyDelay { ref name, .. }) if name == &expected.proxy
    ));
}

#[test]
fn editing_members_updates_collection_and_document() {
    let mut state = GroupPageState::fake_for_test();
    state.select_group("Proxy");
    state.begin_edit_selected();
    state.update_form_field(GroupFormField::Proxies, "DIRECT\nss1");
    state.update_form_field(GroupFormField::Providers, "provider1");
    state.update_form_field(GroupFormField::Filter, "HK");
    let command = state.save_form();
    assert!(matches!(command, Some(AppCommand::SaveConfig { .. })));

    let group = state.collection.find("Proxy").unwrap();
    assert_eq!(group.members.proxies, vec!["DIRECT", "ss1"]);
    assert_eq!(group.members.use_providers, vec!["provider1"]);
    assert_eq!(group.members.filter.as_deref(), Some("HK"));
    let written = state
        .document
        .proxy_groups
        .iter()
        .find(|group| group.name == "Proxy")
        .unwrap();
    assert_eq!(written.proxies, vec!["DIRECT", "ss1"]);
}

#[test]
fn runtime_projection_updates_current_selection_from_backend() {
    let document = ConfigDocument::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/config.yaml"
    )))
    .expect("docs config should parse")
    .typed;
    let mut state = GroupPageState::from_document(document);
    state.apply_runtime_projection(ProxyGroupRuntimeProjection {
        states: vec![ProxyGroupSelectionState {
            group_name: "Proxy".to_string(),
            configured_kind: ProxyGroupKind::Select,
            api_kind: Some("Selector".to_string()),
            selected: Some("ss2".to_string()),
            members: vec![ProxyGroupRuntimeMember {
                name: "ss2".to_string(),
                source: ProxyGroupMemberSource::ProxyNode,
                protocol: None,
                selected: true,
                configured: true,
                index_in_api: 0,
                history: Vec::new(),
            }],
            history: Vec::new(),
        }],
    });

    let item = expanded_item(state, "Proxy");
    assert_eq!(item.current, "ss2");
    assert_eq!(item.members.len(), 1);
}

#[test]
fn runtime_projection_matches_group_name_case_insensitively() {
    let document = ConfigDocument::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/config.yaml"
    )))
    .expect("docs config should parse")
    .typed;
    let mut state = GroupPageState::from_document(document);
    state.apply_runtime_projection(ProxyGroupRuntimeProjection {
        states: vec![ProxyGroupSelectionState {
            group_name: "proxy".to_string(),
            configured_kind: ProxyGroupKind::Select,
            api_kind: Some("SELECTOR".to_string()),
            selected: Some("DIRECT".to_string()),
            members: vec![ProxyGroupRuntimeMember {
                name: "DIRECT".to_string(),
                source: ProxyGroupMemberSource::BuiltInPolicy,
                protocol: None,
                selected: true,
                configured: true,
                index_in_api: 0,
                history: Vec::new(),
            }],
            history: Vec::new(),
        }],
    });

    let item = expanded_item(state, "Proxy");

    assert_eq!(item.kind, "Selector");
    assert_eq!(item.current, "DIRECT");
    assert_eq!(item.members[0].protocol, "DIRECT");
}

#[test]
fn runtime_member_protocol_uses_projection_for_subscription_nodes() {
    let mut state = GroupPageState::from_document(MihomoConfigDocument::default());
    state.apply_runtime_projection(ProxyGroupRuntimeProjection {
        states: vec![ProxyGroupSelectionState {
            group_name: "Proxy".to_string(),
            configured_kind: ProxyGroupKind::Select,
            api_kind: Some("Selector".to_string()),
            selected: Some("tls-node".to_string()),
            members: vec![ProxyGroupRuntimeMember {
                name: "tls-node".to_string(),
                source: ProxyGroupMemberSource::ProxyNode,
                protocol: Some("anytls".to_string()),
                selected: true,
                configured: true,
                index_in_api: 0,
                history: Vec::new(),
            }],
            history: Vec::new(),
        }],
    });

    let item = expanded_item(state, "Proxy");

    assert_eq!(item.members[0].protocol, "AnyTLS");
}

#[test]
fn emoji_names_are_preserved_in_group_and_member_display() {
    let document = ConfigDocument::parse(
        r#"
proxies:
  - name: 🇭🇰 Hong Kong丨01
    type: SS
    server: example.com
    port: 443
    cipher: chacha20-ietf-poly1305
    password: password
proxy-groups:
  - name: 🇭🇰 Hong Kong
    type: SELECT
    proxies:
      - 🇭🇰 Hong Kong丨01
"#,
    )
    .expect("emoji config should parse")
    .typed;
    let group_name = document.proxy_groups[0].name.clone();
    let member_name = document.proxies[0].name.clone();
    let mut state = GroupPageState::from_document(document);
    state.apply_runtime_projection(ProxyGroupRuntimeProjection {
        states: vec![ProxyGroupSelectionState {
            group_name: group_name.clone(),
            configured_kind: ProxyGroupKind::Select,
            api_kind: Some("SELECTOR".to_string()),
            selected: Some(member_name.clone()),
            members: vec![ProxyGroupRuntimeMember {
                name: member_name.clone(),
                source: ProxyGroupMemberSource::ProxyNode,
                protocol: Some("SS".to_string()),
                selected: true,
                configured: true,
                index_in_api: 0,
                history: Vec::new(),
            }],
            history: Vec::new(),
        }],
    });
    let item = state
        .view_model()
        .items
        .into_iter()
        .find(|item| item.name == "🇭🇰 Hong Kong")
        .expect("emoji group should render");

    assert_eq!(item.kind, "Selector");
    assert!(item.members.iter().any(|member| {
        member.name == "🇭🇰 Hong Kong丨01" && member.protocol == "Shadowsocks"
    }));
}

#[test]
fn runtime_only_groups_are_visible_and_selectable() {
    let mut state = GroupPageState::from_document(MihomoConfigDocument::default());
    state.apply_runtime_projection(ProxyGroupRuntimeProjection {
        states: vec![ProxyGroupSelectionState {
            group_name: "SSRDOG".to_string(),
            configured_kind: ProxyGroupKind::Select,
            api_kind: Some("Selector".to_string()),
            selected: Some("Auto".to_string()),
            members: vec![
                ProxyGroupRuntimeMember {
                    name: "Auto".to_string(),
                    source: ProxyGroupMemberSource::ProxyGroup,
                    protocol: None,
                    selected: true,
                    configured: false,
                    index_in_api: 0,
                    history: Vec::new(),
                },
                ProxyGroupRuntimeMember {
                    name: "DIRECT".to_string(),
                    source: ProxyGroupMemberSource::BuiltInPolicy,
                    protocol: None,
                    selected: false,
                    configured: false,
                    index_in_api: 1,
                    history: Vec::new(),
                },
            ],
            history: Vec::new(),
        }],
    });

    let model = state.view_model();
    let item = model
        .items
        .iter()
        .find(|item| item.name == "SSRDOG")
        .expect("runtime-only group should render");
    assert_eq!(item.current, "Auto");
    assert_eq!(item.total_member_count, 2);

    let command = state.select_member("SSRDOG", "DIRECT");
    assert!(matches!(
        command,
        Some(AppCommand::SelectProxy { ref group, ref proxy })
            if group == "SSRDOG" && proxy == "DIRECT"
    ));
}

#[test]
fn runtime_projection_keeps_expanded_runtime_group() {
    let mut state = GroupPageState::from_document(MihomoConfigDocument::default());
    let projection = ProxyGroupRuntimeProjection {
        states: vec![ProxyGroupSelectionState {
            group_name: "GLOBAL".to_string(),
            configured_kind: ProxyGroupKind::Select,
            api_kind: Some("Selector".to_string()),
            selected: Some("node-a".to_string()),
            members: vec![
                ProxyGroupRuntimeMember {
                    name: "node-a".to_string(),
                    source: ProxyGroupMemberSource::ProxyNode,
                    protocol: Some("ss".to_string()),
                    selected: true,
                    configured: false,
                    index_in_api: 0,
                    history: Vec::new(),
                },
                ProxyGroupRuntimeMember {
                    name: "DIRECT".to_string(),
                    source: ProxyGroupMemberSource::BuiltInPolicy,
                    protocol: None,
                    selected: false,
                    configured: false,
                    index_in_api: 1,
                    history: Vec::new(),
                },
            ],
            history: Vec::new(),
        }],
    };

    state.apply_runtime_projection(projection.clone());
    let expanded_before = state
        .view_model()
        .items
        .iter()
        .find(|item| item.name == "GLOBAL")
        .map(|item| item.expanded)
        .unwrap_or(false);
    assert!(expanded_before);

    state.apply_runtime_projection(projection);

    let item = state
        .view_model()
        .items
        .into_iter()
        .find(|item| item.name == "GLOBAL")
        .expect("GLOBAL group should remain visible after refresh");
    assert!(item.expanded);
    assert_eq!(item.members.len(), 2);
}
