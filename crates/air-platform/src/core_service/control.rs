use air_error::{AppResult, PlatformError};

use super::args::{service_start_args, wide_null};
use super::types::{CORE_SERVICE_NAME, CoreServiceSnapshot};
pub fn query_core_service() -> AppResult<CoreServiceSnapshot> {
    query_core_service_impl()
}
pub fn start_core_service() -> AppResult<()> {
    start_core_service_impl(std::process::id())
}

pub fn stop_core_service() -> AppResult<()> {
    stop_core_service_impl()
}
fn open_service_for_start_error(error: std::io::Error) -> air_error::AppError {
    PlatformError::OperationFailed(format!(
        "无法打开内核服务用于启动，请确认服务已安装且当前用户有启动权限: {error}"
    ))
    .into()
}

#[cfg(windows)]
fn query_core_service_impl() -> AppResult<CoreServiceSnapshot> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, OpenServiceW, QueryServiceStatusEx, SC_MANAGER_CONNECT,
        SC_STATUS_PROCESS_INFO, SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_STATUS_PROCESS,
    };

    let manager = open_service_manager(SC_MANAGER_CONNECT)?;
    let service_name = wide_null(CORE_SERVICE_NAME);
    let service = unsafe { OpenServiceW(manager, service_name.as_ptr(), SERVICE_QUERY_STATUS) };
    unsafe {
        CloseServiceHandle(manager);
    }
    if service.is_null() {
        return Ok(CoreServiceSnapshot::default());
    }
    let mut status = std::mem::MaybeUninit::<SERVICE_STATUS_PROCESS>::zeroed();
    let mut needed = 0u32;
    let ok = unsafe {
        QueryServiceStatusEx(
            service,
            SC_STATUS_PROCESS_INFO,
            status.as_mut_ptr() as *mut u8,
            std::mem::size_of::<SERVICE_STATUS_PROCESS>() as u32,
            &mut needed,
        )
    };
    unsafe {
        CloseServiceHandle(service);
    }
    if ok == 0 {
        return Err(PlatformError::OperationFailed(format!(
            "查询内核服务状态失败: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    let status = unsafe { status.assume_init() };
    Ok(CoreServiceSnapshot {
        installed: true,
        running: status.dwCurrentState == SERVICE_RUNNING,
    })
}

#[cfg(not(windows))]
fn query_core_service_impl() -> AppResult<CoreServiceSnapshot> {
    Ok(CoreServiceSnapshot::default())
}

#[cfg(windows)]
fn start_core_service_impl(owner_pid: u32) -> AppResult<()> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, OpenServiceW, SC_MANAGER_CONNECT, SERVICE_QUERY_STATUS,
        SERVICE_RUNNING, SERVICE_START, SERVICE_STATUS_PROCESS, StartServiceW,
    };

    let manager = open_service_manager(SC_MANAGER_CONNECT)?;
    let service_name = wide_null(CORE_SERVICE_NAME);
    let service = unsafe {
        OpenServiceW(
            manager,
            service_name.as_ptr(),
            SERVICE_START | SERVICE_QUERY_STATUS,
        )
    };
    let open_error = std::io::Error::last_os_error();
    unsafe {
        CloseServiceHandle(manager);
    }
    if service.is_null() {
        return Err(open_service_for_start_error(open_error));
    }
    let service_args = service_start_args(owner_pid);
    let service_arg_wide = service_args
        .iter()
        .map(|arg| wide_null(arg))
        .collect::<Vec<_>>();
    let service_arg_ptrs = service_arg_wide
        .iter()
        .map(|arg| arg.as_ptr())
        .collect::<Vec<_>>();
    let ok = unsafe {
        StartServiceW(
            service,
            service_arg_ptrs.len() as u32,
            service_arg_ptrs.as_ptr(),
        )
    };
    if ok == 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(1056) {
            unsafe {
                CloseServiceHandle(service);
            }
            return Err(
                PlatformError::OperationFailed(format!("启动内核服务失败: {error}")).into(),
            );
        }
    }
    let status = wait_service_state(service, SERVICE_RUNNING, std::time::Duration::from_secs(8));
    unsafe {
        CloseServiceHandle(service);
    }
    status.map(|_: SERVICE_STATUS_PROCESS| ())
}

#[cfg(not(windows))]
fn start_core_service_impl(_owner_pid: u32) -> AppResult<()> {
    super::non_windows::unsupported_windows_service()
}

#[cfg(windows)]
fn stop_core_service_impl() -> AppResult<()> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, ControlService, OpenServiceW, SC_MANAGER_CONNECT, SERVICE_CONTROL_STOP,
        SERVICE_QUERY_STATUS, SERVICE_RUNNING, SERVICE_START_PENDING, SERVICE_STATUS, SERVICE_STOP,
        SERVICE_STOP_PENDING, SERVICE_STOPPED,
    };

    const ERROR_SERVICE_CANNOT_ACCEPT_CTRL: i32 = 1061;
    const ERROR_SERVICE_NOT_ACTIVE: i32 = 1062;
    const CORE_SERVICE_STOP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

    let manager = open_service_manager(SC_MANAGER_CONNECT)?;
    let service_name = wide_null(CORE_SERVICE_NAME);
    let service = unsafe {
        OpenServiceW(
            manager,
            service_name.as_ptr(),
            SERVICE_STOP | SERVICE_QUERY_STATUS,
        )
    };
    unsafe {
        CloseServiceHandle(manager);
    }
    if service.is_null() {
        return Ok(());
    }
    let current = query_service_state(service)?;
    if current.dwCurrentState == SERVICE_STOPPED {
        unsafe {
            CloseServiceHandle(service);
        }
        return Ok(());
    }
    if current.dwCurrentState == SERVICE_STOP_PENDING {
        let result = wait_service_state(service, SERVICE_STOPPED, CORE_SERVICE_STOP_TIMEOUT)
            .or_else(|error| {
                tracing::warn!(
                    %error,
                    "core service stayed stop-pending; terminating service process"
                );
                terminate_service_process(service)?;
                wait_service_state(service, SERVICE_STOPPED, std::time::Duration::from_secs(5))
            });
        unsafe {
            CloseServiceHandle(service);
        }
        return result.map(|_| ());
    }
    if current.dwCurrentState == SERVICE_START_PENDING {
        let result = wait_service_state(service, SERVICE_RUNNING, CORE_SERVICE_STOP_TIMEOUT)
            .or_else(|error| {
                tracing::warn!(
                    %error,
                    "core service stayed start-pending; terminating service process"
                );
                terminate_service_process(service)?;
                wait_service_state(service, SERVICE_STOPPED, std::time::Duration::from_secs(5))
            });
        match result {
            Ok(status) if status.dwCurrentState == SERVICE_STOPPED => {
                unsafe {
                    CloseServiceHandle(service);
                }
                return Ok(());
            }
            Ok(_) => {}
            Err(error) => {
                unsafe {
                    CloseServiceHandle(service);
                }
                return Err(error);
            }
        }
    }
    let mut status = std::mem::MaybeUninit::<SERVICE_STATUS>::zeroed();
    let ok = unsafe { ControlService(service, SERVICE_CONTROL_STOP, status.as_mut_ptr()) };
    if ok == 0 {
        let error = std::io::Error::last_os_error();
        if !matches!(
            error.raw_os_error(),
            Some(ERROR_SERVICE_NOT_ACTIVE | ERROR_SERVICE_CANNOT_ACCEPT_CTRL)
        ) {
            unsafe {
                CloseServiceHandle(service);
            }
            return Err(
                PlatformError::OperationFailed(format!("停止内核服务失败: {error}")).into(),
            );
        }
    }
    let result =
        wait_service_state(service, SERVICE_STOPPED, CORE_SERVICE_STOP_TIMEOUT).or_else(|error| {
            tracing::warn!(
                %error,
                "core service did not stop through service control; terminating service process"
            );
            terminate_service_process(service)?;
            wait_service_state(service, SERVICE_STOPPED, std::time::Duration::from_secs(5))
        });
    unsafe {
        CloseServiceHandle(service);
    }
    result.map(|_| ())
}

#[cfg(not(windows))]
fn stop_core_service_impl() -> AppResult<()> {
    super::non_windows::unsupported_windows_service()
}

#[cfg(windows)]
pub(super) fn open_service_manager(
    access: u32,
) -> AppResult<windows_sys::Win32::System::Services::SC_HANDLE> {
    use windows_sys::Win32::System::Services::OpenSCManagerW;

    let manager = unsafe { OpenSCManagerW(std::ptr::null(), std::ptr::null(), access) };
    if manager.is_null() {
        return Err(PlatformError::OperationFailed(format!(
            "打开服务管理器失败: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    Ok(manager)
}

#[cfg(windows)]
fn wait_service_state(
    service: windows_sys::Win32::System::Services::SC_HANDLE,
    expected: u32,
    timeout: std::time::Duration,
) -> AppResult<windows_sys::Win32::System::Services::SERVICE_STATUS_PROCESS> {
    use windows_sys::Win32::System::Services::{
        QueryServiceStatusEx, SC_STATUS_PROCESS_INFO, SERVICE_STATUS_PROCESS,
    };

    let start = std::time::Instant::now();
    loop {
        let mut status = std::mem::MaybeUninit::<SERVICE_STATUS_PROCESS>::zeroed();
        let mut needed = 0u32;
        let ok = unsafe {
            QueryServiceStatusEx(
                service,
                SC_STATUS_PROCESS_INFO,
                status.as_mut_ptr() as *mut u8,
                std::mem::size_of::<SERVICE_STATUS_PROCESS>() as u32,
                &mut needed,
            )
        };
        if ok == 0 {
            return Err(PlatformError::OperationFailed(format!(
                "等待内核服务状态失败: {}",
                std::io::Error::last_os_error()
            ))
            .into());
        }
        let status = unsafe { status.assume_init() };
        if status.dwCurrentState == expected {
            return Ok(status);
        }
        if start.elapsed() > timeout {
            return Err(PlatformError::OperationFailed("等待内核服务状态超时".into()).into());
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

#[cfg(windows)]
fn query_service_state(
    service: windows_sys::Win32::System::Services::SC_HANDLE,
) -> AppResult<windows_sys::Win32::System::Services::SERVICE_STATUS_PROCESS> {
    use windows_sys::Win32::System::Services::{
        QueryServiceStatusEx, SC_STATUS_PROCESS_INFO, SERVICE_STATUS_PROCESS,
    };

    let mut status = std::mem::MaybeUninit::<SERVICE_STATUS_PROCESS>::zeroed();
    let mut needed = 0u32;
    let ok = unsafe {
        QueryServiceStatusEx(
            service,
            SC_STATUS_PROCESS_INFO,
            status.as_mut_ptr() as *mut u8,
            std::mem::size_of::<SERVICE_STATUS_PROCESS>() as u32,
            &mut needed,
        )
    };
    if ok == 0 {
        return Err(PlatformError::OperationFailed(format!(
            "查询内核服务状态失败: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    Ok(unsafe { status.assume_init() })
}

#[cfg(windows)]
fn terminate_service_process(
    service: windows_sys::Win32::System::Services::SC_HANDLE,
) -> AppResult<()> {
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

    let status = query_service_state(service)?;
    if status.dwProcessId == 0 {
        return Ok(());
    }
    // 服务控制通道可能在 START_PENDING/STOP_PENDING 阶段拒绝 STOP(1061)。
    // 此时只能按 SCM 暴露的服务进程 PID 终止 Air 服务进程；服务进程持有的 JobObject
    // 会在句柄关闭时连带杀死 mihomo，避免 TUN 内核残留。
    let process = unsafe { OpenProcess(PROCESS_TERMINATE, 0, status.dwProcessId) };
    if process.is_null() {
        return Err(PlatformError::OperationFailed(format!(
            "打开内核服务进程失败: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    let ok = unsafe { TerminateProcess(process, 1) };
    unsafe {
        let _ = windows_sys::Win32::Foundation::CloseHandle(process);
    }
    if ok == 0 {
        return Err(PlatformError::OperationFailed(format!(
            "终止内核服务进程失败: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    Ok(())
}
