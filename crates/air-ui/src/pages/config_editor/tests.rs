use super::*;
use air_app::AppCommand;
use air_config::ConfigDocument;

#[test]
fn invalid_numeric_text_is_not_diagnosed_until_runtime_write() {
    let mut state = ConfigEditorPageState::fake_for_test();

    let command = state.update_text(ConfigTextField::GlobalMixedPort, "bad-port");
    let save = state.save_group(ConfigEditorGroup::Global);

    assert!(command.is_none());
    assert!(matches!(save, Some(AppCommand::SaveConfig { .. })));
    assert_eq!(state.view_model().draft.global.mixed_port, "");
    assert_eq!(
        state.notice.as_ref().map(|notice| notice.level),
        Some(ConfigNoticeLevel::Success)
    );
}

#[test]
fn valid_update_saves_after_explicit_group_save() {
    let mut state = ConfigEditorPageState::fake_for_test();

    let command = state.update_text(ConfigTextField::GlobalMixedPort, "19090");
    let save = state.save_group(ConfigEditorGroup::Global);

    assert!(command.is_none());
    assert!(matches!(save, Some(AppCommand::SaveConfig { .. })));
    assert_eq!(state.document.global.mixed_port, Some(19090));
    assert!(state.view_model().dirty_groups.is_empty());
}

#[test]
fn save_request_notice_uses_readable_text() {
    let mut state = ConfigEditorPageState::fake_for_test();

    state.update_text(ConfigTextField::GlobalMixedPort, "19090");
    let save = state.save_group(ConfigEditorGroup::Global);

    assert!(matches!(save, Some(AppCommand::SaveConfig { .. })));
    let notice = state
        .notice
        .as_ref()
        .expect("save should set a local notice");
    assert_eq!(notice.level, ConfigNoticeLevel::Success);
    assert_eq!(notice.message, "配置保存请求已提交");
    assert!(
        !notice.message.contains('?'),
        "保存提示不能退化成问号占位文本"
    );
}

#[test]
fn persisted_runtime_mode_sync_does_not_leave_global_dirty_by_itself() {
    let mut state = ConfigEditorPageState::from_document(
        ConfigDocument::parse("mode: rule\nmixed-port: 7890\n")
            .unwrap()
            .typed,
    );

    state.apply_persisted_runtime_mode("direct");

    assert_eq!(state.document.global.mode.as_deref(), Some("direct"));
    assert_eq!(state.view_model().draft.global.mode, "direct");
    assert!(
        !state
            .view_model()
            .dirty_groups
            .contains(&ConfigEditorGroup::Global)
    );
}
