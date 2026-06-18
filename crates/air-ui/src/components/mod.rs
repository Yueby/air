// 043 任务先定义后续页面优化共享入口，部分 helper 会在 044-048 才接入。
#![allow(dead_code)]

use std::{rc::Rc, time::Duration};

use gpui::{
    Anchor, AnyElement, ElementId, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels,
    SharedString, Styled, Window, div, img, px, rgb,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::notification::{Notification, NotificationType};
use gpui_component::scroll::{ScrollableElement, ScrollbarShow};
use gpui_component::switch::Switch;
use gpui_component::{Disableable, Edges, Sizable, Theme, WindowExt};

use air_ui::icons;

/// 第 5 阶段页面优化共享的视觉与交互约束。
///
/// 本模块只封装 air 需要的轻量入口，优先返回 gpui-component 组件本体；
/// 页面仍然持有各自 view model，搜索、排序、导入校验和运行态操作继续通过
/// domain/app command 完成，避免把业务逻辑塞进可视组件。
pub(crate) mod foundation {
    pub(crate) const SPACE_1: f32 = 4.0;
    pub(crate) const SPACE_2: f32 = 8.0;
    pub(crate) const SPACE_3: f32 = 12.0;
    pub(crate) const SPACE_4: f32 = 16.0;
    pub(crate) const SPACE_5: f32 = 20.0;
    pub(crate) const RADIUS_SM: f32 = 4.0;
    pub(crate) const RADIUS_MD: f32 = 6.0;
    pub(crate) const RADIUS_LG: f32 = 8.0;

    pub(crate) const PAGE_TRANSITION_MS: u64 = 120;
    pub(crate) const HOVER_TRANSITION_MS: u64 = 90;
    pub(crate) const FILTER_TRANSITION_MS: u64 = 100;
    pub(crate) const OVERLAY_TRANSITION_MS: u64 = 150;

    pub(crate) const ICON_RULE: &str = "所有页面图标统一从 air_ui::icons 入口渲染嵌入式 SVG，组件库内置图标继续使用 gpui-component。";
    pub(crate) const SWITCH_RULE: &str =
        "二元开关必须使用 gpui-component::switch::Switch，三态配置字段用显式状态 chip。";
    pub(crate) const SCROLL_RULE: &str =
        "页内滚动容器必须使用 gpui-component::scroll::ScrollableElement，并保持滚动条可见。";
    pub(crate) const ALERT_RULE: &str = "页面短反馈必须交给全局右下角通知，不在页面内渲染 Alert。";
    pub(crate) const NOTIFICATION_RULE: &str =
        "全局短提示必须通过 gpui-component::notification::Notification 派发。";
}

const GLOBAL_NOTIFICATION_EDGE_OFFSET: f32 = 16.0;
const GLOBAL_STATUS_BAR_HEIGHT: f32 = 36.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiNoticeLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl UiNoticeLevel {
    fn title(self) -> &'static str {
        match self {
            Self::Info => "提示",
            Self::Success => "已完成",
            Self::Warning => "需要注意",
            Self::Error => "出现错误",
        }
    }

    fn notification_type(self) -> NotificationType {
        match self {
            Self::Info => NotificationType::Info,
            Self::Success => NotificationType::Success,
            Self::Warning => NotificationType::Warning,
            Self::Error => NotificationType::Error,
        }
    }
}

/// gpui-component 的 Switch 已包含受控状态、禁用态和轻量切换动画；
/// air 页面只传入语义状态和 app/domain 回调，不在页面内继续手写图标开关。
pub(crate) fn app_switch(
    id: impl Into<ElementId>,
    checked: bool,
    disabled: bool,
    tooltip: &'static str,
) -> Switch {
    Switch::new(id)
        .checked(checked)
        .disabled(disabled)
        .small()
        .tooltip(tooltip)
}

/// 全局提示统一通过 gpui-component Root 的 notification list；
/// 这里保留唯一入口，避免页面直接依赖 Root 内部结构或重复设置 autohide 策略。
pub(crate) fn push_global_notice(
    window: &mut Window,
    cx: &mut gpui::App,
    level: UiNoticeLevel,
    message: impl Into<gpui::SharedString>,
) {
    window.push_notification(
        Notification::new()
            .title(level.title())
            .message(message)
            .with_type(level.notification_type()),
        cx,
    );
}

/// 使用 gpui-component 的 ScrollableElement 创建纵向滚动容器。
/// 主题默认会跟随系统隐藏滚动条；第 5 阶段要求可检查的滚动条，因此这里把显示模式钉为 Always。
pub(crate) fn push_persistent_global_notice(
    window: &mut Window,
    cx: &mut gpui::App,
    level: UiNoticeLevel,
    key: impl Into<ElementId>,
    message: impl Into<gpui::SharedString>,
    action_label: impl Into<gpui::SharedString>,
    on_action: impl Fn(&mut Window, &mut gpui::App) + 'static,
) {
    let action_label = action_label.into();
    let on_action = Rc::new(on_action);
    window.push_notification(
        Notification::new()
            .id1::<PersistentGlobalNotice>(key)
            .title(level.title())
            .message(message)
            .with_type(level.notification_type())
            // 保存提醒表达的是当前配置仍有未落盘状态，必须等用户保存或手动关闭，不能按普通 toast 自动消失。
            .autohide(false)
            .action(move |_, _, _| {
                let action_label = action_label.clone();
                let on_action = Rc::clone(&on_action);
                Button::new("persistent-notice-action")
                    .label(action_label)
                    .primary()
                    .on_click(move |_, window, cx| {
                        on_action(window, cx);
                    })
            }),
        cx,
    );
}

pub(crate) fn remove_persistent_global_notice(
    window: &mut Window,
    cx: &mut gpui::App,
    key: impl Into<ElementId>,
) {
    window.remove_notification1::<PersistentGlobalNotice>(key, cx);
}

struct PersistentGlobalNotice;

pub(crate) fn vertical_scroll_area(
    id: impl Into<ElementId>,
    content: impl IntoElement,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .flex_col()
        .size_full()
        .min_h(px(0.0))
        .child(content)
        .overflow_y_scrollbar()
}

/// 在应用启动和主题切换后调用，确保 gpui-component 滚动条不被系统自动隐藏策略覆盖。
pub(crate) fn enforce_visible_scrollbars(cx: &mut gpui::App) {
    Theme::global_mut(cx).scrollbar_show = ScrollbarShow::Always;
}

/// 全局通知固定在窗口右下角，但底部要避让 Shell 状态栏。
/// gpui-component 的通知层读取主题里的 placement/margins；这里集中配置，避免各页面手动定位 toast。
pub(crate) fn configure_global_notifications(cx: &mut gpui::App) {
    let theme = Theme::global_mut(cx);
    let offset = px(GLOBAL_NOTIFICATION_EDGE_OFFSET);
    theme.notification.placement = Anchor::BottomRight;
    theme.notification.margins = Edges {
        top: offset,
        right: offset,
        bottom: px(GLOBAL_STATUS_BAR_HEIGHT + GLOBAL_NOTIFICATION_EDGE_OFFSET),
        left: offset,
    };
}

/// 轻量动画参数集中在这里，后续页面使用这些时长保持交互一致；
/// 复杂动画仍应留在具体组件里，且不能阻塞 app command 派发。
pub(crate) fn animation_duration(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

/// 代码/配置预览输入框使用独立底色，避免跟禁用态输入框混在一起导致 YAML 文本发灰。
pub(crate) fn code_editor_background(palette: air_ui::shell::ShellPalette) -> Hsla {
    if palette.background.l > 0.5 {
        rgb(0xf7f9fb).into()
    } else {
        rgb(0x171b20).into()
    }
}

/// 代码/配置预览输入框的边框比普通卡片更清晰，浅色和深色主题都保持可辨识。
pub(crate) fn code_editor_border(palette: air_ui::shell::ShellPalette) -> Hsla {
    if palette.background.l > 0.5 {
        rgb(0xb9c2cf).into()
    } else {
        rgb(0x4b5563).into()
    }
}

const MAX_EMOJI_CODEPOINTS: usize = 10;

/// 渲染可能包含 emoji 的可见文本。Windows 和 GPUI 字体链对彩色 emoji 支持不稳定；
/// 这里按内嵌 Twemoji SVG 资产做最长匹配，命中的 emoji 以图片渲染，普通文本保持原样。
pub(crate) fn emoji_text(text: impl Into<SharedString>) -> impl IntoElement {
    emoji_text_with_size(text, px(18.0))
}

/// 紧凑控件中的 emoji 需要跟随控件行高缩小，避免图片 emoji 撑高 Tag 或表格行。
pub(crate) fn emoji_text_compact(text: impl Into<SharedString>) -> impl IntoElement {
    emoji_text_with_size(text, px(12.0))
}

fn emoji_text_with_size(text: impl Into<SharedString>, emoji_size: Pixels) -> impl IntoElement {
    let text = text.into();
    let parts = emoji_text_parts(text.as_ref());

    div().flex().items_center().min_w(px(0.0)).children(
        parts
            .into_iter()
            .map(move |part| part.into_element(emoji_size)),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum EmojiTextPart {
    Text(String),
    Emoji { asset_path: String },
}

impl EmojiTextPart {
    fn into_element(self, emoji_size: Pixels) -> AnyElement {
        match self {
            Self::Text(text) => div().min_w(px(0.0)).child(text).into_any_element(),
            Self::Emoji { asset_path } => img(asset_path)
                .w(emoji_size)
                .h(emoji_size)
                .flex_none()
                .into_any_element(),
        }
    }
}

fn emoji_text_parts(text: &str) -> Vec<EmojiTextPart> {
    let chars = text.char_indices().collect::<Vec<_>>();
    let mut parts = Vec::new();
    let mut text_start = 0;
    let mut index = 0;

    while index < chars.len() {
        let (start, _) = chars[index];
        let Some((matched_len, asset_path)) = longest_emoji_asset_match(&chars, index) else {
            index += 1;
            continue;
        };

        let end = chars
            .get(index + matched_len)
            .map(|(next_start, _)| *next_start)
            .unwrap_or_else(|| text.len());
        if text_start < start {
            parts.push(EmojiTextPart::Text(text[text_start..start].to_string()));
        }
        parts.push(EmojiTextPart::Emoji { asset_path });
        text_start = end;
        index += matched_len;
    }

    if text_start < text.len() {
        parts.push(EmojiTextPart::Text(text[text_start..].to_string()));
    }

    if parts.is_empty() {
        parts.push(EmojiTextPart::Text(text.to_string()));
    }

    parts
}

fn longest_emoji_asset_match(chars: &[(usize, char)], start: usize) -> Option<(usize, String)> {
    let max_len = MAX_EMOJI_CODEPOINTS.min(chars.len().saturating_sub(start));
    for len in (1..=max_len).rev() {
        let codepoints = chars[start..start + len]
            .iter()
            .map(|(_, ch)| *ch as u32)
            .collect::<Vec<_>>();
        if let Some(asset_path) = icons::emoji_image_asset_path(&codepoints) {
            return Some((len, asset_path));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{EmojiTextPart, UiNoticeLevel, animation_duration, emoji_text_parts, foundation};

    #[test]
    fn foundation_rules_pin_required_component_sources() {
        assert!(foundation::SWITCH_RULE.contains("gpui-component::switch::Switch"));
        assert!(foundation::SCROLL_RULE.contains("gpui-component::scroll::ScrollableElement"));
        assert!(foundation::ALERT_RULE.contains("全局右下角通知"));
        assert!(
            foundation::NOTIFICATION_RULE.contains("gpui-component::notification::Notification")
        );
        assert!(foundation::ICON_RULE.contains("嵌入式 SVG"));
    }

    #[test]
    fn notice_levels_have_stable_titles() {
        assert_eq!(UiNoticeLevel::Info.title(), "提示");
        assert_eq!(UiNoticeLevel::Success.title(), "已完成");
        assert_eq!(UiNoticeLevel::Warning.title(), "需要注意");
        assert_eq!(UiNoticeLevel::Error.title(), "出现错误");
    }

    #[test]
    fn animation_tokens_match_lightweight_policy() {
        assert_eq!(
            animation_duration(foundation::OVERLAY_TRANSITION_MS).as_millis(),
            150
        );
        assert!(foundation::HOVER_TRANSITION_MS <= foundation::PAGE_TRANSITION_MS);
    }

    #[test]
    fn global_notification_bottom_margin_clears_status_bar() {
        assert!(
            super::GLOBAL_STATUS_BAR_HEIGHT + super::GLOBAL_NOTIFICATION_EDGE_OFFSET
                > super::GLOBAL_STATUS_BAR_HEIGHT
        );
    }

    #[test]
    fn emoji_text_parts_replace_known_emoji_sequences_with_assets() {
        assert_eq!(
            emoji_text_parts("🇭🇰 Hong Kong丨01"),
            vec![
                EmojiTextPart::Emoji {
                    asset_path: "emoji/1f1ed-1f1f0.svg".to_string()
                },
                EmojiTextPart::Text(" Hong Kong丨01".to_string())
            ]
        );

        assert_eq!(
            emoji_text_parts("节点❤️1️⃣"),
            vec![
                EmojiTextPart::Text("节点".to_string()),
                EmojiTextPart::Emoji {
                    asset_path: "emoji/2764.svg".to_string()
                },
                EmojiTextPart::Emoji {
                    asset_path: "emoji/31-20e3.svg".to_string()
                }
            ]
        );

        assert_eq!(
            emoji_text_parts("🏃🏻‍♀️"),
            vec![EmojiTextPart::Emoji {
                asset_path: "emoji/1f3c3-1f3fb-200d-2640-fe0f.svg".to_string()
            }]
        );
    }
}
