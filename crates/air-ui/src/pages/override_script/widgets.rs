use gpui::{
    Context, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::StyledExt;
use gpui_component::input::{Input, InputState};

use air_ui::components;
use air_ui::icons::{self, Icon};
use air_ui::shell::{Shell, ShellPalette};

use super::state::{OverridePreviewModalState, OverrideScriptPageViewModel};
pub(super) fn render_toolbar(
    view_model: &OverrideScriptPageViewModel,
    debug_pending: bool,
    save_pending: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    div()
        .id("override-toolbar")
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
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
                .text_sm()
                .font_bold()
                .text_color(palette.text)
                .child(
                    components::app_switch(
                        "override-enabled",
                        view_model.enabled,
                        save_pending || debug_pending,
                        "激活后，生成 core.runtime.config.yaml 前会执行当前覆写脚本。",
                    )
                    .on_click(cx.listener(move |shell, checked, _, cx| {
                        shell.set_override_enabled(*checked);
                        cx.notify();
                    })),
                )
                .child("激活"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(toolbar_button(
                    "override-debug",
                    "调试",
                    Icon::Terminal,
                    !debug_pending && !save_pending,
                    false,
                    palette,
                    cx,
                    |shell, cx| shell.debug_override_script(cx),
                ))
                .child(toolbar_button(
                    "override-save",
                    if view_model.dirty {
                        "保存*"
                    } else {
                        "保存"
                    },
                    Icon::Save,
                    !save_pending && !debug_pending,
                    true,
                    palette,
                    cx,
                    |shell, cx| shell.save_override_script(cx),
                )),
        )
}

pub(super) fn render_editor(editor: Entity<InputState>, palette: ShellPalette) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(
            div()
                .id("override-editor-scroll")
                .flex()
                .flex_col()
                .size_full()
                .min_h(px(0.0))
                .p_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .size_full()
                        .min_w(px(0.0))
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
}

pub(super) fn render_preview_modal(
    modal: OverridePreviewModalState,
    editor: Entity<InputState>,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    if !modal.is_open() {
        return div().into_any_element();
    }

    div()
        .id("override-preview-overlay")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .p_5()
        .bg(palette.background.alpha(0.96))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .w_full()
                .max_w(px(860.0))
                .max_h(px(560.0))
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
                        .child(section_title(Icon::ScrollText, "覆写预览", palette))
                        .child(close_preview_button(palette, cx)),
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

fn toolbar_button(
    id: &'static str,
    label: &'static str,
    icon: Icon,
    enabled: bool,
    primary: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
    action: impl Fn(&mut Shell, &mut Context<Shell>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .h(px(32.0))
        .px_3()
        .rounded_md()
        .cursor_pointer()
        .text_xs()
        .font_bold()
        .bg(if primary && enabled {
            palette.active
        } else if enabled {
            palette.subtle
        } else {
            palette.page
        })
        .text_color(if primary && enabled {
            palette.active_text
        } else if enabled {
            palette.text
        } else {
            palette.muted
        })
        .hover(move |this| {
            if !enabled {
                this
            } else if primary {
                this.bg(palette.active_hover)
            } else {
                this.bg(palette.hover)
            }
        })
        .child(icons::icon(
            icon,
            if primary && enabled {
                palette.active_text
            } else if enabled {
                palette.text
            } else {
                palette.muted
            },
        ))
        .child(label)
        .on_click(cx.listener(move |shell, _, _, cx| {
            if enabled {
                action(shell, cx);
            }
        }))
}

fn close_preview_button(palette: ShellPalette, cx: &mut Context<Shell>) -> impl IntoElement {
    div()
        .id("override-preview-close")
        .flex()
        .items_center()
        .justify_center()
        .size(px(32.0))
        .rounded_md()
        .cursor_pointer()
        .hover(move |this| this.bg(palette.hover))
        .child(icons::icon(Icon::X, palette.text))
        .on_click(cx.listener(|shell, _, _, cx| {
            shell.close_override_preview();
            cx.notify();
        }))
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
