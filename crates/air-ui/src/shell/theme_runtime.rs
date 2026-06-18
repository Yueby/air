use super::*;

pub(super) fn apply_theme_preference(
    preference: ShellThemePreference,
    system_mode: ShellThemeMode,
    window: &mut Window,
    cx: &mut Context<Shell>,
) {
    // gpui-component 维护全局组件主题；Shell 颜色从同一个偏好解析。
    if preference == ShellThemePreference::System {
        Theme::sync_system_appearance(Some(window), cx);
    } else {
        Theme::change(
            preference.resolved_mode(system_mode).as_component_mode(),
            Some(window),
            cx,
        );
    }
    super::components::enforce_visible_scrollbars(cx);
}
