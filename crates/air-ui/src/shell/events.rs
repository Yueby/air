use super::*;

#[cfg(test)]
pub(super) fn apply_app_event_to_state(
    snapshot: &mut AppSnapshot,
    monitor: &mut monitor::MonitorPageState,
    groups: &mut proxy_groups::GroupPageState,
    rules_proxy: &mut rules::RulesProxyPageState,
    connections: &mut connections::ConnectionsPageState,
    subscriptions: &mut subscriptions::SubscriptionPageState,
    event: AppEvent,
) -> ShellAppEventEffect {
    match event {
        AppEvent::SnapshotChanged(next_snapshot) => {
            if *snapshot == next_snapshot {
                ShellAppEventEffect::None
            } else {
                *snapshot = next_snapshot;
                ShellAppEventEffect::Redraw
            }
        }
        AppEvent::RuntimeStatusChanged(status) => {
            let groups_cleared =
                !matches!(status, RuntimeStatus::Running) && groups.clear_runtime_projection();
            if snapshot.runtime == status {
                if groups_cleared {
                    ShellAppEventEffect::Redraw
                } else {
                    ShellAppEventEffect::None
                }
            } else {
                snapshot.runtime = status;
                ShellAppEventEffect::Redraw
            }
        }
        AppEvent::UserVisibleError { message } => {
            let message = redact_log_value(&message);
            snapshot.last_error = Some(message.clone());
            ShellAppEventEffect::UserVisibleError(message)
        }
        AppEvent::UserNotification { level, message } => {
            ShellAppEventEffect::UserNotification(level, redact_log_value(&message))
        }
        AppEvent::MihomoStreamEvent(event) => {
            monitor.apply_stream_event(event.clone());
            connections.apply_stream_event(event);
            ShellAppEventEffect::Redraw
        }
        AppEvent::OverridePreviewGenerated { .. } => ShellAppEventEffect::Redraw,
        AppEvent::ConnectionsStateChanged(response) => {
            connections.apply_connections_response(response);
            ShellAppEventEffect::Redraw
        }
        AppEvent::RulesStateChanged(response) => {
            rules_proxy.apply_rules_response(response);
            ShellAppEventEffect::Redraw
        }
        AppEvent::ProxyGroupStateChanged(projection) => {
            groups.apply_runtime_projection(projection);
            ShellAppEventEffect::Redraw
        }
        AppEvent::ProxyDelayMeasured { name, delay_ms } => {
            groups.apply_proxy_delay_result(&name, delay_ms);
            ShellAppEventEffect::Redraw
        }
        AppEvent::ProxyGroupDelayMeasured {
            name,
            member_delays,
        } => {
            groups.apply_group_delay_result(&name, member_delays);
            ShellAppEventEffect::Redraw
        }
        AppEvent::SubscriptionStateChanged(projection) => {
            subscriptions.apply_projection(projection);
            ShellAppEventEffect::Redraw
        }
        AppEvent::SubscriptionYamlLoaded {
            subscription_id,
            contents,
        } => {
            subscriptions.apply_yaml_preview(&subscription_id, contents);
            ShellAppEventEffect::Redraw
        }
        AppEvent::SubscriptionUpdateCanceled { subscription_id } => {
            subscriptions.mark_update_canceled(&subscription_id);
            ShellAppEventEffect::UserNotification(
                AppNotificationLevel::Warning,
                "订阅更新已取消".to_string(),
            )
        }
        AppEvent::CommandStarted { .. } | AppEvent::CommandFinished { .. } => {
            ShellAppEventEffect::None
        }
    }
}

pub(super) fn apply_app_event_to_active_state(
    active_route: AppRoute,
    snapshot: &mut AppSnapshot,
    monitor: &mut monitor::MonitorPageState,
    groups: &mut proxy_groups::GroupPageState,
    rules_proxy: &mut rules::RulesProxyPageState,
    connections: &mut connections::ConnectionsPageState,
    subscriptions: &mut subscriptions::SubscriptionPageState,
    event: AppEvent,
) -> ShellAppEventEffect {
    // 非激活页面的运行态事件只投递给当前路由；全局快照和通知仍始终保留。
    match event {
        AppEvent::SnapshotChanged(_)
        | AppEvent::UserVisibleError { .. }
        | AppEvent::UserNotification { .. }
        | AppEvent::CommandStarted { .. }
        | AppEvent::CommandFinished { .. } => apply_app_event_to_global_state(snapshot, event),
        AppEvent::RuntimeStatusChanged(status) => {
            let groups_cleared =
                !matches!(status, RuntimeStatus::Running) && groups.clear_runtime_projection();
            let snapshot_changed = snapshot.runtime != status;
            if snapshot_changed {
                snapshot.runtime = status;
            }
            if snapshot_changed || groups_cleared {
                ShellAppEventEffect::Redraw
            } else {
                ShellAppEventEffect::None
            }
        }
        AppEvent::MihomoStreamEvent(event) => match active_route {
            AppRoute::Logs => {
                monitor.apply_stream_event(event);
                ShellAppEventEffect::Redraw
            }
            AppRoute::Connections => {
                if matches!(
                    &event,
                    StreamEvent::Traffic { .. } | StreamEvent::Disconnected { .. }
                ) {
                    // 连接页有自己的连接流处理，但状态栏仍依赖 monitor 保存最近的 `/traffic` 点。
                    monitor.apply_stream_event(event.clone());
                }
                connections.apply_stream_event(event);
                ShellAppEventEffect::Redraw
            }
            _ => {
                // 状态栏网速读取 monitor 的最近流量点，因此非日志页也要消费全局 /traffic 事件。
                if matches!(
                    &event,
                    StreamEvent::Traffic { .. } | StreamEvent::Disconnected { .. }
                ) {
                    monitor.apply_stream_event(event);
                    ShellAppEventEffect::Redraw
                } else {
                    ShellAppEventEffect::None
                }
            }
        },
        AppEvent::OverridePreviewGenerated { .. } => {
            if active_route == AppRoute::OverrideScript {
                ShellAppEventEffect::Redraw
            } else {
                ShellAppEventEffect::None
            }
        }
        AppEvent::ConnectionsStateChanged(response) => {
            if active_route == AppRoute::Connections {
                connections.apply_connections_response(response);
                ShellAppEventEffect::Redraw
            } else {
                ShellAppEventEffect::None
            }
        }
        AppEvent::RulesStateChanged(response) => {
            if active_route == AppRoute::RulesProxy {
                rules_proxy.apply_rules_response(response);
                ShellAppEventEffect::Redraw
            } else {
                ShellAppEventEffect::None
            }
        }
        AppEvent::ProxyGroupStateChanged(projection) => {
            if active_route == AppRoute::ProxyGroups {
                groups.apply_runtime_projection(projection);
                ShellAppEventEffect::Redraw
            } else {
                ShellAppEventEffect::None
            }
        }
        AppEvent::ProxyDelayMeasured { name, delay_ms } => {
            if active_route == AppRoute::ProxyGroups {
                groups.apply_proxy_delay_result(&name, delay_ms);
                ShellAppEventEffect::Redraw
            } else {
                ShellAppEventEffect::None
            }
        }
        AppEvent::ProxyGroupDelayMeasured {
            name,
            member_delays,
        } => {
            if active_route == AppRoute::ProxyGroups {
                groups.apply_group_delay_result(&name, member_delays);
                ShellAppEventEffect::Redraw
            } else {
                ShellAppEventEffect::None
            }
        }
        AppEvent::SubscriptionStateChanged(projection) => {
            if active_route == AppRoute::Subscriptions {
                subscriptions.apply_projection(projection);
                ShellAppEventEffect::Redraw
            } else {
                ShellAppEventEffect::None
            }
        }
        AppEvent::SubscriptionYamlLoaded {
            subscription_id,
            contents,
        } => {
            if active_route == AppRoute::Subscriptions {
                subscriptions.apply_yaml_preview(&subscription_id, contents);
                ShellAppEventEffect::Redraw
            } else {
                ShellAppEventEffect::None
            }
        }
        AppEvent::SubscriptionUpdateCanceled { subscription_id } => {
            if active_route == AppRoute::Subscriptions {
                subscriptions.mark_update_canceled(&subscription_id);
            }
            ShellAppEventEffect::UserNotification(
                AppNotificationLevel::Warning,
                "订阅更新已取消".to_string(),
            )
        }
    }
}

pub(super) fn apply_app_event_to_global_state(
    snapshot: &mut AppSnapshot,
    event: AppEvent,
) -> ShellAppEventEffect {
    // 托盘隐藏后页面状态已经释放，只保留全局快照和用户可见通知。
    // 流事件、页面投影和编辑器结果等到窗口恢复时再从 app service 重新装载。
    match event {
        AppEvent::SnapshotChanged(next_snapshot) => {
            if *snapshot == next_snapshot {
                ShellAppEventEffect::None
            } else {
                *snapshot = next_snapshot;
                ShellAppEventEffect::Redraw
            }
        }
        AppEvent::RuntimeStatusChanged(status) => {
            if snapshot.runtime == status {
                ShellAppEventEffect::None
            } else {
                snapshot.runtime = status;
                ShellAppEventEffect::Redraw
            }
        }
        AppEvent::UserVisibleError { message } => {
            let message = redact_log_value(&message);
            snapshot.last_error = Some(message.clone());
            ShellAppEventEffect::UserVisibleError(message)
        }
        AppEvent::UserNotification { level, message } => {
            ShellAppEventEffect::UserNotification(level, redact_log_value(&message))
        }
        AppEvent::CommandStarted { .. }
        | AppEvent::CommandFinished { .. }
        | AppEvent::MihomoStreamEvent(_)
        | AppEvent::OverridePreviewGenerated { .. }
        | AppEvent::ConnectionsStateChanged(_)
        | AppEvent::RulesStateChanged(_)
        | AppEvent::ProxyGroupStateChanged(_)
        | AppEvent::ProxyDelayMeasured { .. }
        | AppEvent::ProxyGroupDelayMeasured { .. }
        | AppEvent::SubscriptionStateChanged(_)
        | AppEvent::SubscriptionYamlLoaded { .. }
        | AppEvent::SubscriptionUpdateCanceled { .. } => ShellAppEventEffect::None,
    }
}

pub(super) fn apply_subscription_import_error(
    subscriptions: &mut subscriptions::SubscriptionPageState,
    pending_commands: &BTreeMap<CommandId, AppCommand>,
    message: &str,
) -> bool {
    let import_pending = pending_commands.values().any(|command| {
        matches!(
            command,
            AppCommand::ImportSubscriptionUrl { .. } | AppCommand::ImportSubscriptionFile { .. }
        )
    });
    if !import_pending {
        return false;
    }

    // 订阅导入失败先以全局 UserVisibleError 回到 UI；这里只释放订阅页自己的导入中状态。
    subscriptions.apply_user_error(message);
    true
}

pub(super) fn push_app_event_notice(
    window: &mut Window,
    cx: &mut Context<Shell>,
    effect: &ShellAppEventEffect,
) {
    let (level, message) = match effect {
        ShellAppEventEffect::UserVisibleError(message) => {
            (super::components::UiNoticeLevel::Error, message.clone())
        }
        ShellAppEventEffect::UserNotification(level, message) => {
            let level = match level {
                AppNotificationLevel::Info => super::components::UiNoticeLevel::Info,
                AppNotificationLevel::Success => super::components::UiNoticeLevel::Success,
                AppNotificationLevel::Warning => super::components::UiNoticeLevel::Warning,
            };
            (level, message.clone())
        }
        ShellAppEventEffect::None | ShellAppEventEffect::Redraw => return,
    };
    super::components::push_global_notice(window, cx, level, message);
}

pub(super) fn subscription_ui_notice_level(
    level: subscriptions::SubscriptionNoticeLevel,
) -> super::components::UiNoticeLevel {
    match level {
        subscriptions::SubscriptionNoticeLevel::Success => {
            super::components::UiNoticeLevel::Success
        }
        subscriptions::SubscriptionNoticeLevel::Warning => {
            super::components::UiNoticeLevel::Warning
        }
        subscriptions::SubscriptionNoticeLevel::Error => super::components::UiNoticeLevel::Error,
    }
}

pub(super) fn subscription_diagnostic_notice_level(
    severity: SubscriptionDiagnosticSeverity,
) -> super::components::UiNoticeLevel {
    match severity {
        SubscriptionDiagnosticSeverity::Info => super::components::UiNoticeLevel::Info,
        SubscriptionDiagnosticSeverity::Warning => super::components::UiNoticeLevel::Warning,
        SubscriptionDiagnosticSeverity::Error => super::components::UiNoticeLevel::Error,
    }
}
