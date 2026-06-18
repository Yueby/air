use air_error::{AppResult, PlatformError};

use super::args::{wide_null, wide_os_null};
use super::install::{install_core_service_elevated_body, uninstall_core_service_elevated_body};
use super::types::{CoreServiceAction, CoreServicePaths, ELEVATED_SERVICE_HELPER_ARG};

pub fn run_elevated_service_helper_from_env() -> AppResult<()> {
    let action = std::env::args()
        .find_map(|arg| CoreServiceAction::from_arg(&arg))
        .ok_or_else(|| PlatformError::OperationFailed("缺少内核服务操作参数".into()))?;
    let paths = CoreServicePaths::resolve_default()?;
    match action {
        CoreServiceAction::Install => install_core_service_elevated_body(&paths),
        CoreServiceAction::Uninstall => uninstall_core_service_elevated_body(),
    }
}

pub(super) fn run_elevated_service_action(action: CoreServiceAction) -> AppResult<()> {
    run_elevated_service_action_impl(action)
}

#[cfg(windows)]
fn run_elevated_service_action_impl(action: CoreServiceAction) -> AppResult<()> {
    use std::mem::{size_of, zeroed};

    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
    use windows_sys::Win32::UI::Shell::{
        SEE_MASK_NO_CONSOLE, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let current_exe = std::env::current_exe().map_err(|error| {
        PlatformError::OperationFailed(format!("读取当前程序路径失败: {error}"))
    })?;
    let args = vec![
        ELEVATED_SERVICE_HELPER_ARG.to_string(),
        action.as_arg().to_string(),
    ];
    let operation = wide_null("runas");
    let file = wide_os_null(current_exe.as_os_str());
    let parameters = wide_null(&air_platform::privilege::join_windows_args(&args));
    let directory = std::env::current_dir()
        .ok()
        .map(|path| wide_os_null(path.as_os_str()))
        .unwrap_or_else(|| vec![0]);

    let mut execute_info: SHELLEXECUTEINFOW = unsafe { zeroed() };
    execute_info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    execute_info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NO_CONSOLE;
    execute_info.lpVerb = operation.as_ptr();
    execute_info.lpFile = file.as_ptr();
    execute_info.lpParameters = parameters.as_ptr();
    execute_info.lpDirectory = directory.as_ptr();
    execute_info.nShow = SW_HIDE;

    // 安装/卸载服务沿用“隐藏提权 helper”模型：GUI 不重启，只等待一次 UAC 授权后的短任务完成。
    let ok = unsafe { ShellExecuteExW(&mut execute_info) };
    if ok == 0 || execute_info.hProcess.is_null() {
        return Err(PlatformError::OperationFailed(format!(
            "管理员权限申请被取消或内核服务操作启动失败，ShellExecuteExW={}",
            unsafe { GetLastError() }
        ))
        .into());
    }

    unsafe {
        WaitForSingleObject(execute_info.hProcess, u32::MAX);
    }
    let mut code = 1u32;
    let ok = unsafe { GetExitCodeProcess(execute_info.hProcess, &mut code) };
    unsafe {
        let _ = windows_sys::Win32::Foundation::CloseHandle(execute_info.hProcess);
    }
    if ok == 0 || code != 0 {
        return Err(PlatformError::OperationFailed(format!(
            "内核服务提权操作失败，退出码: {code}"
        ))
        .into());
    }
    Ok(())
}

#[cfg(not(windows))]
fn run_elevated_service_action_impl(_action: CoreServiceAction) -> AppResult<()> {
    super::non_windows::unsupported_windows_service_elevation()
}
