use std::rc::Rc;

use gpui::{
    ClipboardItem, Context, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels, Size,
    StatefulInteractiveElement, Styled, div, px, size,
};
use gpui_component::StyledExt;
use gpui_component::{VirtualListScrollHandle, v_virtual_list};

use air_ui::icons::{self, Icon};
use air_ui::shell::{Shell, ShellPalette};

use super::state::{LogEntryView, LogLevel, LogLevelFilter, MonitorViewModel};

const LOG_ROW_MIN_WIDTH: f32 = 1280.0;
pub(super) fn render_log_rows(
    view_model: &MonitorViewModel,
    log_scroll_handle: VirtualListScrollHandle,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> gpui::AnyElement {
    if view_model.visible_logs.is_empty() {
        return div()
            .flex()
            .items_center()
            .justify_center()
            .h(px(240.0))
            .text_sm()
            .text_color(palette.muted)
            .child("没有匹配的日志")
            .into_any_element();
    }

    let logs = Rc::new(view_model.visible_logs.clone());
    let sizes = Rc::new(vec![log_row_size(); logs.len()]);
    v_virtual_list(
        cx.entity().clone(),
        "monitor-log-virtual-list",
        sizes,
        move |_, visible_range, _, _| {
            visible_range
                .filter_map(|index| logs.get(index).cloned())
                .map(|entry| log_row(entry, palette))
                .collect::<Vec<_>>()
        },
    )
    .track_scroll(&log_scroll_handle)
    .into_any_element()
}

pub(super) fn filter_chip(
    filter: LogLevelFilter,
    active_filter: LogLevelFilter,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let active = filter == active_filter;
    div()
        .id(format!("log-filter-{}", filter.label()))
        .flex()
        .items_center()
        .justify_center()
        .h(px(30.0))
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .text_xs()
        .font_bold()
        .bg(if active {
            palette.active
        } else {
            palette.subtle
        })
        .text_color(if active {
            palette.active_text
        } else {
            palette.text
        })
        .hover(move |this| {
            if active {
                this.bg(palette.active_hover)
            } else {
                this.bg(palette.hover)
            }
        })
        .child(filter.label())
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.set_monitor_log_filter(filter);
            cx.notify();
        }))
}

pub(super) fn copy_button(
    copy_text: String,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let enabled = !copy_text.is_empty();
    div()
        .id("monitor-copy-logs")
        .flex()
        .items_center()
        .justify_center()
        .gap_1()
        .h(px(30.0))
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .text_xs()
        .font_bold()
        .bg(if enabled {
            palette.subtle
        } else {
            palette.page
        })
        .text_color(if enabled { palette.text } else { palette.muted })
        .hover(move |this| {
            if enabled {
                this.bg(palette.hover)
            } else {
                this.bg(palette.page)
            }
        })
        .child(icons::icon(
            Icon::Copy,
            if enabled { palette.text } else { palette.muted },
        ))
        .child("复制")
        .on_click(cx.listener(move |_, _, _, cx| {
            if enabled {
                cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()));
            }
        }))
}

pub(super) fn monitor_button(
    label: &'static str,
    icon: Icon,
    enabled: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
    action: impl Fn(&mut Shell) + 'static,
) -> impl IntoElement {
    div()
        .id(format!("monitor-action-{label}"))
        .flex()
        .items_center()
        .justify_center()
        .gap_1()
        .h(px(30.0))
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .text_xs()
        .font_bold()
        .bg(if enabled {
            palette.subtle
        } else {
            palette.page
        })
        .text_color(if enabled { palette.text } else { palette.muted })
        .hover(move |this| {
            if enabled {
                this.bg(palette.hover)
            } else {
                this.bg(palette.page)
            }
        })
        .child(icons::icon(
            icon,
            if enabled { palette.text } else { palette.muted },
        ))
        .child(label)
        .on_click(cx.listener(move |shell, _, _, cx| {
            if enabled {
                action(shell);
                cx.notify();
            }
        }))
}

fn log_row(entry: LogEntryView, palette: ShellPalette) -> impl IntoElement {
    let level_color = log_level_color(entry.level, palette);
    div()
        .flex()
        .items_start()
        .gap_2()
        .min_w(px(LOG_ROW_MIN_WIDTH))
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(palette.border)
        .text_xs()
        .line_height(px(18.0))
        .child(
            div()
                .w(px(56.0))
                .flex_shrink_0()
                .font_family("monospace")
                .text_color(palette.muted)
                .child(entry.sequence_label.clone()),
        )
        .child(
            div()
                .w(px(50.0))
                .flex_shrink_0()
                .font_bold()
                .font_family("monospace")
                .text_color(level_color)
                .child(entry.level_label.clone()),
        )
        .child(
            div()
                .min_w(px(1120.0))
                .font_family("monospace")
                .whitespace_nowrap()
                .text_color(palette.text)
                .child(entry.message.clone()),
        )
}

fn log_row_size() -> Size<Pixels> {
    // VirtualList 依赖稳定行高计算滚动范围；日志行固定高度，避免高频刷新造成滚动抖动。
    size(px(LOG_ROW_MIN_WIDTH), px(38.0))
}

fn log_level_color(level: LogLevel, palette: ShellPalette) -> Hsla {
    match level {
        LogLevel::Debug => palette.muted,
        LogLevel::Info => palette.active,
        LogLevel::Warning => palette.warning,
        LogLevel::Error => palette.danger,
        LogLevel::Unknown => palette.text,
    }
}
