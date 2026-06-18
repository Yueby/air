use std::path::{Path, PathBuf};

use air_error::{AppResult, PlatformError};

use super::args::{
    service_args_from_argv, service_owner_pid_from_args, service_paths_from_args, wide_null,
};
use super::types::CORE_SERVICE_NAME;
pub fn run_core_service_from_env() -> AppResult<()> {
    run_core_service_impl()
}
#[cfg(windows)]
fn run_core_service_impl() -> AppResult<()> {
    use windows_sys::Win32::System::Services::{SERVICE_TABLE_ENTRYW, StartServiceCtrlDispatcherW};

    let mut service_name = wide_null(CORE_SERVICE_NAME);
    let mut table = [
        SERVICE_TABLE_ENTRYW {
            lpServiceName: service_name.as_mut_ptr(),
            lpServiceProc: Some(service_main),
        },
        SERVICE_TABLE_ENTRYW {
            lpServiceName: std::ptr::null_mut(),
            lpServiceProc: None,
        },
    ];
    let ok = unsafe { StartServiceCtrlDispatcherW(table.as_mut_ptr()) };
    if ok == 0 {
        return Err(PlatformError::OperationFailed(format!(
            "启动内核服务调度器失败: {}",
            std::io::Error::last_os_error()
        ))
        .into());
    }
    Ok(())
}

#[cfg(not(windows))]
fn run_core_service_impl() -> AppResult<()> {
    super::non_windows::unsupported_windows_service()
}

#[cfg(windows)]
static SERVICE_STOP_REQUESTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(windows)]
static SERVICE_CHILD: std::sync::Mutex<Option<ServiceChild>> = std::sync::Mutex::new(None);

#[cfg(windows)]
struct ServiceChild {
    process: Option<std::process::Child>,
    _job: Option<JobHandle>,
}

#[cfg(windows)]
struct JobHandle(isize);

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = windows_sys::Win32::Foundation::CloseHandle(
                self.0 as windows_sys::Win32::Foundation::HANDLE,
            );
        }
    }
}

fn assign_process_to_kill_on_close_job(
    child: &std::process::Child,
    label: &'static str,
) -> Option<JobHandle> {
    use std::mem::{size_of, zeroed};
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
        tracing::warn!(label, "failed to create kill-on-close job object");
        return None;
    }
    let handle = JobHandle(job as isize);
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    let configured = unsafe {
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if configured == 0 {
        tracing::warn!(label, "failed to configure kill-on-close job object");
    }
    let assigned = unsafe { AssignProcessToJobObject(job, child.as_raw_handle() as _) };
    if assigned == 0 {
        tracing::warn!(
            label,
            "failed to assign process to kill-on-close job object"
        );
    }
    Some(handle)
}

#[cfg(windows)]
extern "system" fn service_main(argc: u32, argv: *mut *mut u16) {
    use windows_sys::Win32::System::Services::RegisterServiceCtrlHandlerExW;

    let name = wide_null(CORE_SERVICE_NAME);
    let handle = unsafe {
        RegisterServiceCtrlHandlerExW(
            name.as_ptr(),
            Some(service_control_handler),
            std::ptr::null_mut(),
        )
    };
    if handle.is_null() {
        return;
    }
    SERVICE_STOP_REQUESTED.store(false, std::sync::atomic::Ordering::SeqCst);
    let service_args = unsafe { service_args_from_argv(argc, argv) };
    let owner_pid = service_owner_pid_from_args(&service_args);
    let exit_code = match run_service_worker(handle, owner_pid) {
        Ok(()) => 0,
        Err(error) => {
            append_service_log(None, "stderr", &format!("air core service failed: {error}"));
            1
        }
    };
    set_service_status(
        handle,
        windows_sys::Win32::System::Services::SERVICE_STOPPED,
        exit_code,
    );
}

#[cfg(windows)]
extern "system" fn service_control_handler(
    control: u32,
    _event_type: u32,
    _event_data: *mut std::ffi::c_void,
    _context: *mut std::ffi::c_void,
) -> u32 {
    use windows_sys::Win32::System::Services::{SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP};

    if control == SERVICE_CONTROL_STOP || control == SERVICE_CONTROL_SHUTDOWN {
        SERVICE_STOP_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Ok(mut child) = SERVICE_CHILD.lock() {
            if let Some(child) = child.as_mut() {
                if let Some(process) = child.process.as_mut() {
                    let _ = process.kill();
                }
            }
        }
    }
    0
}

#[cfg(windows)]
fn run_service_worker(
    handle: windows_sys::Win32::System::Services::SERVICE_STATUS_HANDLE,
    owner_pid: Option<u32>,
) -> AppResult<()> {
    use std::process::{Command, Stdio};

    use windows_sys::Win32::System::Services::{
        SERVICE_RUNNING, SERVICE_START_PENDING, SERVICE_STOP_PENDING,
    };

    set_service_status(handle, SERVICE_START_PENDING, 0);
    let paths = service_paths_from_args()?;
    paths.init()?;
    let binary_path = paths.cores_dir.join("mihomo.exe");
    let config_path = paths.config_dir.join("core.runtime.config.yaml");
    let log_path = paths.logs_dir.join("core.log");
    if !binary_path.is_file() {
        return Err(PlatformError::OperationFailed(format!(
            "内核服务未找到 mihomo: {}",
            binary_path.display()
        ))
        .into());
    }
    if !config_path.is_file() {
        return Err(PlatformError::OperationFailed(format!(
            "内核服务未找到运行配置: {}",
            config_path.display()
        ))
        .into());
    }

    append_service_log(
        Some(&log_path),
        "stdout",
        "air core service starting mihomo",
    );
    let mut command = Command::new(&binary_path);
    command
        .args([
            "-d",
            &paths.cores_dir.to_string_lossy(),
            "-f",
            &config_path.to_string_lossy(),
        ])
        .current_dir(&paths.cores_dir)
        .env(
            "SAFE_PATHS",
            join_safe_paths(&[
                paths.config_dir.clone(),
                paths.data_dir.clone(),
                paths.cache_dir.clone(),
            ]),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    air_platform::process::hide_std_subprocess_window(&mut command);
    let mut child = command.spawn().map_err(|error| {
        PlatformError::OperationFailed(format!("内核服务启动 mihomo 失败: {error}"))
    })?;
    let child_job = assign_process_to_kill_on_close_job(&child, "core service");
    let stdout = child
        .stdout
        .take()
        .map(|stream| service_log_reader(stream, log_path.clone(), "stdout"));
    let stderr = child
        .stderr
        .take()
        .map(|stream| service_log_reader(stream, log_path.clone(), "stderr"));
    {
        let mut slot = SERVICE_CHILD
            .lock()
            .map_err(|_| PlatformError::OperationFailed("内核服务子进程锁已损坏".into()))?;
        *slot = Some(ServiceChild {
            process: Some(child),
            _job: child_job,
        });
    }
    let _owner_monitor = owner_pid.map(|pid| spawn_owner_process_monitor(pid, log_path.clone()));
    set_service_status(handle, SERVICE_RUNNING, 0);

    loop {
        if SERVICE_STOP_REQUESTED.load(std::sync::atomic::Ordering::SeqCst) {
            set_service_status(handle, SERVICE_STOP_PENDING, 0);
            break;
        }
        let exited = {
            let mut slot = SERVICE_CHILD
                .lock()
                .map_err(|_| PlatformError::OperationFailed("内核服务子进程锁已损坏".into()))?;
            if let Some(child) = slot.as_mut() {
                child
                    .process
                    .as_mut()
                    .map(|process| {
                        process.try_wait().map_err(|error| {
                            PlatformError::OperationFailed(format!(
                                "读取 mihomo 退出状态失败: {error}"
                            ))
                        })
                    })
                    .transpose()?
                    .flatten()
            } else {
                None
            }
        };
        if let Some(status) = exited {
            append_service_log(
                Some(&log_path),
                "stdout",
                &format!(
                    "air core service observed mihomo exit during managed runtime: {:?}",
                    status.code()
                ),
            );
            // mihomo 的 /restart 可能让原进程退出并自行拉起新进程。这里不能结束服务或释放
            // JobObject，否则新进程若仍在同一个 JobObject 中，会被 kill-on-close 一并杀掉。
            if let Ok(mut slot) = SERVICE_CHILD.lock() {
                if let Some(child) = slot.as_mut() {
                    if let Some(mut process) = child.process.take() {
                        let _ = process.wait();
                    }
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    if let Ok(mut slot) = SERVICE_CHILD.lock() {
        if let Some(mut child) = slot.take() {
            if let Some(process) = child.process.as_mut() {
                let _ = process.kill();
                let _ = process.wait();
            }
        }
    }
    if let Some(thread) = stdout {
        let _ = thread.join();
    }
    if let Some(thread) = stderr {
        let _ = thread.join();
    }
    Ok(())
}

#[cfg(windows)]
fn spawn_owner_process_monitor(owner_pid: u32, log_path: PathBuf) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use windows_sys::Win32::Foundation::{CloseHandle, WAIT_FAILED, WAIT_OBJECT_0};
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
        };

        append_service_log(
            Some(&log_path),
            "stdout",
            &format!("air core service tracking GUI owner pid {owner_pid}"),
        );
        let owner = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, owner_pid) };
        if owner.is_null() {
            append_service_log(
                Some(&log_path),
                "stderr",
                &format!("owner pid {owner_pid} is unavailable; stopping service-managed core"),
            );
            request_service_stop_from_owner();
            return;
        }
        let wait = unsafe { WaitForSingleObject(owner, u32::MAX) };
        unsafe {
            CloseHandle(owner);
        }
        if wait == WAIT_OBJECT_0 {
            append_service_log(
                Some(&log_path),
                "stdout",
                &format!("owner pid {owner_pid} exited; stopping service-managed core"),
            );
            request_service_stop_from_owner();
        } else if wait == WAIT_FAILED {
            append_service_log(
                Some(&log_path),
                "stderr",
                &format!(
                    "waiting owner pid {owner_pid} failed: {}",
                    std::io::Error::last_os_error()
                ),
            );
            request_service_stop_from_owner();
        }
    })
}

#[cfg(windows)]
fn request_service_stop_from_owner() {
    // GUI 被任务管理器强杀时不会执行正常退出钩子；服务以 owner PID 作为生命期边界，
    // 一旦 owner 消失就请求自身停止并终止 mihomo，避免 TUN 服务长期残留在后台。
    SERVICE_STOP_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
    if let Ok(mut child) = SERVICE_CHILD.lock() {
        if let Some(child) = child.as_mut() {
            if let Some(process) = child.process.as_mut() {
                let _ = process.kill();
            }
        }
    }
}

#[cfg(windows)]
fn set_service_status(
    handle: windows_sys::Win32::System::Services::SERVICE_STATUS_HANDLE,
    state: u32,
    exit_code: u32,
) {
    use windows_sys::Win32::System::Services::{
        SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP, SERVICE_START_PENDING, SERVICE_STATUS,
        SERVICE_STOPPED, SERVICE_WIN32_OWN_PROCESS, SetServiceStatus,
    };

    let controls = if state == SERVICE_START_PENDING || state == SERVICE_STOPPED {
        0
    } else {
        SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN
    };
    let mut status = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: state,
        dwControlsAccepted: controls,
        dwWin32ExitCode: exit_code,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: 3000,
    };
    unsafe {
        SetServiceStatus(handle, &mut status);
    }
}

#[cfg(windows)]
fn service_log_reader<T>(
    stream: T,
    console_log_path: PathBuf,
    stream_name: &'static str,
) -> std::thread::JoinHandle<()>
where
    T: std::io::Read + Send + 'static,
{
    use std::io::BufRead;

    std::thread::spawn(move || {
        let reader = std::io::BufReader::new(stream);
        for line in reader.lines().map_while(Result::ok) {
            append_service_log(
                Some(&console_log_path),
                stream_name,
                &air_telemetry::redaction::redact_log_value(&line),
            );
        }
    })
}

#[cfg(windows)]
fn append_service_log(path: Option<&Path>, stream: &str, line: &str) {
    use std::io::Write;

    let Some(path) = path else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(error) = air_telemetry::log_retention::prepare_managed_log_for_append(path) {
        tracing::warn!(%error, path = %path.display(), "failed to prepare core service log");
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(
            file,
            "[{}][{stream}] {line}",
            air_telemetry::log_retention::format_current_log_timestamp()
        );
    }
}

#[cfg(windows)]
fn join_safe_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(";")
}
