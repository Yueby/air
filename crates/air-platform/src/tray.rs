use std::sync::mpsc::{self, Receiver};

use air_error::AppResult;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrayEvent {
    ToggleWindow,
    ShowWindow,
    HideWindow,
    StartCore,
    StopCore,
    Quit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrayOptions {
    pub tooltip: String,
    pub icon_png: Option<&'static [u8]>,
}

impl Default for TrayOptions {
    fn default() -> Self {
        Self {
            tooltip: "Air".to_string(),
            icon_png: None,
        }
    }
}

pub struct TrayHandle {
    inner: TrayHandleInner,
}

impl TrayHandle {
    pub fn disabled() -> Self {
        Self {
            inner: TrayHandleInner::Disabled,
        }
    }

    pub fn is_supported(&self) -> bool {
        match &self.inner {
            TrayHandleInner::Disabled => false,
            #[cfg(windows)]
            TrayHandleInner::Windows(handle) => {
                let _ = handle.thread_id;
                true
            }
        }
    }
}

enum TrayHandleInner {
    Disabled,
    #[cfg(windows)]
    Windows(WindowsTrayHandle),
}

pub fn start_tray(options: TrayOptions) -> AppResult<(TrayHandle, Receiver<TrayEvent>)> {
    let (events, receiver) = mpsc::channel();
    let handle = start_tray_impl(options, events)?;
    Ok((handle, receiver))
}

#[cfg(not(windows))]
fn start_tray_impl(
    _options: TrayOptions,
    _events: mpsc::Sender<TrayEvent>,
) -> AppResult<TrayHandle> {
    // 非 Windows 平台先降级为无托盘句柄，调用方仍可统一保留事件轮询逻辑。
    Ok(TrayHandle::disabled())
}

#[cfg(windows)]
fn start_tray_impl(options: TrayOptions, events: mpsc::Sender<TrayEvent>) -> AppResult<TrayHandle> {
    windows::start_windows_tray(options, events)
}

#[cfg(windows)]
struct WindowsTrayHandle {
    thread_id: u32,
    join: Option<std::thread::JoinHandle<()>>,
}

#[cfg(windows)]
impl Drop for WindowsTrayHandle {
    fn drop(&mut self) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_APP};

        // 托盘消息循环运行在独立线程；Drop 时投递自定义关闭消息，确保 Shell 图标能被 NIM_DELETE 清理。
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_APP + 2, 0, 0);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::mem::size_of;
    use std::sync::{Mutex, OnceLock, mpsc};

    use windows_sys::Win32::Foundation::{
        GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM,
    };
    use windows_sys::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject, HGDIOBJ};
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
        DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursorPos,
        GetMessageW, GetWindowLongPtrW, HICON, HWND_MESSAGE, ICONINFO, IDI_APPLICATION, LoadIconW,
        MF_SEPARATOR, MF_STRING, MSG, PostQuitMessage, RegisterClassW, SetForegroundWindow,
        SetWindowLongPtrW, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_APP, WM_CLOSE,
        WM_COMMAND, WM_DESTROY, WM_LBUTTONUP, WM_RBUTTONUP, WM_USER, WNDCLASSW,
    };

    use super::{TrayEvent, TrayHandle, TrayHandleInner, TrayOptions, WindowsTrayHandle};
    use air_error::{AppResult, PlatformError};

    const TRAY_UID: u32 = 1;
    const WM_TRAY_ICON: u32 = WM_USER + 41;
    const WM_TRAY_SHUTDOWN: u32 = WM_APP + 2;
    const MENU_SHOW: usize = 1001;
    const MENU_HIDE: usize = 1002;
    const MENU_START_CORE: usize = 1003;
    const MENU_STOP_CORE: usize = 1004;
    const MENU_QUIT: usize = 1005;
    const TRAY_CLASS_NAME: &str = "AirTrayMessageWindow";
    const TRAY_ICON_SIZE: u32 = 32;

    static TRAY_EVENTS: OnceLock<Mutex<Option<mpsc::Sender<TrayEvent>>>> = OnceLock::new();
    static TRAY_ICON_HANDLE: OnceLock<Mutex<Option<OwnedTrayIcon>>> = OnceLock::new();

    #[derive(Clone, Copy)]
    struct OwnedTrayIcon {
        handle: isize,
        owned: bool,
    }

    struct TrayThreadReady {
        thread_id: u32,
    }

    pub(super) fn start_windows_tray(
        options: TrayOptions,
        events: mpsc::Sender<TrayEvent>,
    ) -> AppResult<TrayHandle> {
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let join = std::thread::Builder::new()
            .name("air-tray".to_string())
            .spawn(move || run_tray_thread(options, events, ready_tx))
            .map_err(|error| {
                PlatformError::OperationFailed(format!("启动托盘线程失败: {error}"))
            })?;

        match ready_rx.recv() {
            Ok(Ok(ready)) => Ok(TrayHandle {
                inner: TrayHandleInner::Windows(WindowsTrayHandle {
                    thread_id: ready.thread_id,
                    join: Some(join),
                }),
            }),
            Ok(Err(message)) => {
                let _ = join.join();
                Err(PlatformError::OperationFailed(message).into())
            }
            Err(error) => {
                let _ = join.join();
                Err(PlatformError::OperationFailed(format!("托盘线程初始化中断: {error}")).into())
            }
        }
    }

    fn run_tray_thread(
        options: TrayOptions,
        events: mpsc::Sender<TrayEvent>,
        ready: mpsc::SyncSender<Result<TrayThreadReady, String>>,
    ) {
        let result = unsafe { initialize_tray_window(&options, events.clone()) };
        let Ok(hwnd) = result else {
            let _ = ready.send(Err(result.unwrap_err()));
            return;
        };

        let thread_id = unsafe { GetCurrentThreadId() };
        let _ = ready.send(Ok(TrayThreadReady { thread_id }));
        tracing::info!(thread_id, "windows tray initialized");

        unsafe {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            remove_tray_icon(hwnd);
            DestroyWindow(hwnd);
        }
        set_tray_sender(None);
        tracing::info!("windows tray message loop stopped");
    }

    unsafe fn initialize_tray_window(
        options: &TrayOptions,
        events: mpsc::Sender<TrayEvent>,
    ) -> Result<HWND, String> {
        set_tray_sender(Some(events));
        let class_name = wide_null(TRAY_CLASS_NAME);
        let hinstance: HINSTANCE = std::ptr::null_mut();
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(tray_wnd_proc),
            hInstance: hinstance,
            lpszClassName: class_name.as_ptr(),
            ..unsafe { std::mem::zeroed() }
        };
        // RegisterClassW 在同一进程重复调用会返回 0；只要后续 CreateWindowExW 成功即可复用已注册类。
        unsafe {
            let _ = RegisterClassW(&wnd_class);
        }
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            set_tray_sender(None);
            return Err(format!("创建托盘消息窗口失败: {}", unsafe {
                GetLastError()
            }));
        }
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 1);
        }
        if let Err(error) = unsafe { add_tray_icon(hwnd, &options.tooltip, options.icon_png) } {
            unsafe {
                DestroyWindow(hwnd);
            }
            set_tray_sender(None);
            return Err(error);
        }
        Ok(hwnd)
    }

    unsafe extern "system" fn tray_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_TRAY_ICON => {
                let mouse_msg = lparam as u32;
                if mouse_msg == WM_RBUTTONUP {
                    unsafe {
                        show_tray_menu(hwnd);
                    }
                    return 0;
                }
                if let Some(event) = tray_event_from_mouse_message(mouse_msg) {
                    send_event(event);
                    return 0;
                }
            }
            WM_COMMAND => {
                if let Some(event) = menu_event_from_id(wparam & 0xffff) {
                    send_event(event);
                    return 0;
                }
            }
            WM_CLOSE | WM_TRAY_SHUTDOWN => {
                unsafe {
                    remove_tray_icon(hwnd);
                }
                unsafe {
                    PostQuitMessage(0);
                }
                return 0;
            }
            WM_DESTROY => {
                unsafe {
                    remove_tray_icon(hwnd);
                }
                unsafe {
                    PostQuitMessage(0);
                }
                return 0;
            }
            _ => {}
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    unsafe fn add_tray_icon(
        hwnd: HWND,
        tooltip: &str,
        icon_png: Option<&'static [u8]>,
    ) -> Result<(), String> {
        let tray_icon = tray_icon_handle(icon_png);
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_UID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAY_ICON,
            hIcon: tray_icon.handle as HICON,
            ..NOTIFYICONDATAW::default()
        };
        copy_utf16_truncated(&mut data.szTip, tooltip);
        let ok = unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
        if ok == 0 {
            destroy_owned_tray_icon(tray_icon);
            return Err(format!("创建托盘图标失败: {}", unsafe {
                GetLastError()
            }));
        }
        store_tray_icon(Some(tray_icon));
        Ok(())
    }

    unsafe fn remove_tray_icon(hwnd: HWND) {
        if unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } == 0 {
            return;
        }
        let data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_UID,
            ..NOTIFYICONDATAW::default()
        };
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        }
        if let Some(icon) = take_tray_icon() {
            destroy_owned_tray_icon(icon);
        }
    }

    unsafe fn show_tray_menu(hwnd: HWND) {
        let menu = unsafe { CreatePopupMenu() };
        if menu.is_null() {
            return;
        }

        unsafe {
            append_menu_item(menu, MENU_SHOW, "显示窗口");
            append_menu_item(menu, MENU_HIDE, "隐藏窗口");
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            append_menu_item(menu, MENU_START_CORE, "启动内核");
            append_menu_item(menu, MENU_STOP_CORE, "停止内核");
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            append_menu_item(menu, MENU_QUIT, "退出");
        }

        let mut point = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut point);
            // Windows 托盘弹出菜单需要先把隐藏窗口设为前台，否则菜单可能不会在失焦时自动关闭。
            let _ = SetForegroundWindow(hwnd);
            let _ = TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON,
                point.x,
                point.y,
                0,
                hwnd,
                std::ptr::null(),
            );
            DestroyMenu(menu);
        }
    }

    unsafe fn append_menu_item(
        menu: windows_sys::Win32::UI::WindowsAndMessaging::HMENU,
        id: usize,
        label: &str,
    ) {
        let label = wide_null(label);
        unsafe {
            let _ = AppendMenuW(menu, MF_STRING, id, label.as_ptr());
        }
    }

    fn send_event(event: TrayEvent) {
        if let Some(sender) = tray_sender() {
            let _ = sender.send(event);
        }
    }

    fn tray_sender() -> Option<mpsc::Sender<TrayEvent>> {
        TRAY_EVENTS
            .get_or_init(|| Mutex::new(None))
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    fn set_tray_sender(sender: Option<mpsc::Sender<TrayEvent>>) {
        if let Ok(mut guard) = TRAY_EVENTS.get_or_init(|| Mutex::new(None)).lock() {
            *guard = sender;
        }
    }

    fn tray_icon_handle(icon_png: Option<&'static [u8]>) -> OwnedTrayIcon {
        if let Some(icon) = create_tray_icon_from_png(icon_png) {
            return OwnedTrayIcon {
                handle: icon as isize,
                owned: true,
            };
        }

        // PNG 解码或 Win32 图标构造失败时使用系统默认图标兜底，避免托盘功能整体不可用。
        OwnedTrayIcon {
            handle: unsafe { LoadIconW(std::ptr::null_mut(), IDI_APPLICATION) } as isize,
            owned: false,
        }
    }

    fn store_tray_icon(icon: Option<OwnedTrayIcon>) {
        if let Ok(mut guard) = TRAY_ICON_HANDLE.get_or_init(|| Mutex::new(None)).lock() {
            *guard = icon;
        }
    }

    fn take_tray_icon() -> Option<OwnedTrayIcon> {
        TRAY_ICON_HANDLE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .ok()
            .and_then(|mut guard| guard.take())
    }

    fn destroy_owned_tray_icon(icon: OwnedTrayIcon) {
        if icon.owned && icon.handle != 0 {
            unsafe {
                let _ = DestroyIcon(icon.handle as HICON);
            }
        }
    }

    fn create_tray_icon_from_png(icon_png: Option<&'static [u8]>) -> Option<HICON> {
        let png = icon_png?;
        let image = image::load_from_memory(png).ok()?;
        let rgba = image
            .resize_exact(
                TRAY_ICON_SIZE,
                TRAY_ICON_SIZE,
                image::imageops::FilterType::Lanczos3,
            )
            .to_rgba8();

        let mut bgra = Vec::with_capacity((TRAY_ICON_SIZE * TRAY_ICON_SIZE * 4) as usize);
        for pixel in rgba.pixels() {
            let [r, g, b, a] = pixel.0;
            bgra.extend_from_slice(&[b, g, r, a]);
        }

        // Windows 托盘只接受 HICON；应用图标源仍保持 PNG，运行时转换为 32 位颜色位图
        // 和透明 mask，再由 CreateIconIndirect 合成托盘可用的图标句柄。
        unsafe {
            let color_bitmap = CreateBitmap(
                TRAY_ICON_SIZE as i32,
                TRAY_ICON_SIZE as i32,
                1,
                32,
                bgra.as_ptr().cast(),
            );
            if color_bitmap.is_null() {
                return None;
            }

            let mask_stride = TRAY_ICON_SIZE.div_ceil(16) * 2;
            let mask = vec![0u8; (mask_stride * TRAY_ICON_SIZE) as usize];
            let mask_bitmap = CreateBitmap(
                TRAY_ICON_SIZE as i32,
                TRAY_ICON_SIZE as i32,
                1,
                1,
                mask.as_ptr().cast(),
            );
            if mask_bitmap.is_null() {
                let _ = DeleteObject(color_bitmap as HGDIOBJ);
                return None;
            }

            let info = ICONINFO {
                fIcon: 1,
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask_bitmap,
                hbmColor: color_bitmap,
            };
            let icon = CreateIconIndirect(&info);
            let _ = DeleteObject(color_bitmap as HGDIOBJ);
            let _ = DeleteObject(mask_bitmap as HGDIOBJ);
            if icon.is_null() { None } else { Some(icon) }
        }
    }

    pub(super) fn menu_event_from_id(id: usize) -> Option<TrayEvent> {
        match id {
            MENU_SHOW => Some(TrayEvent::ShowWindow),
            MENU_HIDE => Some(TrayEvent::HideWindow),
            MENU_START_CORE => Some(TrayEvent::StartCore),
            MENU_STOP_CORE => Some(TrayEvent::StopCore),
            MENU_QUIT => Some(TrayEvent::Quit),
            _ => None,
        }
    }

    fn tray_event_from_mouse_message(mouse_msg: u32) -> Option<TrayEvent> {
        match mouse_msg {
            // 左键单击只表达“切换窗口”意图，实际显示或隐藏由 UI 线程根据当前窗口状态判断。
            WM_LBUTTONUP => Some(TrayEvent::ToggleWindow),
            _ => None,
        }
    }

    fn copy_utf16_truncated<const N: usize>(target: &mut [u16; N], value: &str) {
        target.fill(0);
        if N == 0 {
            return;
        }
        for (index, unit) in value.encode_utf16().take(N - 1).enumerate() {
            target[index] = unit;
        }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn menu_ids_map_to_tray_events() {
            assert_eq!(menu_event_from_id(MENU_SHOW), Some(TrayEvent::ShowWindow));
            assert_eq!(menu_event_from_id(MENU_HIDE), Some(TrayEvent::HideWindow));
            assert_eq!(
                menu_event_from_id(MENU_START_CORE),
                Some(TrayEvent::StartCore)
            );
            assert_eq!(
                menu_event_from_id(MENU_STOP_CORE),
                Some(TrayEvent::StopCore)
            );
            assert_eq!(menu_event_from_id(MENU_QUIT), Some(TrayEvent::Quit));
            assert_eq!(menu_event_from_id(42), None);
        }

        #[test]
        fn left_click_toggles_window_visibility() {
            assert_eq!(
                tray_event_from_mouse_message(WM_LBUTTONUP),
                Some(TrayEvent::ToggleWindow)
            );
            assert_eq!(tray_event_from_mouse_message(WM_RBUTTONUP), None);
        }

        #[test]
        fn utf16_tooltip_is_truncated_and_nul_terminated() {
            let mut target = [0u16; 4];
            copy_utf16_truncated(&mut target, "abcdef");

            assert_eq!(target, ['a' as u16, 'b' as u16, 'c' as u16, 0]);
        }
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn non_windows_tray_degrades_to_disabled_handle() {
        let (handle, _events) = start_tray(TrayOptions::default()).unwrap();

        assert!(!handle.is_supported());
    }
}
