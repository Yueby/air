use super::*;
use air_app::AppCommand;
use air_config::DEFAULT_OVERRIDE_SCRIPT;

#[test]
fn save_command_carries_current_script_and_enabled_state() {
    let mut state = OverrideScriptPageState::new(false, DEFAULT_OVERRIDE_SCRIPT.to_string());
    state.set_script("function override(_, config) { return config; }");
    state.set_enabled(true);

    let command = state.save();

    assert!(matches!(
        command,
        AppCommand::SaveOverrideScript {
            enabled: true,
            ref script
        } if script.contains("return config")
    ));
    assert!(!state.view_model().dirty);
}

#[test]
fn debug_opens_loading_modal_without_marking_saved() {
    let mut state = OverrideScriptPageState::default();
    state.set_script("function override() { return {}; }");

    let command = state.debug();

    assert!(matches!(command, AppCommand::DebugOverrideScript { .. }));
    assert!(matches!(
        state.view_model().preview_modal,
        OverridePreviewModalState::Loading
    ));
    assert!(state.view_model().dirty);
}
