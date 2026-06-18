use air_error::{AppResult, PlatformError};

#[cfg(any(windows, test))]
use std::path::Path;

pub const AUTOSTART_APP_NAME: &str = "Air";

/// 同步当前用户的开机自启状态。
///
/// 自启只负责让系统登录后启动 Air，不携带“静默启动”等 UI 行为参数；
/// 启动后是否隐藏到托盘由 `AppSettings::silent_start` 独立控制。
pub fn set_enabled(enabled: bool) -> AppResult<()> {
    set_enabled_impl(enabled)
}

#[cfg(windows)]
fn set_enabled_impl(enabled: bool) -> AppResult<()> {
    let exe = if enabled {
        Some(std::env::current_exe().map_err(|error| {
            PlatformError::OperationFailed(format!("读取当前程序路径失败: {error}"))
        })?)
    } else {
        None
    };
    windows::set_current_user_run_value(AUTOSTART_APP_NAME, exe.as_deref())
}

#[cfg(not(windows))]
fn set_enabled_impl(enabled: bool) -> AppResult<()> {
    if enabled {
        Err(PlatformError::Unsupported("当前平台尚未接入开机自启".into()).into())
    } else {
        Ok(())
    }
}

#[cfg(any(windows, test))]
pub(crate) fn autostart_command_for_exe(exe: &Path) -> String {
    air_platform::privilege::join_windows_args(&[exe.to_string_lossy().to_string()])
}

#[cfg(windows)]
mod windows {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
        RegCreateKeyExW, RegDeleteValueW, RegSetValueExW,
    };

    use air_error::{AppResult, PlatformError};

    const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    pub(super) fn set_current_user_run_value(name: &str, exe: Option<&Path>) -> AppResult<()> {
        let key = RunKey::open()?;
        let name = wide_null(name);
        match exe {
            Some(exe) => {
                let command = super::autostart_command_for_exe(exe);
                let bytes = wide_null(&command)
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>();
                let status = unsafe {
                    RegSetValueExW(
                        key.handle,
                        name.as_ptr(),
                        0,
                        REG_SZ,
                        bytes.as_ptr(),
                        bytes.len() as u32,
                    )
                };
                if status != 0 {
                    return Err(PlatformError::OperationFailed(format!(
                        "写入开机自启注册表失败: {status}"
                    ))
                    .into());
                }
            }
            None => {
                let status = unsafe { RegDeleteValueW(key.handle, name.as_ptr()) };
                // 删除不存在的 Run 项应视为已关闭，避免 UI 反复报错。
                if status != 0 && status != windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND {
                    return Err(PlatformError::OperationFailed(format!(
                        "删除开机自启注册表失败: {status}"
                    ))
                    .into());
                }
            }
        }
        Ok(())
    }

    struct RunKey {
        handle: HKEY,
    }

    impl RunKey {
        fn open() -> AppResult<Self> {
            let mut handle: HKEY = std::ptr::null_mut();
            let path = wide_null(RUN_KEY_PATH);
            let status = unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    path.as_ptr(),
                    0,
                    std::ptr::null_mut(),
                    REG_OPTION_NON_VOLATILE,
                    KEY_SET_VALUE,
                    std::ptr::null(),
                    &mut handle,
                    std::ptr::null_mut(),
                )
            };
            if status != 0 {
                return Err(PlatformError::OperationFailed(format!(
                    "打开开机自启注册表失败: {status}"
                ))
                .into());
            }
            Ok(Self { handle })
        }
    }

    impl Drop for RunKey {
        fn drop(&mut self) {
            unsafe {
                RegCloseKey(self.handle);
            }
        }
    }

    fn wide_null(value: impl AsRef<OsStr>) -> Vec<u16> {
        value
            .as_ref()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_command_quotes_exe_without_silent_argument() {
        let command = autostart_command_for_exe(Path::new(r"C:\Program Files\Air\air.exe"));

        assert_eq!(command, r#""C:\Program Files\Air\air.exe""#);
        assert!(!command.contains("silent"));
    }
}
