// 子进程窗口策略集中放在 platform 层，避免 core/app 直接散落 Windows 专有标志。
// Windows GUI 进程启动 console 子程序时默认可能闪出控制台窗口；这里仅隐藏窗口，
// stdout/stderr 仍由调用方 pipe 到日志，不能影响诊断能力。

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn hide_tokio_subprocess_window(command: &mut tokio::process::Command) {
    apply_tokio_no_window(command);
}

pub fn hide_std_subprocess_window(command: &mut std::process::Command) {
    apply_std_no_window(command);
}

#[cfg(windows)]
fn apply_tokio_no_window(command: &mut tokio::process::Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn apply_tokio_no_window(_command: &mut tokio::process::Command) {}

#[cfg(windows)]
fn apply_std_no_window(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;

    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn apply_std_no_window(_command: &mut std::process::Command) {}
