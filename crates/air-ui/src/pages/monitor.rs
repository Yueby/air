use gpui::{Context, Entity, InteractiveElement, IntoElement, ParentElement, Styled, div, px};
use gpui_component::VirtualListScrollHandle;
use gpui_component::animation::{Transition, ease_out_cubic};
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::{ScrollableElement, ScrollbarAxis};

use air_ui::components::{self, foundation};
use air_ui::icons::Icon;
use air_ui::shell::{Shell, ShellPalette};

mod format;
mod state;
mod widgets;

#[cfg(test)]
mod tests;

pub use format::format_bytes;
pub use state::{
    LogEntryView, LogLevel, LogLevelFilter, MAX_LOG_ENTRIES, MAX_METRIC_POINTS, MonitorPageState,
    MonitorViewModel, StreamConnectionState,
};
pub(crate) fn render_log_page(
    state: &MonitorPageState,
    search_input: Entity<InputState>,
    log_scroll_handle: VirtualListScrollHandle,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let view_model = state.view_model();
    let filter_buttons = LogLevelFilter::ALL
        .iter()
        .fold(div().flex().gap_1(), |row, filter| {
            row.child(widgets::filter_chip(
                *filter,
                view_model.active_filter,
                palette,
                cx,
            ))
        });
    let rows = widgets::render_log_rows(&view_model, log_scroll_handle.clone(), palette, cx);
    let copy_text = format::visible_log_text(&view_model);

    let content = div()
        .flex()
        .flex_col()
        .size_full()
        .min_h(px(0.0))
        .bg(palette.background)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .px_5()
                .py_3()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .min_w_0()
                        .child(filter_buttons)
                        .child(div().w(px(300.0)).child(Input::new(&search_input))),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .text_color(palette.muted)
                        .child(format!(
                            "总计 {} 条，匹配 {} 条，显示 {} 条",
                            view_model.log_count,
                            view_model.filtered_count,
                            view_model.rendered_count
                        ))
                        .child(widgets::copy_button(copy_text, palette, cx))
                        .child(widgets::monitor_button(
                            "清空",
                            Icon::Trash2,
                            true,
                            palette,
                            cx,
                            |shell| shell.clear_monitor_logs(),
                        )),
                ),
        )
        .child(div().h(px(1.0)).w_full().bg(palette.border))
        .child(
            div().flex_1().min_h(px(0.0)).p_5().child(
                div()
                    .relative()
                    .size_full()
                    .rounded_md()
                    .border_1()
                    .border_color(palette.border)
                    .bg(palette.surface)
                    .child(
                        div()
                            .id("logs-page-scroll")
                            .relative()
                            .size_full()
                            .child(rows)
                            .scrollbar(&log_scroll_handle, ScrollbarAxis::Both),
                    ),
            ),
        );

    Transition::new(components::animation_duration(
        foundation::FILTER_TRANSITION_MS,
    ))
    .ease(ease_out_cubic)
    .fade(0.0, 1.0)
    .apply(content, "log-page-refresh")
}
