use gpui::{
    Context, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled, Window,
    div, prelude::FluentBuilder, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{Disableable, Sizable, StyledExt};

use air_ui::icons::{self, Icon};
use air_ui::shell::{Shell, ShellPalette};

use super::render::SubscriptionPageInputs;
use super::state::SubscriptionConfigFormState;
pub(super) fn render_config_form(
    form: &SubscriptionConfigFormState,
    inputs: SubscriptionPageInputs,
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
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(section_title(Icon::FilePenLine, "配置编辑", palette))
                .child(
                    div()
                        .text_xs()
                        .text_color(palette.muted)
                        .child(format!("id: {}", form.id)),
                ),
        )
        .child(form_input("配置名称", inputs.config_name, palette))
        .child(
            div()
                .flex()
                .gap_2()
                .child(form_input(
                    "更新间隔，小时",
                    inputs.config_interval_hours,
                    palette,
                ))
                .child(form_input("节点数量", inputs.config_proxy_count, palette)),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(form_input(
                    "已用流量 GB",
                    inputs.config_usage_used_gb,
                    palette,
                ))
                .child(form_input(
                    "总流量 GB",
                    inputs.config_usage_total_gb,
                    palette,
                )),
        )
        .child(
            div()
                .text_xs()
                .text_color(palette.muted)
                .child(format!("来源: {}", form.source_label)),
        )
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(component_button(
                    "subscription-config-cancel",
                    "取消",
                    Icon::X,
                    true,
                    false,
                    ButtonKind::Ghost,
                    palette,
                    cx,
                    |shell, _, _| shell.close_subscription_modal(),
                ))
                .child(component_button(
                    "subscription-config-save",
                    "保存",
                    Icon::Save,
                    true,
                    false,
                    ButtonKind::Primary,
                    palette,
                    cx,
                    |shell, window, cx| shell.save_subscription_config_form(window, cx),
                )),
        )
}

pub(super) fn form_input(
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

pub(super) fn form_textarea(
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
        .child(
            // 订阅链接等长文本在弹窗里需要明确的 textarea 高度，避免单行输入导致横向滚动过长。
            Input::new(&input).h(px(88.0)),
        )
}

#[derive(Clone, Copy)]
pub(super) enum ButtonKind {
    Primary,
    Ghost,
}

pub(super) fn card_refresh_button(
    id: impl Into<gpui::ElementId>,
    enabled: bool,
    loading: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
    action: impl Fn(&mut Shell, &mut Window, &mut Context<Shell>) + 'static,
) -> impl IntoElement {
    let shell = cx.entity().clone();
    Button::new(id)
        .small()
        .ghost()
        .disabled(!enabled)
        .loading(loading)
        .child(icons::icon(
            Icon::RefreshCw,
            if enabled { palette.text } else { palette.muted },
        ))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            if !enabled {
                return;
            }
            let _ = shell.update(cx, |shell, cx| {
                action(shell, window, cx);
                cx.notify();
            });
        })
}

pub(super) fn close_subscription_modal_button(
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    component_button(
        "subscription-edit-close",
        "",
        Icon::X,
        true,
        false,
        ButtonKind::Ghost,
        palette,
        cx,
        |shell, _, _| shell.close_subscription_modal(),
    )
}

pub(super) fn component_button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    icon: Icon,
    enabled: bool,
    loading: bool,
    kind: ButtonKind,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
    action: impl Fn(&mut Shell, &mut Window, &mut Context<Shell>) + 'static,
) -> impl IntoElement {
    let shell = cx.entity().clone();
    let button = Button::new(id)
        .small()
        .disabled(!enabled)
        .loading(loading)
        .child(icons::icon(
            icon,
            if enabled { palette.text } else { palette.muted },
        ))
        .when(!label.is_empty(), |this| this.child(label))
        .on_click(move |_, window, cx| {
            if !enabled {
                return;
            }
            let _ = shell.update(cx, |shell, cx| {
                action(shell, window, cx);
                cx.notify();
            });
        });

    match kind {
        ButtonKind::Primary => button.primary(),
        ButtonKind::Ghost => button.ghost(),
    }
}

pub(super) fn section_title(
    icon: Icon,
    title: &'static str,
    palette: ShellPalette,
) -> impl IntoElement {
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
