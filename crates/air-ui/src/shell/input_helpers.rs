use super::*;

pub(super) fn text_input(
    window: &mut Window,
    cx: &mut Context<Shell>,
    placeholder: &str,
    value: &str,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(value.to_string())
    })
}

pub(super) fn code_editor_state(
    window: &mut Window,
    cx: &mut Context<InputState>,
    language: &'static str,
    value: impl Into<String>,
) -> InputState {
    // YAML/JSON/JS 编辑器统一使用 4 空格缩进，保持预览和脚本编辑体验一致。
    InputState::new(window, cx)
        .code_editor(language)
        .multi_line(true)
        .tab_size(TabSize {
            tab_size: CODE_EDITOR_TAB_SIZE,
            hard_tabs: false,
        })
        .default_value(value.into())
}

pub(super) fn multiline_text_input(
    window: &mut Window,
    cx: &mut Context<Shell>,
    placeholder: &str,
    value: &str,
) -> Entity<InputState> {
    cx.new(|cx| {
        // 列表字段以换行表达多个条目，必须启用多行模式，避免单行输入渲染换行内容异常。
        InputState::new(window, cx)
            .multi_line(true)
            .rows(4)
            .soft_wrap(false)
            .placeholder(placeholder)
            .default_value(value.to_string())
    })
}

pub(super) fn wrapping_multiline_text_input(
    window: &mut Window,
    cx: &mut Context<Shell>,
    placeholder: &str,
    value: &str,
) -> Entity<InputState> {
    cx.new(|cx| {
        // 订阅 URL 一类长文本需要直接在输入框内换行查看，避免弹窗出现横向滚动条。
        InputState::new(window, cx)
            .multi_line(true)
            .rows(4)
            .soft_wrap(true)
            .placeholder(placeholder)
            .default_value(value.to_string())
    })
}

pub(super) fn subscription_yaml_editor(
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> Entity<InputState> {
    cx.new(|cx| {
        // 订阅编辑页只读展示缓存 YAML；导入和更新仍由订阅流水线负责。
        code_editor_state(window, cx, "yaml", "# 正在读取订阅缓存\n").soft_wrap(false)
    })
}
