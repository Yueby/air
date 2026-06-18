use gpui::{Context, Entity, InteractiveElement, IntoElement, ParentElement, Styled, div, px};
use gpui_component::StyledExt;
use gpui_component::VirtualListScrollHandle;
use gpui_component::animation::{Transition, ease_out_cubic};
use gpui_component::input::{Input, InputState};

use air_ui::components::{self, foundation};
use air_ui::shell::{Shell, ShellPalette};

mod mapping;
mod state;
mod widgets;

#[cfg(test)]
mod tests;

pub(crate) use state::{RulesProxyPageState, RulesProxyPageViewModel};
#[derive(Clone)]
pub(crate) struct RulesProxyPageInputs {
    pub(crate) search: Entity<InputState>,
}

pub(crate) fn render_rules_proxy_page(
    state: &RulesProxyPageState,
    inputs: RulesProxyPageInputs,
    scroll_handle: VirtualListScrollHandle,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let view_model = state.view_model();
    let content = div()
        .relative()
        .flex()
        .flex_col()
        .size_full()
        .flex_1()
        .min_w_0()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(render_toolbar(&view_model, inputs, palette))
        .child(widgets::render_rule_list(
            &view_model,
            scroll_handle,
            palette,
            cx,
        ));

    Transition::new(components::animation_duration(
        foundation::PAGE_TRANSITION_MS,
    ))
    .ease(ease_out_cubic)
    .fade(0.0, 1.0)
    .slide_y(px(4.0), px(0.0))
    .apply(content, "rules-page-enter")
}

fn render_toolbar(
    view_model: &RulesProxyPageViewModel,
    inputs: RulesProxyPageInputs,
    palette: ShellPalette,
) -> impl IntoElement {
    let count_label = if view_model.search_query.trim().is_empty() {
        format!("共 {} 条规则", view_model.total_count)
    } else {
        format!(
            "筛选后 {} / {} 条",
            view_model.filtered_count, view_model.total_count
        )
    };

    div()
        .id("rules-toolbar")
        .flex()
        .items_center()
        .gap_3()
        .flex_shrink_0()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(palette.border)
        .child(
            div()
                .flex_1()
                .min_w(px(180.0))
                .child(Input::new(&inputs.search)),
        )
        .child(
            div()
                .flex_none()
                .text_xs()
                .font_bold()
                .text_color(palette.muted)
                .child(count_label),
        )
}
