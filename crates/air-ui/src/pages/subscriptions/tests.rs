use super::*;
use std::path::PathBuf;

use air_app::{AppCommand, SubscriptionStateProjection};
use air_mihomo::subscriptions::{
    SubscriptionCacheMetadata, SubscriptionSource, SubscriptionSourceKind, SubscriptionUpdateResult,
};

#[test]
fn url_is_redacted_in_list_view_model() {
    let state = SubscriptionPageState::fake_for_test();
    let model = state.view_model();

    let work = model.items.iter().find(|item| item.id == "work").unwrap();
    assert_eq!(work.url_label, "<redacted-url>");
    assert!(!work.url_label.contains("secret-token"));
}

#[test]
fn failed_update_keeps_last_success_cache_visible() {
    let state = SubscriptionPageState::fake_for_test();
    let model = state.view_model();

    let backup = model.items.iter().find(|item| item.id == "backup").unwrap();
    assert_eq!(
        backup.cache_state,
        SubscriptionCacheState::StaleAfterFailure
    );
    assert_eq!(backup.cache_label, "旧缓存");
    assert_ne!(backup.last_success, "-");
    assert!(backup.last_error.is_some());
}

#[test]
fn manual_update_and_cancel_emit_background_commands() {
    let mut state = SubscriptionPageState::fake_for_test();
    let _ = state.select("work");

    let update = state.update_selected();
    assert!(matches!(
        update,
        Some(AppCommand::UpdateSubscription { ref subscription_id }) if subscription_id == "work"
    ));
    assert_eq!(state.updating_id.as_deref(), Some("work"));

    let cancel = state.cancel_update();
    assert!(matches!(
        cancel,
        Some(AppCommand::CancelSubscriptionUpdate { ref subscription_id }) if subscription_id == "work"
    ));
    assert!(state.updating_id.is_none());
}

#[test]
fn manual_update_allows_disabled_subscription_cache_refresh() {
    let mut state = SubscriptionPageState::fake_for_test();

    let update = state.update_by_id("backup");

    assert!(matches!(
        update,
        Some(AppCommand::UpdateSubscription { ref subscription_id }) if subscription_id == "backup"
    ));
    assert_eq!(state.updating_id.as_deref(), Some("backup"));
}

#[test]
fn selecting_subscription_emits_real_app_command() {
    let mut state = SubscriptionPageState::fake_for_test();

    let command = state.select("backup");

    assert!(matches!(
        command,
        Some(AppCommand::SelectSubscription { ref subscription_id })
            if subscription_id == "backup"
    ));
    assert_eq!(state.selected_id.as_deref(), Some("backup"));
}

#[test]
fn selecting_current_subscription_is_noop() {
    let mut state = SubscriptionPageState::fake_for_test();
    let _ = state.select("work");
    state.notice = Some(SubscriptionNotice::warning("keep notice"));

    let command = state.select("work");

    assert!(command.is_none());
    assert_eq!(state.selected_id.as_deref(), Some("work"));
    assert_eq!(
        state.notice.as_ref().map(|notice| notice.message.as_str()),
        Some("keep notice")
    );
}

#[test]
fn projection_uses_active_subscription_as_selected_card() {
    let mut backup =
        SubscriptionSource::remote("backup", "Backup", "https://backup.example.test/sub");
    backup.enabled = true;
    let mut work = SubscriptionSource::remote("work", "Work", "https://example.test/sub");
    work.enabled = false;

    let state = SubscriptionPageState::from_projection(SubscriptionStateProjection {
        active_subscription_id: Some("backup".into()),
        sources: vec![work, backup],
        ..SubscriptionStateProjection::default()
    });

    assert_eq!(state.selected_id.as_deref(), Some("backup"));
}

#[test]
fn add_and_edit_forms_emit_persistence_commands() {
    let mut state = SubscriptionPageState::fake_for_test();

    let form = state.begin_add();
    assert_eq!(form.proxy, "DIRECT");
    assert!(form.user_agent.is_empty());
    state.update_form_field(SubscriptionFormField::Name, "New");
    state.update_form_field(SubscriptionFormField::Url, "https://new.example.test/sub");
    let add = state.save_form();
    assert!(matches!(
        add,
        Some(AppCommand::SaveSubscriptionSource { ref source })
            if source.name == "New"
    ));

    let _ = state.begin_edit_by_id("work");
    state.update_form_field(SubscriptionFormField::Name, "Work Renamed");
    let edit = state.save_form();
    assert!(matches!(
        edit,
        Some(AppCommand::SaveSubscriptionSource { ref source })
            if source.name == "Work Renamed" && source.id == "work"
    ));
}

#[test]
fn editing_card_by_context_target_does_not_activate_it() {
    let mut state = SubscriptionPageState::fake_for_test();
    assert_eq!(state.selected_id.as_deref(), Some("work"));

    let form = state.begin_edit_by_id("backup").unwrap();

    assert_eq!(form.id, "backup");
    assert_eq!(state.selected_id.as_deref(), Some("work"));
}

#[test]
fn deleting_card_by_context_target_does_not_activate_it() {
    let mut state = SubscriptionPageState::fake_for_test();
    assert_eq!(state.selected_id.as_deref(), Some("work"));

    let command = state.delete_by_id("backup");

    assert!(matches!(
        command,
        Some(AppCommand::DeleteSubscription { ref subscription_id })
            if subscription_id == "backup"
    ));
    assert_eq!(state.selected_id.as_deref(), Some("work"));
}

#[test]
fn reorder_before_emits_persisted_source_order() {
    let mut state = SubscriptionPageState::fake_for_test();
    let command = state.reorder_before("base64", "work");

    assert!(matches!(
        command,
        Some(AppCommand::ReorderSubscriptions { ref ordered_ids })
            if ordered_ids == &vec!["base64".to_string(), "work".to_string(), "backup".to_string()]
    ));
    assert_eq!(state.sources[0].id, "base64");
}

#[test]
fn form_parses_headers_without_accepting_empty_names() {
    let form = SubscriptionFormState {
        id: "sub".to_string(),
        name: "Sub".to_string(),
        url: "https://example.test/sub".to_string(),
        request_headers: "Authorization: Bearer token\n: ignored\nAccept: text/yaml".to_string(),
        ..SubscriptionFormState::default()
    };

    let source = form.to_source().unwrap();
    assert_eq!(
        source.request_headers.get("Accept").map(String::as_str),
        Some("text/yaml")
    );
    assert!(source.request_headers.get("").is_none());
}

#[test]
fn default_form_uses_direct_proxy_and_empty_headers() {
    let form = SubscriptionFormState::default();

    assert_eq!(form.proxy, "DIRECT");
    assert!(form.user_agent.is_empty());
    assert!(form.request_headers.is_empty());
}

#[test]
fn import_url_requires_basic_http_url_and_emits_import_command() {
    let mut state = SubscriptionPageState::fake_for_test();

    state.update_import_url("ftp://example.test/sub.yaml");
    assert!(!state.view_model().import_url_valid);
    assert!(state.import_url().is_none());
    assert_eq!(state.import_status, SubscriptionImportStatus::Failed);

    state.update_import_url("https://example.test/sub.yaml?token=secret");
    assert!(state.view_model().import_url_valid);
    let command = state.import_url();
    assert!(matches!(
        command,
        Some(AppCommand::ImportSubscriptionUrl { ref url, .. })
            if url == "https://example.test/sub.yaml?token=secret"
    ));
    assert!(state.import_url.is_empty());
    assert_eq!(state.import_status, SubscriptionImportStatus::Importing);
}

#[test]
fn yaml_file_selection_rejects_non_yaml_extension() {
    let path = PathBuf::from("subscription.txt");
    let validation = validate_yaml_file_selection(&path);

    assert!(!validation.accepted);
    assert!(validation.message.contains(".yaml"));
}

#[test]
fn yaml_file_selection_accepts_yaml_extension_without_reading_file() {
    let path = PathBuf::from("missing-but-valid.yml");
    let validation = validate_yaml_file_selection(&path);

    assert!(validation.accepted, "{}", validation.message);
    assert_eq!(validation.proxy_count, 0);
}

#[test]
fn config_modal_updates_metadata_without_touching_source_kind() {
    let mut state = SubscriptionPageState::fake_for_test();
    let _ = state.select("work");
    let form = state.begin_edit_config_selected();
    assert_eq!(form.id, "work");
    assert_eq!(state.modal, SubscriptionModalState::EditConfig);

    state.update_config_form_field(SubscriptionConfigFormField::Name, "Work Renamed");
    state.update_config_form_field(SubscriptionConfigFormField::ProxyCount, "99");
    state.save_config_form();

    let source = state.selected_source().unwrap();
    assert_eq!(source.name, "Work Renamed");
    assert!(matches!(source.source_kind, SubscriptionSourceKind::Remote));
    assert_eq!(state.parsed_proxy_counts.get("work"), Some(&99));
}

#[test]
fn disabled_subscription_still_shows_usage_and_expiry() {
    let mut source = SubscriptionSource::remote("disabled", "Disabled", "https://example.test/sub");
    source.enabled = false;

    let mut cache = SubscriptionCacheMetadata::new("disabled");
    let mut result = SubscriptionUpdateResult::success(1_779_403_200_000, 42_120);
    result.user_info = Some(air_mihomo::subscriptions::SubscriptionUserInfo {
        upload: Some(1024 * 1024 * 1024),
        download: Some(1024 * 1024 * 1024),
        total: Some(4 * 1024 * 1024 * 1024),
        expire: Some(1_816_327_723_000),
    });
    cache.last_update = Some(result);

    let usage = usage_from_cache(
        &source,
        Some(&cache),
        SubscriptionCacheState::Ready,
        1_779_403_200_000,
    );

    assert_eq!(usage.label, "2.0 / 4.0 GB");
    assert_eq!(usage.percent, 50.0);
    assert!(usage.expires_label.is_some());
    assert!(usage.expires_tooltip.is_some());
}
