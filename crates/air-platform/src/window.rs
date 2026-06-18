use gpui::Window;

use air_error::AppResult;

#[cfg(target_os = "windows")]
use air_error::PlatformError;

#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
        IsIconic, IsWindowVisible, SW_HIDE, SW_RESTORE, SetForegroundWindow, ShowWindowAsync,
    },
};

/// 隐藏主窗口。
///
/// GPUI 当前 Windows 后端的 `App::hide()` 仍是空实现，托盘菜单如果继续走 `cx.hide()`
/// 会表现为点击后没有任何变化。这里把平台差异收口到 platform 模块，通过原生 HWND 完成
/// 真正的隐藏，避免 UI 层散落 Win32 调用。
#[cfg(target_os = "windows")]
pub fn hide_window(window: &Window) -> AppResult<()> {
    let hwnd = window_hwnd(window)?;
    show_window_with_command(hwnd, SW_HIDE, "hide")
}

/// 恢复主窗口显示。
///
/// `ShowWindowAsync(SW_RESTORE)` 负责把隐藏或最小化的窗口重新显示，随后尝试前置窗口。
/// Windows 可能因为前台窗口限制拒绝前置，这不影响窗口恢复显示。
#[cfg(target_os = "windows")]
pub fn show_window(window: &Window) -> AppResult<()> {
    let hwnd = window_hwnd(window)?;
    show_window_with_command(hwnd, SW_RESTORE, "restore")?;
    let foreground_result = unsafe { SetForegroundWindow(hwnd) };
    if foreground_result == 0 {
        tracing::warn!("window restored but Windows refused foreground activation");
    }
    Ok(())
}

/// 判断托盘切换语义下窗口是否已经可见。
///
/// Windows 最小化窗口虽然仍可能带有可见样式，但对用户来说需要被恢复；因此这里把最小化
/// 视为“不可见”，让托盘单击可以把它重新显示到前台。
#[cfg(target_os = "windows")]
pub fn is_window_visible(window: &Window) -> AppResult<bool> {
    let hwnd = window_hwnd(window)?;
    let visible = unsafe { IsWindowVisible(hwnd) } != 0;
    let minimized = unsafe { IsIconic(hwnd) } != 0;
    Ok(visible && !minimized)
}

#[cfg(not(target_os = "windows"))]
pub fn hide_window(_window: &Window) -> AppResult<()> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn show_window(_window: &Window) -> AppResult<()> {
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn is_window_visible(_window: &Window) -> AppResult<bool> {
    Ok(true)
}

#[cfg(target_os = "windows")]
fn window_hwnd(window: &Window) -> AppResult<HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| PlatformError::OperationFailed(format!("无法读取窗口句柄: {error}")))?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(handle.hwnd.get() as HWND),
        other => Err(PlatformError::OperationFailed(format!(
            "当前窗口句柄不是 Win32 HWND: {other:?}"
        ))
        .into()),
    }
}

#[cfg(target_os = "windows")]
fn show_window_with_command(hwnd: HWND, command: i32, operation: &str) -> AppResult<()> {
    let result = unsafe { ShowWindowAsync(hwnd, command) };
    if result == 0 {
        // ShowWindowAsync 的返回值表示调用前是否可见，不是成功/失败；隐藏窗口恢复时返回 0 属于正常路径。
        tracing::debug!(
            operation,
            "Win32 ShowWindowAsync returned previous hidden state"
        );
    }
    Ok(())
}
