use gpui::{Context, Entity, IntoElement, ParentElement, Styled, div, px};
use gpui_component::animation::{Transition, ease_out_cubic};
use gpui_component::input::InputState;

use air_ui::components::{self, foundation};
use air_ui::shell::{Shell, ShellPalette};

mod state;
mod widgets;

#[cfg(test)]
mod tests;

#[cfg(test)]
use state::OverridePreviewModalState;
pub(crate) use state::OverrideScriptPageState;
#[derive(Clone)]
pub(crate) struct OverrideScriptPageInputs {
    pub(crate) editor: Entity<InputState>,
    pub(crate) preview_editor: Entity<InputState>,
}

pub(crate) fn render_override_script_page(
    state: &OverrideScriptPageState,
    inputs: OverrideScriptPageInputs,
    debug_pending: bool,
    save_pending: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let view_model = state.view_model();
    let content = div()
        .relative()
        .flex()
        .flex_col()
        .size_full()
        .min_w_0()
        .min_h(px(0.0))
        .overflow_hidden()
        .bg(palette.background)
        .child(widgets::render_toolbar(
            &view_model,
            debug_pending,
            save_pending,
            palette,
            cx,
        ))
        .child(widgets::render_editor(inputs.editor.clone(), palette))
        .child(widgets::render_preview_modal(
            view_model.preview_modal,
            inputs.preview_editor,
            palette,
            cx,
        ));

    Transition::new(components::animation_duration(
        foundation::PAGE_TRANSITION_MS,
    ))
    .ease(ease_out_cubic)
    .fade(0.0, 1.0)
    .slide_y(px(4.0), px(0.0))
    .apply(content, "override-script-page-enter")
}
