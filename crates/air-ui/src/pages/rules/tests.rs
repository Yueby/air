use super::*;
use air_app::AppCommand;

#[test]
fn runtime_rules_are_loaded_from_rules_response() {
    let state = RulesProxyPageState::sample_for_test();
    let model = state.view_model();

    assert_eq!(model.total_count, 2);
    assert_eq!(
        model.visible_rule(0).map(|rule| rule.payload.as_str()),
        Some("🇭🇰 example.com")
    );
    assert!(model.visible_rule(0).is_some_and(|rule| !rule.disabled));
    assert!(model.visible_rule(1).is_some_and(|rule| rule.disabled));
}

#[test]
fn filter_matches_payload_type_proxy_and_state() {
    let mut state = RulesProxyPageState::sample_for_test();

    state.set_search_query("hong kong");
    let model = state.view_model();

    assert_eq!(model.visible_rule_indices.len(), 1);
    assert_eq!(
        model.visible_rule(0).map(|rule| rule.proxy.as_str()),
        Some("🇭🇰 Hong Kong丨01")
    );

    state.set_search_query("disabled");
    let model = state.view_model();
    assert_eq!(model.visible_rule_indices.len(), 1);
    assert_eq!(model.visible_rule(0).map(|rule| rule.index), Some(1));
}

#[test]
fn switch_request_returns_runtime_disable_command_without_persistence() {
    let mut state = RulesProxyPageState::sample_for_test();

    let command = state.request_rule_enabled(1, true);

    assert!(matches!(
        command,
        Some(AppCommand::DisableRule {
            index: 1,
            disabled: false
        })
    ));
    assert!(
        state
            .view_model()
            .visible_rule(1)
            .is_some_and(|rule| !rule.disabled)
    );
}
