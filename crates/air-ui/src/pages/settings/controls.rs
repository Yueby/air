use gpui::{
    Axis, Entity, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, div, px,
};
use gpui_component::input::{Input, InputState};
use gpui_component::setting::{SettingField, SettingGroup, SettingItem};
use gpui_component::tooltip::Tooltip;

use air_settings::GuiThemePreference;
use air_ui::icons::{self, Icon};
use air_ui::pages::config_editor::{self, ConfigBoolField, ConfigEditorGroup, ConfigTextField};
use air_ui::shell::{Shell, ShellPalette};

use super::application_pages::SettingsBoolField;
pub(super) fn with_config_notice_group(
    group: ConfigEditorGroup,
    model: &config_editor::ConfigEditorViewModel,
    palette: ShellPalette,
    shell: Entity<Shell>,
    groups: Vec<SettingGroup>,
) -> Vec<SettingGroup> {
    let _ = (group, model, palette, shell);
    // 设置页的保存提示改走右下角全局持久通知，页面内容区不再插入 alert，
    // 避免表单在首次编辑时被额外提示块顶动并造成焦点附近布局跳变。
    groups
}

pub(super) fn input_item(
    label: &'static str,
    input: Entity<InputState>,
    description: &'static str,
    _palette: ShellPalette,
) -> SettingItem {
    SettingItem::new(
        label,
        SettingField::render(move |_, _, _| Input::new(&input).w_full()),
    )
    .layout(Axis::Vertical)
    .description(description)
}

pub(super) fn textarea_item(
    label: &'static str,
    input: Entity<InputState>,
    description: &'static str,
    _palette: ShellPalette,
) -> SettingItem {
    SettingItem::new(
        label,
        SettingField::render(move |_, _, _| {
            // 数组字段在表单层按行拆分，固定多行高度可以明确表达 textarea 语义，
            // 避免把换行数组误呈现成单行输入。
            Input::new(&input).w_full().h(px(112.0))
        }),
    )
    .layout(Axis::Vertical)
    .description(description)
}

pub(super) fn app_switch_item(
    title: &'static str,
    description: &'static str,
    checked: bool,
    field: SettingsBoolField,
    shell: Entity<Shell>,
    _palette: ShellPalette,
) -> SettingItem {
    SettingItem::new(
        title,
        SettingField::switch(
            move |_| checked,
            move |value, cx| {
                shell.update(cx, |shell, cx| {
                    shell.set_settings_bool(field, value);
                    cx.notify();
                });
            },
        ),
    )
    .description(description)
}

pub(super) fn config_switch_item(
    title: &'static str,
    description: &'static str,
    value: Option<bool>,
    default: bool,
    field: ConfigBoolField,
    shell: Entity<Shell>,
    _palette: ShellPalette,
) -> SettingItem {
    let checked = value.unwrap_or(default);
    SettingItem::new(
        title,
        SettingField::switch(
            move |_| checked,
            move |value, cx| {
                shell.update(cx, |shell, cx| {
                    shell.set_config_bool(field, value);
                    cx.notify();
                });
            },
        ),
    )
    .description(description)
}

pub(super) fn config_choice_item(
    title: &'static str,
    description: &'static str,
    selected: &str,
    default: &'static str,
    choices: Vec<(&'static str, &'static str, Icon)>,
    field: ConfigTextField,
    shell: Entity<Shell>,
    palette: ShellPalette,
) -> SettingItem {
    let selected = if selected.trim().is_empty() {
        default
    } else {
        selected
    }
    .to_string();
    SettingItem::new(
        title,
        SettingField::render(move |_, _, _| {
            segmented_choices(
                choices
                    .iter()
                    .map(|(value, label, icon)| {
                        (
                            *label,
                            *icon,
                            selected.as_str() == *value,
                            ChoiceAction::ConfigText(field, *value),
                        )
                    })
                    .collect(),
                shell.clone(),
                palette,
            )
        }),
    )
    .description(description)
}

pub(super) fn config_dropdown_item(
    title: &'static str,
    description: &'static str,
    selected: &str,
    default: &'static str,
    choices: Vec<(&'static str, &'static str)>,
    field: ConfigTextField,
    shell: Entity<Shell>,
) -> SettingItem {
    let selected = if selected.trim().is_empty() {
        default
    } else {
        selected
    }
    .to_string();
    let options = choices
        .into_iter()
        .map(|(value, label)| (SharedString::from(value), SharedString::from(label)))
        .collect::<Vec<_>>();

    SettingItem::new(
        title,
        SettingField::scrollable_dropdown(
            options,
            move |_| SharedString::from(selected.clone()),
            move |value, cx| {
                shell.update(cx, |shell, cx| {
                    shell.set_config_text(field, value.to_string());
                    cx.notify();
                });
            },
        ),
    )
    .description(description)
}

pub(super) fn theme_choice_item(
    selected: GuiThemePreference,
    shell: Entity<Shell>,
    palette: ShellPalette,
) -> SettingItem {
    SettingItem::new(
        "主题",
        SettingField::render(move |_, _, _| {
            segmented_choices(
                vec![
                    (
                        "跟随系统",
                        Icon::MonitorCog,
                        selected == GuiThemePreference::System,
                        ChoiceAction::Theme(GuiThemePreference::System),
                    ),
                    (
                        "浅色",
                        Icon::Sun,
                        selected == GuiThemePreference::Light,
                        ChoiceAction::Theme(GuiThemePreference::Light),
                    ),
                    (
                        "深色",
                        Icon::Moon,
                        selected == GuiThemePreference::Dark,
                        ChoiceAction::Theme(GuiThemePreference::Dark),
                    ),
                ],
                shell.clone(),
                palette,
            )
        }),
    )
}

#[derive(Clone, Copy)]
enum ChoiceAction {
    Theme(GuiThemePreference),
    ConfigText(ConfigTextField, &'static str),
}

fn segmented_choices(
    choices: Vec<(&'static str, Icon, bool, ChoiceAction)>,
    shell: Entity<Shell>,
    palette: ShellPalette,
) -> impl IntoElement {
    choices.into_iter().fold(
        div().flex().items_center().gap_2().flex_wrap(),
        |row, (label, icon, selected, action)| {
            row.child(
                div()
                    .id(format!("settings-choice-{label}"))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_none()
                    .h(px(32.0))
                    .w(px(32.0))
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if selected {
                        palette.active
                    } else {
                        palette.subtle
                    })
                    .text_color(if selected {
                        palette.active_text
                    } else {
                        palette.text
                    })
                    .hover(move |this| {
                        if selected {
                            this.bg(palette.active_hover)
                        } else {
                            this.bg(palette.hover)
                        }
                    })
                    .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
                    .child(icons::icon(
                        icon,
                        if selected {
                            palette.active_text
                        } else {
                            palette.text
                        },
                    ))
                    .on_click({
                        let shell = shell.clone();
                        move |_, window, cx| match action {
                            ChoiceAction::Theme(theme) => {
                                shell.update(cx, |shell, cx| {
                                    shell.set_settings_theme(theme, window, cx);
                                    cx.notify();
                                });
                            }
                            ChoiceAction::ConfigText(field, value) => {
                                shell.update(cx, |shell, cx| {
                                    shell.set_config_text(field, value);
                                    cx.notify();
                                });
                            }
                        }
                    }),
            )
        },
    )
}

pub(super) fn readonly_value_item(
    title: &'static str,
    value: &'static str,
    icon: Icon,
    palette: ShellPalette,
) -> SettingItem {
    SettingItem::new(
        title,
        SettingField::render(move |_, _, _| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(icons::icon(icon, palette.active))
                .child(div().text_sm().text_color(palette.muted).child(value))
        }),
    )
}
