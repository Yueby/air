use gpui::{
    Context, Div, InteractiveElement, IntoElement, ParentElement, Stateful,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use gpui_component::StyledExt;
use gpui_component::tooltip::Tooltip;

use air_ui::icons::{self, Icon};
use air_ui::shell::{Shell, ShellPalette};

pub(super) fn page_button(
    id: &'static str,
    label: &'static str,
    icon: Icon,
    enabled: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
    action: impl Fn(&mut Shell) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
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
            cx.stop_propagation();
            if enabled {
                action(shell);
                cx.notify();
            }
        }))
}

pub(super) fn toolbar_icon_button(
    id: String,
    icon: Icon,
    tooltip: &'static str,
    enabled: bool,
    color: gpui::Hsla,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
    action: impl Fn(&mut Shell, &mut Window, &mut Context<Shell>) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .size(px(28.0))
        .rounded_md()
        .cursor_pointer()
        .text_color(if enabled { color } else { palette.muted })
        .hover(move |this| {
            if enabled {
                this.bg(palette.hover)
            } else {
                this
            }
        })
        .tooltip(move |window, cx| Tooltip::new(tooltip).build(window, cx))
        .child(icons::icon(
            icon,
            if enabled { color } else { palette.muted },
        ))
        .on_click(cx.listener(move |shell, _, window, cx| {
            cx.stop_propagation();
            if enabled {
                action(shell, window, cx);
                cx.notify();
            }
        }))
}
