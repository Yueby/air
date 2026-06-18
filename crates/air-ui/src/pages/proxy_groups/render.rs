use std::collections::BTreeMap;

use gpui::{
    Context, Entity, InteractiveElement, IntoElement, ParentElement, ScrollHandle,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::StyledExt;
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement;
use gpui_component::tooltip::Tooltip;

use air_config::model::ProxyGroupKind;
use air_mihomo::groups::{
    ProxyGroupMemberOrigin, ProxyGroupMemberReference, ProxyGroupMemberSource,
    ProxyGroupRuntimeMember, ProxyGroupSelectionState,
};
use air_mihomo::proxies::ProxyDelayStatus;
use air_ui::components;
use air_ui::icons::{self, Icon};
use air_ui::shell::{Shell, ShellPalette};

use super::format::*;
use super::runtime_projection::*;
use super::state::*;
impl GroupDetailView {
    pub(crate) fn from_group(
        group: &air_mihomo::groups::ProxyGroupSettings,
        runtime: Option<&ProxyGroupSelectionState>,
        references: &[ProxyGroupMemberReference],
        delay: GroupDelaySnapshot,
        proxy_protocols: &BTreeMap<String, String>,
        member_delays: &BTreeMap<(String, String), GroupDelaySnapshot>,
    ) -> Self {
        let proxies = references
            .iter()
            .filter(|item| item.origin == ProxyGroupMemberOrigin::Proxies)
            .map(|reference| {
                let delay = member_delays
                    .get(&(group.common.name.clone(), reference.member_name.clone()))
                    .copied()
                    .unwrap_or_default();
                GroupMemberView::from_reference(reference, proxy_protocols, delay)
            })
            .collect::<Vec<_>>();
        let providers = references
            .iter()
            .filter(|item| item.origin == ProxyGroupMemberOrigin::UseProviders)
            .map(|reference| {
                let delay = member_delays
                    .get(&(group.common.name.clone(), reference.member_name.clone()))
                    .copied()
                    .unwrap_or_default();
                GroupMemberView::from_reference(reference, proxy_protocols, delay)
            })
            .collect::<Vec<_>>();
        let runtime_members = runtime
            .map(|state| {
                state
                    .members
                    .iter()
                    .map(|member| {
                        GroupMemberView::from_runtime(
                            &group.common.name,
                            member,
                            proxy_protocols,
                            member_delays,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Self {
            name: group.common.name.clone(),
            kind: proxy_group_type_display_label(group.common.kind.as_str()).to_string(),
            current: runtime.and_then(|state| state.selected.clone()),
            selectable: matches!(group.common.kind, ProxyGroupKind::Select),
            automatic: is_automatic_group(&group.common.kind),
            proxies,
            providers,
            runtime_members,
            health_url: group.health_check.url.clone(),
            health_interval: group.health_check.interval,
            strategy: group.balancing.strategy.clone(),
            delay_status: delay.status,
            delay_ms: delay.delay_ms,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupMemberView {
    pub name: String,
    pub protocol: String,
    pub source: ProxyGroupMemberSource,
    pub selected: bool,
    pub configured: bool,
    pub delay_status: ProxyDelayStatus,
    pub delay_ms: Option<u64>,
    pub(crate) historical_delay: GroupDelaySnapshot,
}

impl GroupMemberView {
    pub(crate) fn from_reference(
        reference: &ProxyGroupMemberReference,
        proxy_protocols: &BTreeMap<String, String>,
        delay: GroupDelaySnapshot,
    ) -> Self {
        Self {
            name: reference.member_name.clone(),
            protocol: member_protocol(&reference.member_name, &reference.source, proxy_protocols),
            source: reference.source.clone(),
            selected: false,
            configured: true,
            delay_status: delay.status,
            delay_ms: delay.delay_ms,
            historical_delay: GroupDelaySnapshot::default(),
        }
    }

    pub(crate) fn from_runtime(
        group_name: &str,
        member: &ProxyGroupRuntimeMember,
        proxy_protocols: &BTreeMap<String, String>,
        member_delays: &BTreeMap<(String, String), GroupDelaySnapshot>,
    ) -> Self {
        let delay = member_delays
            .get(&(group_name.to_string(), member.name.clone()))
            .copied()
            .unwrap_or_default();
        let historical_delay = history_delay_snapshot(&member.history).unwrap_or_default();
        Self {
            name: member.name.clone(),
            protocol: member
                .protocol
                .as_deref()
                .map(proxy_type_display_label)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| member_protocol(&member.name, &member.source, proxy_protocols)),
            source: member.source.clone(),
            selected: member.selected,
            configured: member.configured,
            delay_status: delay.status,
            delay_ms: delay.delay_ms,
            historical_delay,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupNotice {
    pub level: GroupNoticeLevel,
    pub message: String,
}

impl GroupNotice {
    pub(crate) fn success(message: impl Into<String>) -> Self {
        Self {
            level: GroupNoticeLevel::Success,
            message: message.into(),
        }
    }

    pub(crate) fn warning(message: impl Into<String>) -> Self {
        Self {
            level: GroupNoticeLevel::Warning,
            message: message.into(),
        }
    }

    pub(crate) fn error(message: impl Into<String>) -> Self {
        Self {
            level: GroupNoticeLevel::Error,
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupNoticeLevel {
    Success,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct GroupDelaySnapshot {
    pub(crate) status: ProxyDelayStatus,
    pub(crate) delay_ms: Option<u64>,
}

pub(crate) fn render_groups_page(
    state: &GroupPageState,
    inputs: GroupPageInputs,
    palette: ShellPalette,
    page_width: f32,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let view_model = state.view_model();

    div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .h_full()
        .w_full()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(render_proxy_toolbar(
            &view_model,
            inputs.search.clone(),
            palette,
            cx,
        ))
        .child(render_proxy_split_content(
            &view_model,
            &inputs,
            palette,
            page_width,
            cx,
        ))
        .child(render_group_modal(&view_model, inputs, palette, cx))
}

#[derive(Clone)]
pub(crate) struct GroupPageInputs {
    pub search: Entity<InputState>,
    pub group_scroll_handle: ScrollHandle,
    pub member_scroll_handle: ScrollHandle,
    pub proxies: Entity<InputState>,
    pub providers: Entity<InputState>,
    pub filter: Entity<InputState>,
    pub exclude_filter: Entity<InputState>,
}

fn render_proxy_toolbar(
    view_model: &GroupPageViewModel,
    search: Entity<InputState>,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .w_full()
        .h(px(52.0))
        .px_4()
        .border_b_1()
        .border_color(palette.border)
        .bg(palette.background)
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .child(Input::new(&search).w_full()),
        )
        .child(proxy_toolbar_icon_button(
            "proxy-delay-sort-toggle",
            Icon::ArrowDownWideNarrow,
            "按延迟排序",
            view_model.sort_members_by_delay,
            palette,
            cx,
            |shell, _, cx| {
                shell.toggle_group_delay_sort();
                cx.notify();
            },
        ))
        .child(proxy_toolbar_icon_button(
            "proxy-selected-group-delay",
            Icon::Gauge,
            "测速当前分组",
            false,
            palette,
            cx,
            |shell, window, cx| {
                shell.test_selected_group_delay(window, cx);
            },
        ))
}

fn render_proxy_split_content(
    view_model: &GroupPageViewModel,
    inputs: &GroupPageInputs,
    palette: ShellPalette,
    page_width: f32,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    if view_model.items.is_empty() {
        return div()
            .flex()
            .items_center()
            .justify_center()
            .flex_1()
            .min_h(px(0.0))
            .bg(palette.background)
            .text_sm()
            .text_color(palette.muted)
            .child("内核未运行，暂无代理数据");
    }

    let selected = selected_group_item(view_model);
    div()
        .flex()
        .flex_1()
        .min_h(px(0.0))
        .w_full()
        .overflow_hidden()
        .bg(palette.background)
        .child(render_group_column(
            view_model,
            &inputs.group_scroll_handle,
            palette,
            cx,
        ))
        .child(render_member_column(
            selected,
            view_model,
            &inputs.member_scroll_handle,
            palette,
            page_width,
            cx,
        ))
}

fn render_group_column(
    view_model: &GroupPageViewModel,
    scroll_handle: &ScrollHandle,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let list = view_model.items.iter().fold(
        div()
            .flex()
            .flex_col()
            .gap_2()
            .w_full()
            .pl_2()
            .pr_4()
            .py_2(),
        |list, item| list.child(render_group_card(item, item.expanded, palette, cx)),
    );

    div()
        .id("proxy-group-scroll")
        .flex()
        .flex_col()
        .w(px(PROXY_GROUP_COLUMN_WIDTH))
        .min_w(px(PROXY_GROUP_COLUMN_WIDTH))
        .h_full()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(
            div()
                .id("proxy-group-scroll-area")
                .flex()
                .flex_col()
                .size_full()
                .min_h(px(0.0))
                .track_scroll(scroll_handle)
                .overflow_y_scroll()
                .child(list),
        )
        .vertical_scrollbar(scroll_handle)
}

fn render_member_column(
    selected: Option<&GroupListItem>,
    view_model: &GroupPageViewModel,
    scroll_handle: &ScrollHandle,
    palette: ShellPalette,
    page_width: f32,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let content_width = (page_width - PROXY_GROUP_COLUMN_WIDTH).max(PROXY_MEMBER_CARD_MIN_WIDTH);
    let mut scroll_area = div()
        .id("proxy-member-scroll-area")
        .flex()
        .flex_col()
        .size_full()
        .min_h(px(0.0))
        .track_scroll(scroll_handle)
        .overflow_y_scroll();

    if let Some(item) = selected {
        scroll_area = scroll_area.child(div().pl_2().pr_4().py_2().child(render_group_members(
            item,
            content_width,
            palette,
            cx,
        )));
    } else {
        scroll_area = scroll_area.child(
            div().pl_2().pr_4().py_2().child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(180.0))
                    .text_sm()
                    .text_color(palette.muted)
                    .child(if view_model.search_query.trim().is_empty() {
                        "请选择代理组"
                    } else {
                        "没有匹配的节点"
                    }),
            ),
        );
    }

    div()
        .id("proxy-member-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .h_full()
        .min_h(px(0.0))
        .border_l_1()
        .border_color(palette.border)
        .overflow_hidden()
        .child(scroll_area)
        .vertical_scrollbar(scroll_handle)
}

fn selected_group_item(view_model: &GroupPageViewModel) -> Option<&GroupListItem> {
    view_model
        .items
        .iter()
        .find(|item| item.expanded)
        .or_else(|| view_model.items.first())
}

pub(crate) fn item_matches_search(item: &GroupListItem, search: &str) -> bool {
    if search.is_empty() {
        return true;
    }

    item.name.to_ascii_lowercase().contains(search)
        || item.kind.to_ascii_lowercase().contains(search)
        || item.current.to_ascii_lowercase().contains(search)
        || item.total_member_count.to_string().contains(search)
        || item
            .members
            .iter()
            .any(|member| member_matches_search(member, search))
}

fn render_group_card(
    item: &GroupListItem,
    selected: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let name = item.name.clone();
    let group_id = element_id_fragment(&item.name);
    div()
        .id(format!("group-card-{group_id}"))
        .flex()
        .flex_col()
        .gap_2()
        .w(px(PROXY_CARD_WIDTH))
        .h(px(76.0))
        .p_2()
        .rounded_md()
        .border_1()
        .border_color(if selected {
            palette.active
        } else {
            palette.border
        })
        .bg(if selected {
            palette.active.alpha(PROXY_SELECTED_CARD_ALPHA)
        } else {
            palette.surface
        })
        .cursor_pointer()
        .hover(move |this| this.bg(palette.hover))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .min_w(px(0.0))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .truncate()
                        .text_sm()
                        .font_bold()
                        .text_color(palette.text)
                        .child(components::emoji_text(item.name.clone())),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(24.0))
                        .rounded(px(12.0))
                        .border_1()
                        .border_color(palette.active)
                        .bg(palette.active.alpha(0.12))
                        .text_xs()
                        .font_bold()
                        .text_color(palette.active)
                        .child(item.total_member_count.to_string()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .text_xs()
                .text_color(palette.muted)
                .child(
                    div()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .truncate()
                        .child(item.kind.clone()),
                )
                .child(
                    div()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .truncate()
                        .child(components::emoji_text(item.current.clone())),
                ),
        )
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.select_group(name.clone());
            cx.notify();
        }))
}
fn proxy_toolbar_icon_button(
    id: &'static str,
    icon: Icon,
    tooltip: &'static str,
    active: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
    action: impl Fn(&mut Shell, &mut Window, &mut Context<Shell>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(32.0))
        .rounded_md()
        .border_1()
        .border_color(if active {
            palette.active
        } else {
            palette.border
        })
        .bg(if active {
            palette.subtle
        } else {
            palette.surface
        })
        .cursor_pointer()
        .hover(move |this| this.bg(palette.hover))
        .tooltip(move |window, cx| Tooltip::new(tooltip).build(window, cx))
        .child(icons::icon(icon, palette.text))
        .on_click(cx.listener(move |shell, _, window, cx| {
            action(shell, window, cx);
            cx.stop_propagation();
        }))
}

fn render_group_members(
    item: &GroupListItem,
    page_width: f32,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    if item.members.is_empty() {
        let empty = if item.filter_query.trim().is_empty() {
            "暂无可展示节点"
        } else {
            "没有匹配的节点"
        };
        return div()
            .flex()
            .items_center()
            .justify_center()
            .h(px(76.0))
            .rounded_md()
            .bg(palette.surface)
            .text_sm()
            .text_color(palette.muted)
            .child(empty);
    }

    let columns = group_member_card_columns(page_width);
    item.members.iter().fold(
        div().grid().grid_cols(columns).gap_2().w_full(),
        |grid, member| {
            grid.child(render_member_card(
                member,
                item.name.clone(),
                item.selectable,
                palette,
                cx,
            ))
        },
    )
}

fn group_member_card_columns(page_width: f32) -> u16 {
    const PAGE_INLINE_PADDING: f32 = 40.0;
    const GROUP_BLOCK_INLINE_PADDING: f32 = 32.0;

    // 成员卡片列数只由可用页面宽度推导，grid 再平分剩余空间；
    let available_width = (page_width - PAGE_INLINE_PADDING - GROUP_BLOCK_INLINE_PADDING)
        .max(PROXY_MEMBER_CARD_MIN_WIDTH);
    (((available_width + PROXY_CARD_GAP) / (PROXY_MEMBER_CARD_MIN_WIDTH + PROXY_CARD_GAP)).floor()
        as u16)
        .max(1)
}

fn render_member_card(
    member: &GroupMemberView,
    group_name: String,
    _selectable: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let select_group = group_name.clone();
    let select_member = member.name.clone();
    let card_id = format!(
        "group-member-{}-{}",
        element_id_fragment(&group_name),
        element_id_fragment(&member.name)
    );
    div()
        .id(card_id)
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .w_full()
        .min_w(px(0.0))
        .h(px(72.0))
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(if member.selected {
            palette.active
        } else {
            palette.border
        })
        .bg(if member.selected {
            palette.active.alpha(PROXY_SELECTED_CARD_ALPHA)
        } else {
            palette.page
        })
        .cursor_pointer()
        .hover(move |this| this.bg(palette.hover))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .flex_1()
                .min_w(px(0.0))
                .child(
                    div()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .truncate()
                        .text_sm()
                        .font_bold()
                        .text_color(palette.text)
                        .child(components::emoji_text(member.name.clone())),
                )
                .child(
                    div()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .truncate()
                        .text_xs()
                        .text_color(palette.muted)
                        .child(member.protocol.clone()),
                ),
        )
        .child(render_member_delay(member, group_name.clone(), palette, cx))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.select_group_member(select_group.clone(), select_member.clone(), window, cx);
            cx.stop_propagation();
            cx.notify();
        }))
}

fn render_member_delay(
    member: &GroupMemberView,
    group_name: String,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let display_delay = displayed_member_delay(member);
    let member_name = member.name.clone();
    let delay_id = format!(
        "group-member-delay-{}-{}",
        element_id_fragment(&group_name),
        element_id_fragment(&member_name)
    );
    div()
        .id(delay_id)
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .min_w(px(54.0))
        .h(px(28.0))
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .text_xs()
        .font_bold()
        .text_color(delay_color(
            display_delay.status,
            display_delay.delay_ms,
            palette,
        ))
        .hover(move |this| this.bg(palette.subtle))
        .child(delay_label(display_delay.status, display_delay.delay_ms))
        .on_click(cx.listener(move |shell, _, window, cx| {
            shell.test_group_member_delay(group_name.clone(), member_name.clone(), window, cx);
            cx.stop_propagation();
            cx.notify();
        }))
}

fn render_group_modal(
    view_model: &GroupPageViewModel,
    inputs: GroupPageInputs,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    if view_model.modal == GroupModalState::None {
        return div();
    }

    div()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .flex()
        .items_center()
        .justify_center()
        .p_6()
        .bg(palette.background)
        .child(render_group_form(&view_model.form, inputs, palette, cx))
}

fn render_group_form(
    form: &GroupFormState,
    inputs: GroupPageInputs,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .w(px(560.0))
        .max_w_full()
        .p_4()
        .rounded_md()
        .border_1()
        .border_color(palette.border)
        .bg(palette.page)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(section_title(Icon::Pencil, "Edit group members", palette))
                .child(
                    div()
                        .text_xs()
                        .text_color(palette.muted)
                        .child(format!("{} / {}", form.group_name, form.kind)),
                ),
        )
        .child(form_input(
            "proxies, one node or policy per line",
            inputs.proxies,
            palette,
        ))
        .child(form_input(
            "use providers, one provider per line",
            inputs.providers,
            palette,
        ))
        .child(
            div()
                .flex()
                .gap_2()
                .child(form_input("filter", inputs.filter, palette))
                .child(form_input("exclude-filter", inputs.exclude_filter, palette)),
        )
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(group_action_button(
                    "取消",
                    Icon::X,
                    true,
                    palette,
                    cx,
                    |shell, _, _| shell.close_group_modal(),
                ))
                .child(group_action_button(
                    "保存",
                    Icon::Save,
                    true,
                    palette,
                    cx,
                    |shell, window, cx| shell.save_group_form(window, cx),
                )),
        )
}

fn group_action_button(
    label: &'static str,
    icon: Icon,
    enabled: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
    action: impl Fn(&mut Shell, &mut Window, &mut Context<Shell>) + 'static,
) -> impl IntoElement {
    div()
        .id(format!("group-action-{label}"))
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
        .on_click(cx.listener(move |shell, _, window, cx| {
            if enabled {
                action(shell, window, cx);
                cx.notify();
            }
        }))
}

fn form_input(
    label: &'static str,
    input: Entity<InputState>,
    palette: ShellPalette,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .gap_1()
        .child(div().text_xs().text_color(palette.muted).child(label))
        .child(Input::new(&input))
}

fn section_title(icon: Icon, title: &'static str, palette: ShellPalette) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .font_bold()
        .text_color(palette.text)
        .child(icons::icon(icon, palette.active))
        .child(title)
}
