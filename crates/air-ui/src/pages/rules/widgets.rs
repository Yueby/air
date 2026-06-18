use std::rc::Rc;

use gpui::{
    Context, Entity, InteractiveElement, IntoElement, ParentElement, Pixels, Size, Styled, div, px,
    size,
};
use gpui_component::StyledExt;
use gpui_component::scroll::{ScrollableElement, ScrollbarAxis};
use gpui_component::{VirtualListScrollHandle, v_virtual_list};

use air_ui::components;
use air_ui::icons::{self, Icon};
use air_ui::shell::{Shell, ShellPalette};

use super::state::{RulesProxyPageViewModel, RuntimeRuleItem};

const RULE_ROW_HEIGHT: f32 = 76.0;
const RULE_ROW_CARD_HEIGHT: f32 = 68.0;
const RULE_ROW_MIN_WIDTH: f32 = 720.0;
pub(super) fn render_rule_list(
    view_model: &RulesProxyPageViewModel,
    scroll_handle: VirtualListScrollHandle,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    if view_model.visible_rule_indices.is_empty() {
        return div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h(px(0.0))
            .overflow_hidden()
            .child(empty_state(
                Icon::SearchX,
                if view_model.total_count == 0 {
                    "内核尚未返回规则"
                } else {
                    "没有匹配的规则"
                },
                palette,
            ));
    }

    let view_model = Rc::new(view_model.clone());
    let row_sizes = Rc::new(vec![rule_row_size(); view_model.visible_rule_indices.len()]);
    let rows = v_virtual_list(
        cx.entity().clone(),
        "rules-virtual-list",
        row_sizes,
        move |_, visible_range, _, cx| {
            let shell = cx.entity().clone();
            visible_range
                .filter_map(|index| view_model.visible_rule(index).cloned())
                .map(|item| render_rule_row(item, palette, shell.clone()))
                .collect::<Vec<_>>()
        },
    )
    .track_scroll(&scroll_handle)
    .into_any_element();

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(
            div()
                .id("rules-scroll-area")
                .relative()
                .size_full()
                .min_h(px(0.0))
                // 运行态规则列表使用常显滚动条，水平留白让行卡片不会贴边或压到滚动条。
                .px_4()
                .pt_3()
                .child(rows)
                .scrollbar(&scroll_handle, ScrollbarAxis::Vertical),
        )
}

pub(super) fn render_rule_row(
    item: RuntimeRuleItem,
    palette: ShellPalette,
    shell: Entity<Shell>,
) -> impl IntoElement {
    let index = item.index;
    let enabled = !item.disabled;

    div()
        .id(format!("runtime-rule-row-{index}"))
        .flex()
        .items_center()
        .gap_3()
        .w_full()
        .px_3()
        .h(px(RULE_ROW_CARD_HEIGHT))
        .rounded_md()
        .border_1()
        .border_color(palette.border)
        .bg(palette.page)
        .hover(move |this| this.bg(palette.hover).border_color(palette.active))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .flex_1()
                .min_w(px(0.0))
                .child(
                    div()
                        .text_sm()
                        .font_bold()
                        .text_color(palette.text)
                        .child(components::emoji_text(item.display_payload().to_string())),
                )
                .child(div().text_xs().text_color(palette.muted).child(
                    components::emoji_text_compact(item.target_line().to_string()),
                )),
        )
        .child(
            div().flex_none().child(
                components::app_switch(
                    format!("runtime-rule-enabled-{index}"),
                    enabled,
                    false,
                    "启用或禁用当前运行态规则，不写回配置文件。",
                )
                .on_click(move |checked, _, cx| {
                    shell.update(cx, |shell, cx| {
                        shell.toggle_runtime_rule(index, *checked);
                        cx.notify();
                    });
                }),
            ),
        )
}

fn rule_row_size() -> Size<Pixels> {
    // 规则页可能有上万条运行态规则；固定行高让虚拟列表能稳定计算可见范围，
    // 避免一次性构建所有行导致页面卡顿。
    size(px(RULE_ROW_MIN_WIDTH), px(RULE_ROW_HEIGHT))
}

fn empty_state(icon: Icon, message: &'static str, palette: ShellPalette) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .min_h(px(260.0))
        .text_sm()
        .font_bold()
        .text_color(palette.muted)
        .child(icons::icon(icon, palette.muted))
        .child(message)
}
