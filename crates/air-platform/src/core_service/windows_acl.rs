use air_error::{AppResult, PlatformError};

use super::args::wide_null;
use super::types::{
    CORE_SERVICE_NAME, SERVICE_ADMIN_RIGHTS_SDDL, SERVICE_INTERACTIVE_USER_RIGHTS_SDDL,
    STANDARD_WRITE_DAC, STANDARD_WRITE_OWNER,
};
#[cfg(windows)]
pub(super) fn grant_interactive_users_service_control(
    service: windows_sys::Win32::System::Services::SC_HANDLE,
) -> AppResult<()> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};
    use windows_sys::Win32::System::Services::SetServiceObjectSecurity;

    // 服务由管理员安装，但 GUI 以普通权限运行：交互用户/Users 只保留查询、启动、停止能力；
    // SYSTEM/Administrators 保留删除和改 ACL 等维护权限，避免服务被旧 DACL 锁成不可卸载状态。
    let sddl = wide_null(&core_service_security_sddl(None));
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 || descriptor.is_null() {
        return Err(PlatformError::OperationFailed(format!(
            "生成内核服务权限描述符失败: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    let ok = unsafe { SetServiceObjectSecurity(service, DACL_SECURITY_INFORMATION, descriptor) };
    unsafe {
        LocalFree(descriptor as _);
    }
    if ok == 0 {
        return Err(PlatformError::OperationFailed(format!(
            "更新内核服务权限失败: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    Ok(())
}

#[cfg(windows)]
pub(super) fn recover_core_service_acl(
    manager: windows_sys::Win32::System::Services::SC_HANDLE,
    service_name: *const u16,
) -> AppResult<()> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, OpenServiceW, SERVICE_QUERY_STATUS,
    };

    let service = unsafe {
        OpenServiceW(
            manager,
            service_name,
            STANDARD_WRITE_DAC | SERVICE_QUERY_STATUS,
        )
    };
    if !service.is_null() {
        let result = grant_interactive_users_service_control(service);
        unsafe { CloseServiceHandle(service) };
        return result;
    }

    let direct_error = std::io::Error::last_os_error();
    tracing::warn!(
        error = %direct_error,
        service = CORE_SERVICE_NAME,
        "opening core service for DACL repair failed; attempting ownership recovery"
    );
    enable_take_ownership_privilege()?;
    let service = unsafe { OpenServiceW(manager, service_name, STANDARD_WRITE_OWNER) };
    if service.is_null() {
        return Err(PlatformError::OperationFailed(format!(
            "修复内核服务权限失败，无法接管服务对象: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    let owner_result = set_core_service_owner_to_administrators(service);
    unsafe { CloseServiceHandle(service) };
    owner_result?;

    let service = unsafe {
        OpenServiceW(
            manager,
            service_name,
            STANDARD_WRITE_DAC | SERVICE_QUERY_STATUS,
        )
    };
    if service.is_null() {
        return Err(PlatformError::OperationFailed(format!(
            "修复内核服务权限失败，接管后仍无法写入 DACL: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    let result = grant_interactive_users_service_control(service);
    unsafe { CloseServiceHandle(service) };
    result
}

#[cfg(windows)]
fn enable_take_ownership_privilege() -> AppResult<()> {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, LUID, SetLastError};
    use windows_sys::Win32::Security::{
        AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED,
        SE_TAKE_OWNERSHIP_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token = std::ptr::null_mut();
    let ok = unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
    };
    if ok == 0 || token.is_null() {
        return Err(PlatformError::OperationFailed(format!(
            "打开当前进程令牌失败，无法修复内核服务权限: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }

    let mut luid = LUID::default();
    let ok = unsafe { LookupPrivilegeValueW(std::ptr::null(), SE_TAKE_OWNERSHIP_NAME, &mut luid) };
    if ok == 0 {
        unsafe { CloseHandle(token) };
        return Err(PlatformError::OperationFailed(format!(
            "查询 SeTakeOwnershipPrivilege 失败: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }

    let privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };
    unsafe { SetLastError(0) };
    let ok = unsafe {
        AdjustTokenPrivileges(
            token,
            0,
            &privileges,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    let last_error = unsafe { GetLastError() };
    unsafe { CloseHandle(token) };
    if ok == 0 || last_error == windows_sys::Win32::Foundation::ERROR_NOT_ALL_ASSIGNED {
        return Err(PlatformError::OperationFailed(format!(
            "启用 SeTakeOwnershipPrivilege 失败: {}",
            std::io::Error::from_raw_os_error(last_error as i32)
        ))
        .into());
    }
    Ok(())
}

#[cfg(windows)]
fn set_core_service_owner_to_administrators(
    service: windows_sys::Win32::System::Services::SC_HANDLE,
) -> AppResult<()> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows_sys::Win32::Security::{OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};
    use windows_sys::Win32::System::Services::SetServiceObjectSecurity;

    // 接管只用于旧 ACL 把管理员的 WRITE_DAC/DELETE 锁掉的场景。所有者改成 Administrators 后，
    // 立刻写回统一 DACL，避免留下后续仍不可维护的服务安全描述符。
    let sddl = wide_null(&core_service_security_sddl(Some("BA")));
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 || descriptor.is_null() {
        return Err(PlatformError::OperationFailed(format!(
            "生成内核服务所有者描述符失败: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    let ok = unsafe { SetServiceObjectSecurity(service, OWNER_SECURITY_INFORMATION, descriptor) };
    unsafe {
        LocalFree(descriptor as _);
    }
    if ok == 0 {
        return Err(PlatformError::OperationFailed(format!(
            "接管内核服务对象失败: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    Ok(())
}

pub(super) fn core_service_security_sddl(owner: Option<&str>) -> String {
    let owner = owner.map(|sid| format!("O:{sid}")).unwrap_or_default();
    format!(
        "{owner}D:(A;;{admin};;;SY)(A;;{admin};;;BA)(A;;{user};;;IU)(A;;{user};;;BU)",
        admin = SERVICE_ADMIN_RIGHTS_SDDL,
        user = SERVICE_INTERACTIVE_USER_RIGHTS_SDDL
    )
}
