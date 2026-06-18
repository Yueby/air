use air_error::{AppResult, PlatformError};

pub(super) fn unsupported_windows_service<T>() -> AppResult<T> {
    Err(PlatformError::Unsupported("当前平台不支持 Windows 服务".into()).into())
}

pub(super) fn unsupported_windows_service_elevation<T>() -> AppResult<T> {
    Err(PlatformError::Unsupported("当前平台不支持 Windows 服务提权操作".into()).into())
}
