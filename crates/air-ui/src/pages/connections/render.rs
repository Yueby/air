use std::time::SystemTime;

use gpui::{
    AnyElement, Context, Div, Entity, InteractiveElement, IntoElement, MouseButton, ObjectFit,
    ParentElement, ScrollHandle, Stateful, StatefulInteractiveElement, Styled, StyledImage, div,
    img, px,
};
use gpui_component::animation::{Transition, ease_out_cubic};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement;
use gpui_component::select::{Select, SelectState};
use gpui_component::tag::Tag;
use gpui_component::{Sizable, StyledExt};

use air_platform::process_icon;
use air_ui::components::{self, foundation};
use air_ui::icons::{self, Icon};
use air_ui::shell::{Shell, ShellPalette};

use super::format::*;
use super::render_controls::*;
use super::state::*;
#[derive(Clone, Debug)]
pub struct ConnectionListItem {
    pub id: String,
    pub app_name: String,
    pub process_path: String,
    pub target: String,
    pub connection_type: String,
    pub chain: String,
    pub primary_chain: String,
    pub provider_chain: String,
    pub rule: String,
    pub inbound: String,
    pub dns_mode: String,
    pub endpoint_line: String,
    pub remote: String,
    pub upload_speed: u64,
    pub download_speed: u64,
    pub upload_total: u64,
    pub download_total: u64,
    pub upload_speed_label: String,
    pub download_speed_label: String,
    pub upload_total_label: String,
    pub download_total_label: String,
    pub relative_start: String,
    pub started_at_epoch: Option<i64>,
    pub status: ConnectionStatusFilter,
}

impl ConnectionListItem {
    pub(super) fn from_entry(entry: &ConnectionEntry, now: SystemTime) -> Self {
        Self {
            id: entry.id.clone(),
            app_name: entry.app_name.clone(),
            process_path: entry.process_path.clone(),
            target: entry.target.clone(),
            connection_type: entry.connection_type.clone(),
            chain: entry.chain_label(),
            primary_chain: entry.primary_chain_label(),
            provider_chain: entry.provider_chain_label(),
            rule: entry.rule_label(),
            inbound: entry.inbound_name.clone(),
            dns_mode: entry.dns_mode.clone(),
            endpoint_line: entry.endpoint_line(),
            remote: entry.remote_label(),
            upload_speed: entry.upload_speed,
            download_speed: entry.download_speed,
            upload_total: entry.upload_total,
            download_total: entry.download_total,
            upload_speed_label: format_bytes_per_second(entry.upload_speed),
            download_speed_label: format_bytes_per_second(entry.download_speed),
            upload_total_label: format_bytes(entry.upload_total),
            download_total_label: format_bytes(entry.download_total),
            relative_start: relative_time_label(&entry.start, now),
            started_at_epoch: entry.started_at_epoch,
            status: entry.status,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionNotice {
    pub level: ConnectionNoticeLevel,
    pub message: String,
}

impl ConnectionNotice {
    pub(super) fn error(message: impl Into<String>) -> Self {
        Self {
            level: ConnectionNoticeLevel::Error,
            message: message.into(),
        }
    }

    pub(super) fn info(message: impl Into<String>) -> Self {
        Self {
            level: ConnectionNoticeLevel::Info,
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionNoticeLevel {
    Info,
    Error,
}

#[derive(Clone)]
pub(crate) struct ConnectionsPageInputs {
    pub search: Entity<InputState>,
    pub status: Entity<SelectState<Vec<&'static str>>>,
    pub sort_field: Entity<SelectState<Vec<&'static str>>>,
}

pub(crate) fn render_connections_page(
    state: &ConnectionsPageState,
    inputs: ConnectionsPageInputs,
    detail_editor: Entity<InputState>,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let view_model = state.view_model();

    div()
        .relative()
        .flex()
        .flex_col()
        .size_full()
        .flex_1()
        .min_w_0()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(render_toolbar(&view_model, inputs, palette, cx))
        .child(render_cards_area(
            &view_model,
            &state.card_scroll_handle,
            palette,
            cx,
        ))
        .child(render_pending_close(&view_model, palette, cx))
        .child(render_detail_modal(&view_model, detail_editor, palette, cx))
}

fn render_toolbar(
    view_model: &ConnectionsPageViewModel,
    inputs: ConnectionsPageInputs,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    div()
        .id("connections-toolbar")
        .flex()
        .flex_col()
        .gap_2()
        .flex_shrink_0()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(palette.border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .min_w_0()
                .child(status_filter_group(view_model.status, palette, cx))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(Input::new(&inputs.search).w_full()),
                )
                .child(
                    div()
                        .w(px(128.0))
                        .min_w(px(118.0))
                        .child(Select::new(&inputs.sort_field).w_full()),
                )
                .child(sort_direction_button(
                    view_model.sort.direction,
                    palette,
                    cx,
                ))
                .child(toolbar_icon_button(
                    "connection-close-filtered".to_string(),
                    Icon::CircleX,
                    "关闭全部连接",
                    view_model.closable_filtered_count > 0,
                    palette.danger,
                    palette,
                    cx,
                    |shell, window, cx| shell.request_close_all_connections(window, cx),
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .flex_wrap()
                .min_w(px(0.0))
                .child(overview_metric(
                    Icon::Activity,
                    format!(
                        "{} {} / {} {}",
                        ConnectionStatusFilter::Active.label(),
                        view_model.active_count,
                        ConnectionStatusFilter::Closed.label(),
                        view_model.closed_count
                    ),
                    palette,
                ))
                .child(overview_metric(
                    Icon::Upload,
                    format_bytes(view_model.total_upload),
                    palette,
                ))
                .child(overview_metric(
                    Icon::Download,
                    format_bytes(view_model.total_download),
                    palette,
                ))
                .child(overview_metric(
                    Icon::ArrowUpNarrowWide,
                    format_bytes_per_second(view_model.total_upload_speed),
                    palette,
                ))
                .child(overview_metric(
                    Icon::ArrowDownWideNarrow,
                    format_bytes_per_second(view_model.total_download_speed),
                    palette,
                )),
        )
}

fn status_filter_group(
    current: ConnectionStatusFilter,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    div()
        .id("connection-status-group")
        .flex()
        .items_center()
        .gap_1()
        .rounded_md()
        .border_1()
        .border_color(palette.border)
        .bg(palette.subtle)
        .p(px(2.0))
        .child(status_filter_button(
            ConnectionStatusFilter::Active,
            current == ConnectionStatusFilter::Active,
            palette,
            cx,
        ))
        .child(status_filter_button(
            ConnectionStatusFilter::Closed,
            current == ConnectionStatusFilter::Closed,
            palette,
            cx,
        ))
}

fn status_filter_button(
    status: ConnectionStatusFilter,
    selected: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let id = match status {
        ConnectionStatusFilter::Active => "connection-status-active",
        ConnectionStatusFilter::Closed => "connection-status-closed",
    };

    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(28.0))
        .px_3()
        .rounded_md()
        .cursor_pointer()
        .text_xs()
        .font_bold()
        .bg(if selected {
            palette.active
        } else {
            palette.subtle
        })
        .text_color(if selected {
            palette.surface
        } else {
            palette.text
        })
        .hover(move |this| {
            if selected {
                this
            } else {
                this.bg(palette.hover)
            }
        })
        .child(status.label())
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.set_connection_status_filter(status);
            cx.notify();
        }))
}

fn overview_metric(
    icon: Icon,
    label: impl Into<String>,
    palette: ShellPalette,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .h(px(24.0))
        .px_2()
        .rounded_md()
        .bg(palette.subtle)
        .text_xs()
        .font_bold()
        .text_color(palette.text)
        .child(icons::icon(icon, palette.muted))
        .child(label.into())
}

fn render_cards_area(
    view_model: &ConnectionsPageViewModel,
    scroll_handle: &ScrollHandle,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let content = if view_model.items.is_empty() {
        empty_state(view_model.status, palette).into_any_element()
    } else {
        let cards = view_model
            .items
            .iter()
            .fold(div().flex().flex_col().gap_2().w_full(), |column, item| {
                column.child(connection_card(item, palette, cx))
            });

        // 筛选和排序变化时只做轻量淡入，卡片 hover 仍由自身背景变化承担，
        // 避免运行态连接频繁刷新时出现影响可读性的位移动画。
        Transition::new(components::animation_duration(
            foundation::FILTER_TRANSITION_MS,
        ))
        .ease(ease_out_cubic)
        .fade(0.0, 1.0)
        .apply(cards, format!("connections-cards-{:?}", view_model.sort))
        .into_any_element()
    };

    div()
        .id("connections-card-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(
            div()
                .id("connections-card-scroll-area")
                .flex()
                .flex_col()
                .size_full()
                .min_h(px(0.0))
                .track_scroll(scroll_handle)
                .overflow_y_scroll()
                .child(div().px_4().py_3().child(content)),
        )
        .vertical_scrollbar(scroll_handle)
}

fn connection_card(
    item: &ConnectionListItem,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> Stateful<Div> {
    let close_id = item.id.clone();
    let detail_id = item.id.clone();
    let is_active = item.status == ConnectionStatusFilter::Active;

    div()
        .id(format!("connection-card-{}", sanitize_id(&item.id)))
        .flex()
        .items_stretch()
        .gap_2()
        .w_full()
        .min_h(px(68.0))
        .px_3()
        .py_3()
        .rounded_md()
        .border_1()
        .border_color(palette.border)
        .bg(if is_active {
            palette.page
        } else {
            palette.subtle
        })
        .cursor_pointer()
        .hover(move |this| this.bg(palette.hover).border_color(palette.active))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.open_connection_detail(detail_id.clone(), window, cx);
            cx.notify();
        }))
        .child(process_icon_view(item, palette))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .flex_1()
                .min_w(px(0.0))
                .child(
                    div()
                        .flex()
                        .flex_nowrap()
                        .items_center()
                        .gap_2()
                        .min_w(px(0.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .min_w(px(0.0))
                                .flex_1()
                                .child(app_name_text(item, palette))
                                .child(icons::sized_icon(
                                    Icon::ChevronRight,
                                    palette.muted,
                                    px(14.0),
                                ))
                                .child(target_text(item, palette)),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_xs()
                                .text_color(palette.muted)
                                .child(item.relative_start.clone()),
                        ),
                )
                .child(connection_tags(item, palette)),
        )
        .child(if is_active {
            toolbar_icon_button(
                format!("connection-close-{}", sanitize_id(&item.id)),
                Icon::X,
                "关闭连接",
                true,
                palette.danger,
                palette,
                cx,
                move |shell, window, cx| {
                    shell.request_close_connection(close_id.clone(), window, cx)
                },
            )
            .into_any_element()
        } else {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(px(28.0))
                .rounded_md()
                .text_color(palette.muted)
                .child(icons::icon(Icon::Check, palette.muted))
                .into_any_element()
        })
}

fn process_icon_view(item: &ConnectionListItem, palette: ShellPalette) -> AnyElement {
    let fallback =
        || icons::sized_icon(app_icon(&item.app_name), palette.active, px(42.0)).into_any_element();
    let child = process_icon::cached_icon_for_process_path(&item.process_path)
        .map(|icon| {
            img(icon.png_path)
                .size(px(42.0))
                .object_fit(ObjectFit::Contain)
                .into_any_element()
        })
        .unwrap_or_else(fallback);

    div()
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .w(px(52.0))
        .self_stretch()
        .child(child)
        .into_any_element()
}

fn connection_tags(item: &ConnectionListItem, palette: ShellPalette) -> impl IntoElement {
    let mut tags: Vec<(&'static str, TagContent, TagTone)> = vec![
        (
            "type",
            TagContent::Text(item.connection_type.clone()),
            TagTone::Secondary,
        ),
        (
            "chain",
            TagContent::EmojiText(item.primary_chain.clone()),
            TagTone::Secondary,
        ),
        (
            "traffic-total",
            TagContent::Transfer {
                upload: item.upload_total_label.clone(),
                download: item.download_total_label.clone(),
            },
            TagTone::Secondary,
        ),
        (
            "traffic-speed",
            TagContent::Transfer {
                upload: item.upload_speed_label.clone(),
                download: item.download_speed_label.clone(),
            },
            TagTone::Secondary,
        ),
    ];
    if !item.rule.is_empty() && item.rule != "-" {
        tags.push((
            "rule",
            TagContent::Text(item.rule.clone()),
            TagTone::Secondary,
        ));
    }

    tags.into_iter()
        .filter(|(_, content, _)| content.visible())
        .fold(
            div()
                .flex()
                .items_center()
                .gap_1()
                .flex_wrap()
                .min_w(px(0.0)),
            |row, (id, content, tone)| row.child(inline_tag(id, content, tone, palette)),
        )
}

fn app_name_text(item: &ConnectionListItem, palette: ShellPalette) -> impl IntoElement {
    div()
        .flex_none()
        .text_sm()
        .font_bold()
        .text_color(palette.text)
        .whitespace_nowrap()
        .child(item.app_name.clone())
}

fn target_text(item: &ConnectionListItem, palette: ShellPalette) -> impl IntoElement {
    div()
        .min_w(px(0.0))
        .flex_1()
        .text_sm()
        .font_bold()
        .text_color(palette.text)
        .whitespace_nowrap()
        .truncate()
        .child(item.target.clone())
}

#[derive(Clone, Copy)]
enum TagTone {
    Secondary,
}

enum TagContent {
    Text(String),
    EmojiText(String),
    Transfer { upload: String, download: String },
}

impl TagContent {
    fn visible(&self) -> bool {
        match self {
            Self::Text(value) | Self::EmojiText(value) => {
                !value.trim().is_empty() && value.trim() != "-"
            }
            Self::Transfer { upload, download } => {
                !upload.trim().is_empty() || !download.trim().is_empty()
            }
        }
    }

    fn id_label(&self) -> String {
        match self {
            Self::Text(value) | Self::EmojiText(value) => value.clone(),
            Self::Transfer { upload, download } => format!("{upload}-{download}"),
        }
    }
}

fn inline_tag(
    id: &'static str,
    content: TagContent,
    tone: TagTone,
    palette: ShellPalette,
) -> impl IntoElement {
    let id_label = content.id_label();
    let content = match tone {
        TagTone::Secondary => Tag::secondary()
            .small()
            .rounded(px(4.0))
            .whitespace_normal()
            .text_size(px(10.0))
            .child(tag_content(content, palette))
            .into_any_element(),
    };

    div()
        .id(format!("connection-tag-{id}-{}", sanitize_id(&id_label)))
        .max_w(px(260.0))
        .child(content)
}

fn tag_content(content: TagContent, palette: ShellPalette) -> AnyElement {
    match content {
        TagContent::Text(value) => div().child(value).into_any_element(),
        TagContent::EmojiText(value) => div()
            .h(px(12.0))
            .flex()
            .items_center()
            .overflow_hidden()
            .child(components::emoji_text_compact(value))
            .into_any_element(),
        TagContent::Transfer { upload, download } => div()
            .flex()
            .items_center()
            .gap_1()
            .child(icons::sized_icon(Icon::ArrowDown, palette.active, px(12.0)))
            .child(download)
            .child(icons::sized_icon(Icon::ArrowUp, palette.warning, px(12.0)))
            .child(upload)
            .into_any_element(),
    }
}

fn empty_state(status: ConnectionStatusFilter, palette: ShellPalette) -> impl IntoElement {
    div()
        .id("connections-empty")
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .min_h(px(260.0))
        .text_sm()
        .text_color(palette.muted)
        .child(icons::icon(Icon::SearchX, palette.muted))
        .child(match status {
            ConnectionStatusFilter::Active => "没有匹配的活动连接",
            ConnectionStatusFilter::Closed => "没有匹配的已关闭连接",
        })
}

fn render_pending_close(
    view_model: &ConnectionsPageViewModel,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let Some(pending) = view_model.pending_close.as_ref() else {
        return div().into_any_element();
    };
    let message = match pending {
        PendingClose::One { id, target } => {
            format!("确认关闭连接 {} ({target})？", short_id(id))
        }
        PendingClose::Filtered {
            count,
            status_label,
            query_label,
            ..
        } => format!("确认关闭 {status_label} 中符合“{query_label}”的 {count} 个活动连接？"),
    };
    let close_count = match pending {
        PendingClose::One { .. } => 1,
        PendingClose::Filtered { count, .. } => *count,
    };

    Transition::new(components::animation_duration(
        foundation::OVERLAY_TRANSITION_MS,
    ))
    .ease(ease_out_cubic)
    .fade(0.0, 1.0)
    .apply(
        div()
            .absolute()
            .right_4()
            .bottom_4()
            .flex()
            .items_center()
            .gap_3()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(palette.warning)
            .bg(palette.surface)
            .child(icons::icon(Icon::AlertTriangle, palette.warning))
            .child(
                div()
                    .max_w(px(420.0))
                    .text_sm()
                    .font_bold()
                    .text_color(palette.text)
                    .child(message),
            )
            .child(page_button(
                "connection-close-cancel",
                "取消",
                Icon::Undo2,
                true,
                palette,
                cx,
                |shell| shell.cancel_pending_connection_close(),
            ))
            .child(confirm_close_button(close_count, palette, cx)),
        "connections-close-confirm",
    )
    .into_any_element()
}

fn render_detail_modal(
    view_model: &ConnectionsPageViewModel,
    editor: Entity<InputState>,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> AnyElement {
    let Some(detail) = view_model.detail.as_ref() else {
        return div().into_any_element();
    };

    div()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .bg(palette.background.alpha(0.96))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .w_full()
                .max_w(px(860.0))
                .max_h(px(620.0))
                .h_full()
                .p_4()
                .rounded_md()
                .border_1()
                .border_color(palette.border)
                .bg(palette.page)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .min_w_0()
                                .child(icons::icon(Icon::ScrollText, palette.active))
                                .child(
                                    div()
                                        .min_w_0()
                                        .text_sm()
                                        .font_bold()
                                        .text_color(palette.text)
                                        .child(detail.title.clone()),
                                ),
                        )
                        .child(close_detail_button(palette, cx)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h(px(0.0))
                        .rounded_md()
                        .border_1()
                        .border_color(components::code_editor_border(palette))
                        .bg(components::code_editor_background(palette))
                        .p_2()
                        .child(
                            Input::new(&editor)
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .font_family("monospace")
                                .size_full(),
                        ),
                ),
        )
        .into_any_element()
}

fn close_detail_button(palette: ShellPalette, cx: &mut Context<Shell>) -> impl IntoElement {
    div()
        .id("connection-detail-close")
        .flex()
        .items_center()
        .justify_center()
        .size(px(32.0))
        .rounded_md()
        .cursor_pointer()
        .text_color(palette.text)
        .hover(move |this| this.bg(palette.hover))
        .child(icons::icon(Icon::X, palette.text))
        .on_click(cx.listener(|shell, _, _, cx| {
            shell.close_connection_detail();
            cx.notify();
        }))
}

fn confirm_close_button(
    close_count: usize,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let shell = cx.entity().clone();
    Button::new("connection-close-confirm")
        .small()
        .ghost()
        .child(icons::icon(Icon::Check, palette.text))
        .child("确认")
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            let _ = shell.update(cx, |shell, cx| {
                shell.confirm_pending_connection_close();
                cx.notify();
            });
            shell.update(cx, |shell, cx| {
                shell.notify_connection_success(
                    format!("已提交关闭 {close_count} 个连接的命令"),
                    window,
                    cx,
                );
            });
        })
}

fn sort_direction_button(
    direction: SortDirection,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    toolbar_icon_button(
        "connection-sort-direction".to_string(),
        direction.icon(),
        direction.label(),
        true,
        palette.text,
        palette,
        cx,
        |shell, _, _| shell.toggle_connection_sort_direction(),
    )
}
