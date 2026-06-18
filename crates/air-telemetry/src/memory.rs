pub fn shrink_process_memory(reason: &'static str) {
    force_mimalloc_collect();
    trim_platform_working_set(reason);
}

fn force_mimalloc_collect() {
    unsafe {
        // 低内存边界只在托盘隐藏等明确时机触发；强制 collect 会回收 mimalloc
        // 已释放的线程堆和空闲页，但不能释放仍被 Rust 对象持有的业务数据。
        libmimalloc_sys::mi_collect(true);
    }
}

#[cfg(target_os = "windows")]
fn trim_platform_working_set(reason: &'static str) {
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};

    let ok = unsafe {
        // Windows 将 -1/-1 解释为尽量清空当前进程工作集；这只影响常驻物理页，
        // 不改变仍然提交或仍被对象持有的虚拟内存，适合隐藏到托盘后的极低内存目标。
        SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX)
    };
    if ok == 0 {
        let error = unsafe { GetLastError() };
        tracing::warn!(reason, error, "failed to trim current process working set");
    } else {
        tracing::info!(reason, "process memory shrink requested");
    }
}

#[cfg(not(target_os = "windows"))]
fn trim_platform_working_set(reason: &'static str) {
    tracing::info!(
        reason,
        "process memory shrink requested; platform working-set trim is unavailable"
    );
}
