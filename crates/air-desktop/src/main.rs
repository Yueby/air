#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mimalloc::MiMalloc;

// 桌面主进程统一使用 mimalloc，覆盖 GUI、后台 Tokio runtime 和 mihomo 管理层的常规堆分配。
// 只在 binary 入口声明全局 allocator，避免库代码被测试或外部复用时强行接管宿主进程策略。
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    // main 只负责顶层启动和错误展示，具体装配留在库代码中，便于后续 GUI 与测试复用。
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> air_error::AppResult<()> {
    if air_app::run_service_entrypoints()? {
        return Ok(());
    }
    if let Some((force_start_core, single_instance_events)) = air_app::prepare_gui_launch()? {
        air_ui::launch(force_start_core, single_instance_events);
    }
    Ok(())
}
