use air_error::{AppResult, PlatformError};

use super::args::{service_binary_args, wide_null};
use super::control::{query_core_service, stop_core_service};
use super::elevated::run_elevated_service_action;
use super::types::{
    CORE_SERVICE_DISPLAY_NAME, CORE_SERVICE_NAME, CoreServiceAction, CoreServicePaths,
    CoreServiceSnapshot,
};

pub fn install_core_service(paths: &CoreServicePaths) -> AppResult<CoreServiceSnapshot> {
    if air_platform::privilege::current_process_is_elevated()? {
        install_core_service_elevated_body(paths)?;
    } else {
        run_elevated_service_action(CoreServiceAction::Install)?;
    }
    query_core_service()
}

pub fn uninstall_core_service() -> AppResult<CoreServiceSnapshot> {
    if air_platform::privilege::current_process_is_elevated()? {
        uninstall_core_service_elevated_body()?;
    } else {
        run_elevated_service_action(CoreServiceAction::Uninstall)?;
    }
    query_core_service()
}

pub(super) fn install_core_service_elevated_body(paths: &CoreServicePaths) -> AppResult<()> {
    install_core_service_impl(paths)
}

pub(super) fn uninstall_core_service_elevated_body() -> AppResult<()> {
    uninstall_core_service_impl()
}

#[cfg(windows)]
fn install_core_service_impl(paths: &CoreServicePaths) -> AppResult<()> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, CreateServiceW, OpenServiceW, SC_MANAGER_CONNECT,
        SC_MANAGER_CREATE_SERVICE, SERVICE_ALL_ACCESS, SERVICE_CHANGE_CONFIG, SERVICE_DEMAND_START,
        SERVICE_ERROR_NORMAL, SERVICE_QUERY_STATUS, SERVICE_START, SERVICE_STOP,
        SERVICE_WIN32_OWN_PROCESS,
    };
    const ERROR_SERVICE_EXISTS: i32 = 1073;

    let manager = super::control::open_service_manager(SC_MANAGER_CREATE_SERVICE)?;
    let service_name = wide_null(CORE_SERVICE_NAME);
    let display_name = wide_null(CORE_SERVICE_DISPLAY_NAME);
    let command_line = wide_null(&air_platform::privilege::join_windows_args(
        &service_binary_args(paths)?,
    ));
    let handle = unsafe {
        CreateServiceW(
            manager,
            service_name.as_ptr(),
            display_name.as_ptr(),
            SERVICE_ALL_ACCESS,
            SERVICE_WIN32_OWN_PROCESS,
            SERVICE_DEMAND_START,
            SERVICE_ERROR_NORMAL,
            command_line.as_ptr(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    unsafe {
        CloseServiceHandle(manager);
    }
    if handle.is_null() {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_SERVICE_EXISTS) {
            let manager = super::control::open_service_manager(SC_MANAGER_CONNECT)?;
            let service = unsafe {
                OpenServiceW(
                    manager,
                    service_name.as_ptr(),
                    SERVICE_CHANGE_CONFIG | SERVICE_QUERY_STATUS | SERVICE_START | SERVICE_STOP,
                )
            };
            unsafe {
                CloseServiceHandle(manager);
            }
            if service.is_null() {
                return Err(PlatformError::OperationFailed(format!(
                    "内核服务已存在，但无法打开以更新权限: {}",
                    std::io::Error::last_os_error()
                ))
                .into());
            }
            let result = super::windows_acl::grant_interactive_users_service_control(service);
            unsafe {
                CloseServiceHandle(service);
            }
            return result;
        }
        return Err(PlatformError::OperationFailed(format!("创建内核服务失败: {error}")).into());
    }
    super::windows_acl::grant_interactive_users_service_control(handle)?;
    unsafe {
        CloseServiceHandle(handle);
    }
    Ok(())
}

#[cfg(not(windows))]
fn install_core_service_impl(_paths: &CoreServicePaths) -> AppResult<()> {
    super::non_windows::unsupported_windows_service()
}

#[cfg(windows)]
fn uninstall_core_service_impl() -> AppResult<()> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, DeleteService, OpenServiceW, SC_MANAGER_CONNECT, SERVICE_QUERY_STATUS,
        SERVICE_STOP,
    };

    let _ = stop_core_service();
    let manager = super::control::open_service_manager(SC_MANAGER_CONNECT)?;
    let service_name = wide_null(CORE_SERVICE_NAME);
    let delete_access = SERVICE_STOP
        | SERVICE_QUERY_STATUS
        | super::types::STANDARD_DELETE
        | super::types::STANDARD_READ_CONTROL;
    let mut service = unsafe { OpenServiceW(manager, service_name.as_ptr(), delete_access) };
    if service.is_null() {
        let open_error = std::io::Error::last_os_error();
        if open_error.raw_os_error()
            == Some(windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED as i32)
        {
            tracing::warn!(
                error = %open_error,
                service = CORE_SERVICE_NAME,
                "core service delete access denied; attempting ACL recovery"
            );
            super::windows_acl::recover_core_service_acl(manager, service_name.as_ptr())?;
            service = unsafe { OpenServiceW(manager, service_name.as_ptr(), delete_access) };
        }
    }
    unsafe { CloseServiceHandle(manager) };
    if service.is_null() {
        return Ok(());
    }
    let ok = unsafe { DeleteService(service) };
    unsafe {
        CloseServiceHandle(service);
    }
    if ok == 0 {
        return Err(PlatformError::OperationFailed(format!(
            "删除内核服务失败: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    Ok(())
}

#[cfg(not(windows))]
fn uninstall_core_service_impl() -> AppResult<()> {
    super::non_windows::unsupported_windows_service()
}
