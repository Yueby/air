use gpui::{
    AppContext, Context, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement,
    ScrollHandle, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
    relative,
};
use gpui_component::animation::{Transition, ease_out_cubic};
use gpui_component::button::{Button, ButtonVariants, DropdownButton};
use gpui_component::input::{Input, InputState};
use gpui_component::menu::{ContextMenuExt, PopupMenuItem};
use gpui_component::scroll::ScrollableElement;
use gpui_component::tooltip::Tooltip;
use gpui_component::{ActiveTheme, StyledExt};

use air_mihomo::subscriptions::{
    SubscriptionCacheMetadata, SubscriptionDiagnostic, SubscriptionDiagnosticSeverity,
    SubscriptionSource, SubscriptionUpdateOutcome,
};
use air_telemetry::redaction::redact_log_value;
use air_ui::components::{self, foundation};
use air_ui::icons::Icon;
use air_ui::shell::{Shell, ShellPalette};

use super::cache_view::*;
use super::form_render::*;
use super::format::*;
use super::state::*;
impl SubscriptionListItem {
    pub(super) fn from_source(
        source: &SubscriptionSource,
        cache: Option<&SubscriptionCacheMetadata>,
        diagnostics: Vec<SubscriptionDiagnostic>,
        node_count: usize,
        selected: bool,
        updating: bool,
    ) -> Self {
        let last_update = cache.and_then(|cache| cache.last_update.as_ref());
        let last_error = last_update
            .filter(|result| result.outcome == SubscriptionUpdateOutcome::Failed)
            .and_then(|result| result.message.clone());
        let last_checked_at = last_update.map(|result| result.checked_at);
        let now = now_timestamp();
        let last_checked = last_update
            .map(|result| format_relative_past_timestamp(result.checked_at, now))
            .unwrap_or_else(|| "从未更新".to_string());
        let last_checked_tooltip = last_update
            .map(|result| format_shanghai_timestamp(result.checked_at))
            .unwrap_or_else(|| "从未更新".to_string());
        let last_success = cache
            .and_then(|cache| cache.last_success_at)
            .map(format_shanghai_timestamp)
            .unwrap_or_else(|| "-".to_string());
        let cache_state = cache_state(cache, last_update);
        let cache_label = cache_label(cache, last_update, cache_state);
        let usage = usage_from_cache(source, cache, cache_state, now);

        Self {
            id: source.id.clone(),
            name: source.name.clone(),
            url_label: source
                .url
                .as_ref()
                .map(|url| url.redacted_label().to_string())
                .unwrap_or_else(|| "本地导入".to_string()),
            enabled: source.enabled,
            updating,
            selected,
            node_count,
            last_checked_at,
            last_success,
            last_checked,
            last_checked_tooltip,
            last_error,
            cache_state,
            cache_label,
            usage,
            diagnostics: diagnostics
                .into_iter()
                .map(SubscriptionDiagnosticView::from)
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionCacheState {
    Ready,
    StaleAfterFailure,
    FailedNoCache,
    Empty,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubscriptionDiagnosticView {
    pub severity: SubscriptionDiagnosticSeverity,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubscriptionYamlImportValidation {
    pub accepted: bool,
    pub message: String,
    pub proxy_count: usize,
    pub diagnostics: Vec<SubscriptionDiagnosticView>,
}

impl SubscriptionYamlImportValidation {
    pub(super) fn accepted(
        proxy_count: usize,
        diagnostics: Vec<SubscriptionDiagnosticView>,
    ) -> Self {
        Self {
            accepted: true,
            message: format!("YAML 校验通过，识别到 {proxy_count} 个节点"),
            proxy_count,
            diagnostics,
        }
    }

    pub(super) fn rejected(message: impl Into<String>) -> Self {
        Self {
            accepted: false,
            message: message.into(),
            proxy_count: 0,
            diagnostics: Vec::new(),
        }
    }
}

impl From<SubscriptionDiagnostic> for SubscriptionDiagnosticView {
    fn from(diagnostic: SubscriptionDiagnostic) -> Self {
        Self {
            severity: diagnostic.severity,
            code: diagnostic.code,
            message: diagnostic.message,
        }
    }
}

impl From<&SubscriptionDiagnosticView> for SubscriptionDiagnostic {
    fn from(diagnostic: &SubscriptionDiagnosticView) -> Self {
        Self {
            severity: diagnostic.severity.clone(),
            code: diagnostic.code.clone(),
            message: redact_log_value(&diagnostic.message),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscriptionNotice {
    pub level: SubscriptionNoticeLevel,
    pub message: String,
}

impl SubscriptionNotice {
    pub(super) fn success(message: impl Into<String>) -> Self {
        Self {
            level: SubscriptionNoticeLevel::Success,
            message: message.into(),
        }
    }

    pub(super) fn warning(message: impl Into<String>) -> Self {
        Self {
            level: SubscriptionNoticeLevel::Warning,
            message: message.into(),
        }
    }

    pub(super) fn error(message: impl Into<String>) -> Self {
        Self {
            level: SubscriptionNoticeLevel::Error,
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionNoticeLevel {
    Success,
    Warning,
    Error,
}

#[derive(Clone)]
pub(crate) struct SubscriptionPageInputs {
    pub import_url: Entity<InputState>,
    pub name: Entity<InputState>,
    pub url: Entity<InputState>,
    pub interval_hours: Entity<InputState>,
    pub user_agent: Entity<InputState>,
    pub proxy: Entity<InputState>,
    pub request_headers: Entity<InputState>,
    pub config_name: Entity<InputState>,
    pub config_interval_hours: Entity<InputState>,
    pub config_proxy_count: Entity<InputState>,
    pub config_usage_used_gb: Entity<InputState>,
    pub config_usage_total_gb: Entity<InputState>,
    pub yaml_preview_editor: Entity<InputState>,
}

impl SubscriptionPageInputs {
    pub(crate) fn clear_import_url(&self, window: &mut Window, cx: &mut Context<Shell>) {
        self.import_url
            .update(cx, |input, cx| input.set_value(String::new(), window, cx));
    }

    pub(crate) fn set_from_form(
        &self,
        form: &SubscriptionFormState,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) {
        // InputState 自身持有光标和历史；切换新增/编辑目标时同步实体内容，
        // 避免表单状态和可见输入框出现两套草稿。
        self.name.update(cx, |input, cx| {
            input.set_value(form.name.clone(), window, cx)
        });
        self.url.update(cx, |input, cx| {
            input.set_value(form.url.clone(), window, cx)
        });
        self.interval_hours.update(cx, |input, cx| {
            input.set_value(form.interval_hours.clone(), window, cx)
        });
        self.user_agent.update(cx, |input, cx| {
            input.set_value(form.user_agent.clone(), window, cx)
        });
        self.proxy.update(cx, |input, cx| {
            input.set_value(form.proxy.clone(), window, cx)
        });
        self.request_headers.update(cx, |input, cx| {
            input.set_value(form.request_headers.clone(), window, cx)
        });
    }

    pub(crate) fn set_yaml_preview(
        &self,
        contents: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) {
        self.yaml_preview_editor
            .update(cx, |input, cx| input.set_value(contents.into(), window, cx));
    }
}

pub(crate) fn render_subscription_page(
    state: &SubscriptionPageState,
    inputs: SubscriptionPageInputs,
    palette: ShellPalette,
    page_width: f32,
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
        .child(render_import_toolbar(
            &view_model,
            inputs.import_url.clone(),
            palette,
            cx,
        ))
        .child(render_subscription_cards_area(
            &view_model,
            &state.card_scroll_handle,
            palette,
            page_width,
            cx,
        ))
        .child(render_subscription_modal(&view_model, inputs, palette, cx));

    Transition::new(components::animation_duration(
        foundation::PAGE_TRANSITION_MS,
    ))
    .ease(ease_out_cubic)
    .fade(0.0, 1.0)
    .slide_y(px(4.0), px(0.0))
    .apply(content, "subscription-page-enter")
}

fn render_import_toolbar(
    view_model: &SubscriptionPageViewModel,
    import_url_input: Entity<InputState>,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    div()
        .id("subscription-import-toolbar")
        .flex()
        .items_center()
        .gap_2()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(palette.border)
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .child(Input::new(&import_url_input).w_full()),
        )
        .child(import_dropdown_button(
            view_model.import_status == SubscriptionImportStatus::Importing,
            cx,
        ))
}

fn render_subscription_cards(
    view_model: &SubscriptionPageViewModel,
    palette: ShellPalette,
    page_width: f32,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let content = if view_model.items.is_empty() {
        div()
            .flex()
            .items_center()
            .justify_center()
            .h(px(280.0))
            .text_sm()
            .text_color(palette.muted)
            .child("暂无订阅源")
    } else {
        let columns = subscription_card_columns(page_width);
        view_model.items.iter().fold(
            div().grid().grid_cols(columns).gap_3().w_full(),
            |grid, item| grid.child(render_subscription_card(item, palette, cx)),
        )
    };

    Transition::new(components::animation_duration(
        foundation::FILTER_TRANSITION_MS,
    ))
    .ease(ease_out_cubic)
    .fade(0.0, 1.0)
    .apply(div().p_1().child(content), "subscription-card-grid")
}

fn render_subscription_cards_area(
    view_model: &SubscriptionPageViewModel,
    scroll_handle: &ScrollHandle,
    palette: ShellPalette,
    page_width: f32,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    div()
        .id("subscription-card-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .min_h(px(0.0))
        .overflow_hidden()
        .child(
            div()
                .id("subscription-card-scroll-area")
                .flex()
                .flex_col()
                .size_full()
                .min_h(px(0.0))
                .track_scroll(scroll_handle)
                .overflow_y_scroll()
                .child(div().px_4().py_3().child(render_subscription_cards(
                    view_model, palette, page_width, cx,
                ))),
        )
        .vertical_scrollbar(scroll_handle)
}

fn import_dropdown_button(loading: bool, cx: &mut Context<Shell>) -> impl IntoElement {
    let shell = cx.entity().clone();
    let primary_shell = shell.clone();
    DropdownButton::new("subscription-import-menu")
        .primary()
        .button(
            Button::new("subscription-import-primary")
                .label("导入")
                .on_click(move |_, window, cx| {
                    let _ = primary_shell.update(cx, |shell, cx| {
                        shell.import_subscription_url(window, cx);
                        cx.notify();
                    });
                }),
        )
        .loading(loading)
        .dropdown_menu(move |menu, _window, _cx| {
            let remote_shell = shell.clone();
            let local_shell = shell.clone();
            menu.item(
                PopupMenuItem::new("导入远程").on_click(move |_, window, cx| {
                    let _ = remote_shell.update(cx, |shell, cx| {
                        shell.import_subscription_url(window, cx);
                        cx.notify();
                    });
                }),
            )
            .item(
                PopupMenuItem::new("导入本地").on_click(move |_, window, cx| {
                    let _ = local_shell.update(cx, |shell, cx| {
                        shell.choose_subscription_yaml_file(window, cx);
                        cx.notify();
                    });
                }),
            )
        })
}

fn subscription_card_columns(page_width: f32) -> u16 {
    const CARD_MIN_WIDTH: f32 = 280.0;
    const CARD_GAP: f32 = 12.0;
    const LIST_INLINE_PADDING: f32 = 40.0;

    // 列数只由页面可用宽度推导，扣除列表左右内边距和卡片间距后交给 grid 平分，
    // 这样窗口变化时同一行会铺满，同时所有订阅卡片保持相同宽度。
    let available_width = (page_width - LIST_INLINE_PADDING).max(CARD_MIN_WIDTH);
    (((available_width + CARD_GAP) / (CARD_MIN_WIDTH + CARD_GAP)).floor() as u16).max(1)
}

fn render_subscription_card(
    item: &SubscriptionListItem,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let id = item.id.clone();
    let state_color = cache_color(item.cache_state, palette);
    let usage_width = relative((item.usage.percent / 100.0).clamp(0.0, 1.0));
    let card_background = if item.selected {
        palette.subtle
    } else {
        palette.page
    };
    let foreground = if item.enabled {
        palette.text
    } else {
        palette.muted
    };

    let card = div()
        .id(format!("subscription-card-{}", item.id))
        .relative()
        .overflow_hidden()
        .flex()
        .flex_col()
        .gap_3()
        .w_full()
        .min_w(px(0.0))
        .h(px(132.0))
        .p_4()
        .rounded_md()
        .border_1()
        .border_color(if item.selected {
            palette.active
        } else {
            palette.border
        })
        .cursor_pointer()
        .bg(card_background)
        .hover(move |this| this.bg(palette.hover).border_color(palette.active))
        .child(
            div()
                .relative()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .text_base()
                        .font_bold()
                        .text_color(foreground)
                        .child(components::emoji_text(item.name.clone())),
                )
                .child(card_refresh_button(
                    format!("subscription-update-{}", item.id),
                    !item.updating,
                    item.updating,
                    palette,
                    cx,
                    {
                        let id = item.id.clone();
                        move |shell, window, cx| {
                            shell.update_subscription_by_id(id.clone(), window, cx)
                        }
                    },
                )),
        )
        .child(
            div()
                .relative()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(render_usage_summary(item, foreground, palette)),
                )
                .child(
                    div()
                        .flex_none()
                        .text_sm()
                        .font_bold()
                        .text_color(palette.muted)
                        .child(item.last_checked.clone())
                        .id(format!("subscription-updated-at-{}", item.id))
                        .tooltip({
                            let tooltip = item.last_checked_tooltip.clone();
                            move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx)
                        }),
                ),
        )
        .child(
            div()
                .relative()
                .h(px(6.0))
                .rounded_sm()
                .overflow_hidden()
                .bg(palette.surface)
                .child(div().h_full().w(usage_width).bg(if item.updating {
                    palette.warning
                } else {
                    state_color
                })),
        )
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.select_subscription(id.clone());
            cx.notify();
        }))
        .on_drag(
            SubscriptionCardDrag {
                id: item.id.clone(),
            },
            |drag, _, _, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
            },
        )
        .drag_over::<SubscriptionCardDrag>(move |this, _, _, cx| {
            this.border_color(cx.theme().drag_border)
                .border_l_2()
                .shadow_md()
                .bg(palette.hover)
        })
        .on_drop(cx.listener({
            let target_id = item.id.clone();
            move |shell, drag: &SubscriptionCardDrag, window, cx| {
                shell.reorder_subscription_before(drag.id.clone(), target_id.clone(), window, cx);
                cx.notify();
            }
        }))
        .context_menu({
            let id = item.id.clone();
            let shell = cx.entity().clone();
            move |menu, _window, _cx| {
                let edit_id = id.clone();
                let delete_id = id.clone();
                let edit_shell = shell.clone();
                let delete_shell = shell.clone();
                menu.item(PopupMenuItem::new("编辑").on_click(move |_, window, cx| {
                    let _ = edit_shell.update(cx, |shell, cx| {
                        shell.begin_edit_subscription_by_id(edit_id.clone(), window, cx);
                        cx.notify();
                    });
                }))
                .item(PopupMenuItem::new("删除").on_click(move |_, _window, cx| {
                    let _ = delete_shell.update(cx, |shell, cx| {
                        shell.delete_subscription_by_id(delete_id.clone(), _window, cx);
                        cx.notify();
                    });
                }))
            }
        });

    Transition::new(components::animation_duration(
        foundation::HOVER_TRANSITION_MS,
    ))
    .ease(ease_out_cubic)
    .fade(0.0, 1.0)
    .apply(card, format!("subscription-card-transition-{}", item.id))
}

fn render_usage_summary(
    item: &SubscriptionListItem,
    foreground: gpui::Hsla,
    palette: ShellPalette,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .font_bold()
        .text_color(foreground)
        .child(item.usage.label.clone())
        .when_some(item.usage.expires_label.clone(), |this, label| {
            this.child(
                div()
                    .text_sm()
                    .font_bold()
                    .text_color(palette.muted)
                    .child(format!("· {label}"))
                    .id(format!("subscription-expires-at-{}", item.id))
                    .when_some(item.usage.expires_tooltip.clone(), |this, tooltip| {
                        this.tooltip(move |window, cx| {
                            Tooltip::new(tooltip.clone()).build(window, cx)
                        })
                    }),
            )
        })
}

fn render_subscription_modal(
    view_model: &SubscriptionPageViewModel,
    inputs: SubscriptionPageInputs,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    if view_model.modal == SubscriptionModalState::None {
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
        .bg(palette.background.alpha(0.96))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(match view_model.modal {
            SubscriptionModalState::Add => {
                render_subscription_form(&view_model.form, inputs, palette, cx).into_any_element()
            }
            SubscriptionModalState::EditSubscription => render_subscription_edit_form(
                &view_model.form,
                inputs,
                view_model.yaml_preview_loading,
                palette,
                cx,
            )
            .into_any_element(),
            SubscriptionModalState::EditConfig => {
                render_config_form(&view_model.config_form, inputs, palette, cx).into_any_element()
            }
            SubscriptionModalState::None => div().into_any_element(),
        })
}

fn render_subscription_edit_form(
    form: &SubscriptionFormState,
    inputs: SubscriptionPageInputs,
    yaml_loading: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    div()
        .flex()
        .gap_4()
        .w_full()
        .max_w(px(980.0))
        .h(px(580.0))
        .p_4()
        .rounded_md()
        .border_1()
        .border_color(palette.border)
        .bg(palette.page)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .w(px(340.0))
                .min_h(px(0.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(section_title(Icon::Pencil, "编辑订阅", palette))
                        .child(
                            div()
                                .text_xs()
                                .text_color(palette.muted)
                                .child(format!("id: {}", form.id)),
                        ),
                )
                .child(form_input("名称", inputs.name, palette))
                .child(form_input("更新间隔，小时", inputs.interval_hours, palette))
                .child(form_textarea("订阅 URL", inputs.url, palette))
                .child(form_input("User-Agent", inputs.user_agent, palette))
                .child(form_input("更新代理，例如 DIRECT", inputs.proxy, palette))
                .child(form_input(
                    "请求头，每行 Name: Value",
                    inputs.request_headers,
                    palette,
                ))
                .child(div().flex_1())
                .child(
                    Button::new("subscription-edit-save")
                        .primary()
                        .label("保存")
                        .w_full()
                        .on_click(cx.listener(|shell, _, window, cx| {
                            shell.save_subscription_form(window, cx);
                            cx.notify();
                        })),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .flex_1()
                .min_w(px(0.0))
                .min_h(px(0.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(section_title(Icon::ScrollText, "订阅 YAML", palette))
                        .child(close_subscription_modal_button(palette, cx)),
                )
                .when(yaml_loading, |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(palette.muted)
                            .child("正在读取订阅缓存"),
                    )
                })
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
                            Input::new(&inputs.yaml_preview_editor)
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .font_family("monospace")
                                .size_full(),
                        ),
                ),
        )
}

fn render_subscription_form(
    form: &SubscriptionFormState,
    inputs: SubscriptionPageInputs,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .w(px(600.0))
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
                .child(section_title(Icon::Pencil, "订阅表单", palette))
                .child(
                    div()
                        .text_xs()
                        .text_color(palette.muted)
                        .child(format!("id: {}", form.id)),
                ),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(form_input("名称", inputs.name, palette))
                .child(form_input("更新间隔，小时", inputs.interval_hours, palette)),
        )
        .child(form_textarea(
            "订阅 URL，保存后列表默认脱敏显示",
            inputs.url,
            palette,
        ))
        .child(
            div()
                .flex()
                .gap_2()
                .child(form_input("User-Agent", inputs.user_agent, palette))
                .child(form_input("更新代理，例如 DIRECT", inputs.proxy, palette)),
        )
        .child(form_input(
            "请求头，每行 Name: Value",
            inputs.request_headers,
            palette,
        ))
        .child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(component_button(
                    "subscription-form-cancel",
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
                    "subscription-form-save",
                    "保存",
                    Icon::Save,
                    true,
                    false,
                    ButtonKind::Primary,
                    palette,
                    cx,
                    |shell, window, cx| shell.save_subscription_form(window, cx),
                )),
        )
}
