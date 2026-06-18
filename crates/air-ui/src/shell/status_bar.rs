use super::*;

pub(crate) fn runtime_status_label(status: &RuntimeStatus) -> &str {
    match status {
        RuntimeStatus::Idle => "未启动",
        RuntimeStatus::Starting => "启动中",
        RuntimeStatus::Running => "运行中",
        RuntimeStatus::Stopping => "停止中",
        RuntimeStatus::Failed { .. } => "异常",
    }
}

pub(super) fn status_bar_core_item(
    snapshot: &AppSnapshot,
    tun_enabled: bool,
    core_pending: bool,
    tun_pending: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let status = snapshot.runtime.clone();
    let color = runtime_status_color(&status, palette);
    let tooltip = runtime_status_label(&status).to_string();
    let version = snapshot
        .runtime_info
        .as_ref()
        .and_then(|info| info.version.as_ref())
        .map(|version| normalize_core_version_label(version))
        .unwrap_or_else(|| "未知".to_string());
    let shell = cx.entity().clone();

    div()
        .id("status-bar-core")
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .h(px(26.0))
        .rounded_md()
        .bg(palette.subtle)
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .child(icons::icon(Icon::CircleGauge, color))
        .child(div().font_bold().text_color(palette.text).child("内核"))
        .context_menu(move |menu, _window, _cx| {
            let core_shell = shell.clone();
            let tun_shell = shell.clone();
            let restart_shell = shell.clone();
            let logs_shell = shell.clone();
            let core_running = matches!(status, RuntimeStatus::Running);
            let core_switch_disabled =
                core_pending || matches!(status, RuntimeStatus::Starting | RuntimeStatus::Stopping);
            let restart_disabled = core_pending || !core_running;
            let tun_next = !tun_enabled;
            let version_text = version.clone();

            menu.item(
                PopupMenuItem::element(move |_, _| {
                    status_menu_switch_row("启动内核", core_running, core_switch_disabled, palette)
                })
                .disabled(core_switch_disabled)
                .on_click(move |_, _window, cx| {
                    let _ = core_shell.update(cx, |shell, cx| {
                        shell.toggle_core_from_status_menu();
                        cx.notify();
                    });
                }),
            )
            .item(
                PopupMenuItem::element(move |_, _| {
                    status_menu_switch_row("启用 TUN", tun_enabled, tun_pending, palette)
                })
                .disabled(tun_pending)
                .on_click(move |_, window, cx| {
                    let _ = tun_shell.update(cx, |shell, cx| {
                        shell.toggle_tun_from_status_menu(tun_next, window, cx);
                        cx.notify();
                    });
                }),
            )
            .item(
                PopupMenuItem::new("重启内核")
                    .disabled(restart_disabled)
                    .on_click(move |_, _window, cx| {
                        let _ = restart_shell.update(cx, |shell, cx| {
                            shell.restart_core_from_status_menu();
                            cx.notify();
                        });
                    }),
            )
            .item(
                PopupMenuItem::new("查看日志").on_click(move |_, window, cx| {
                    let _ = logs_shell.update(cx, |shell, cx| {
                        shell.open_logs_from_status_menu(window, cx);
                        cx.notify();
                    });
                }),
            )
            .item(
                PopupMenuItem::element(move |_, _| {
                    status_menu_value_row("版本", version_text.clone(), palette)
                })
                .disabled(true),
            )
        })
}

pub(super) fn status_menu_switch_row(
    label: &'static str,
    checked: bool,
    disabled: bool,
    palette: ShellPalette,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .w(px(220.0))
        .text_sm()
        .text_color(if disabled {
            palette.muted
        } else {
            palette.text
        })
        .child(label)
        .child(super::components::app_switch(
            format!("status-core-menu-{label}"),
            checked,
            disabled,
            "切换状态",
        ))
}

pub(super) fn status_menu_value_row(
    label: &'static str,
    value: String,
    palette: ShellPalette,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .w(px(220.0))
        .text_sm()
        .text_color(palette.muted)
        .child(label)
        .child(div().font_bold().text_color(palette.text).child(value))
}

pub(super) fn normalize_core_version_label(version: &str) -> String {
    let trimmed = version.trim();
    if trimmed.is_empty() {
        return "未知".to_string();
    }
    if trimmed.starts_with('v') || trimmed.starts_with('V') {
        trimmed.to_string()
    } else {
        format!("v{trimmed}")
    }
}

pub(super) fn status_bar_mode_switch(
    mode: &str,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let active_mode = normalized_mode(mode);
    [("rule", "规则"), ("global", "全局"), ("direct", "直连")]
        .into_iter()
        .fold(
            div()
                .flex()
                .items_center()
                .gap_1()
                .p(px(2.0))
                .h(px(28.0))
                .rounded_md()
                .bg(palette.subtle),
            |row, (value, label)| {
                let selected = active_mode == value;
                row.child(
                    div()
                        .id(format!("status-bar-mode-{value}"))
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(24.0))
                        .px_2()
                        .rounded_md()
                        .cursor_pointer()
                        .text_color(if selected {
                            palette.active_text
                        } else {
                            palette.muted
                        })
                        .bg(if selected {
                            palette.active
                        } else {
                            palette.subtle
                        })
                        .hover(move |this| {
                            if selected {
                                this.bg(palette.active_hover)
                            } else {
                                this.bg(palette.hover)
                            }
                        })
                        .child(label)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_runtime_mode(value, window, cx);
                            cx.notify();
                        })),
                )
            },
        )
}

pub(super) fn runtime_status_color(status: &RuntimeStatus, palette: ShellPalette) -> Hsla {
    match status {
        RuntimeStatus::Running => palette.active,
        RuntimeStatus::Starting | RuntimeStatus::Stopping => palette.warning,
        RuntimeStatus::Failed { .. } => palette.danger,
        RuntimeStatus::Idle => palette.muted,
    }
}

pub(super) fn normalized_mode(mode: &str) -> &'static str {
    match mode.trim().to_ascii_lowercase().as_str() {
        "global" => "global",
        "direct" => "direct",
        _ => "rule",
    }
}

pub(super) fn status_runtime_mode_value(mode: &str) -> String {
    normalized_mode(mode).to_string()
}

pub(super) fn traffic_item(icon: Icon, value: &str, palette: ShellPalette) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(icons::icon(icon, palette.muted))
        .child(value.to_string())
}
