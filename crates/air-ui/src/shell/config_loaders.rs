use super::*;

pub(super) fn config_save_notice_key(group: config_editor::ConfigEditorGroup) -> &'static str {
    match group {
        config_editor::ConfigEditorGroup::Global => "settings-config-save-global",
        config_editor::ConfigEditorGroup::Tun => "settings-config-save-tun",
        config_editor::ConfigEditorGroup::Sniffer => "settings-config-save-sniffer",
        config_editor::ConfigEditorGroup::Dns => "settings-config-save-dns",
    }
}

pub(super) fn should_push_config_notice(level: config_editor::ConfigNoticeLevel) -> bool {
    // 成功态由后台命令确认后统一通知；错误和警告是本地构建 YAML 等即时问题，需要立刻反馈。
    !matches!(level, config_editor::ConfigNoticeLevel::Success)
}

pub(super) fn sync_platform_autostart(enabled: bool) {
    if let Err(error) = air_platform::autostart::set_enabled(enabled) {
        tracing::warn!(%error, enabled, "failed to synchronize platform autostart setting");
    }
}

pub(super) fn load_initial_config_editor(
    router: Option<&AppCommandRouter>,
) -> config_editor::ConfigEditorPageState {
    router
        .and_then(
            |router| match router.services().current_profile_document() {
                Ok(Some(document)) => Some(config_editor::ConfigEditorPageState::from_document(
                    document.typed,
                )),
                Ok(None) => None,
                Err(error) => {
                    tracing::warn!(%error, "failed to load current profile for config editor");
                    None
                }
            },
        )
        .unwrap_or_else(config_editor::ConfigEditorPageState::empty)
}

pub(super) fn load_saved_tun_enabled(router: Option<&AppCommandRouter>) -> Option<bool> {
    router.and_then(
        |router| match router.services().current_profile_document() {
            Ok(Some(document)) => Some(
                document
                    .typed
                    .tun
                    .as_ref()
                    .and_then(|tun| tun.enable)
                    .unwrap_or(false),
            ),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(%error, "failed to load saved tun state for status bar");
                None
            }
        },
    )
}

pub(super) fn load_saved_runtime_mode(router: Option<&AppCommandRouter>) -> Option<String> {
    router.and_then(
        |router| match router.services().current_profile_document() {
            Ok(Some(document)) => Some(status_runtime_mode_value(
                document.typed.global.mode.as_deref().unwrap_or("rule"),
            )),
            Ok(None) => None,
            Err(error) => {
                tracing::warn!(%error, "failed to load saved runtime mode for status bar");
                None
            }
        },
    )
}

pub(super) fn load_initial_override_script(router: Option<&AppCommandRouter>) -> String {
    router
        .and_then(|router| router.services().load_override_script().ok())
        .unwrap_or_else(|| air_config::DEFAULT_OVERRIDE_SCRIPT.to_string())
}

pub(super) fn load_initial_subscriptions(
    router: Option<&AppCommandRouter>,
) -> subscriptions::SubscriptionPageState {
    router
        .and_then(|router| match router.services().subscription_projection() {
            Ok(projection) => Some(subscriptions::SubscriptionPageState::from_projection(
                projection,
            )),
            Err(error) => {
                tracing::warn!(%error, "failed to load subscription projection");
                None
            }
        })
        .unwrap_or_else(subscriptions::SubscriptionPageState::empty)
}
