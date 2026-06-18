use std::path::PathBuf;

use air_app::AppEvent;
use air_app::events::AppNotificationLevel;
use air_error::AppResult;

use super::context::CommandExecutionContext;
use super::shared::{
    reload_current_runtime_config_if_running, reload_runtime_config_if_running, runtime_is_running,
};

pub(super) async fn handle_load_profile(
    context: &CommandExecutionContext,
    path: PathBuf,
) -> AppResult<()> {
    context
        .services
        .import_profile_file_validated(&path)
        .await?;
    context
        .services
        .emit_notification(AppNotificationLevel::Success, "Profile 已加载并完成校验");
    Ok(())
}

pub(super) async fn handle_save_config(
    context: &CommandExecutionContext,
    profile: String,
) -> AppResult<()> {
    context
        .services
        .save_current_config_validated(&profile)
        .await?;
    if runtime_is_running(&context.services) {
        reload_runtime_config_if_running(&context.services).await?;
        context
            .services
            .emit_notification(AppNotificationLevel::Success, "配置已保存，运行配置已重载");
        return Ok(());
    }
    context
        .services
        .emit_notification(AppNotificationLevel::Success, "配置已保存");
    Ok(())
}

pub(super) async fn handle_set_override_script_enabled(
    context: &CommandExecutionContext,
    enabled: bool,
) -> AppResult<()> {
    context.services.set_override_script_enabled(enabled)?;
    // 激活状态变更会改变下一份 runtime 配置的真实内容；立即重写并在核心运行中重载，
    // 避免 UI 开关状态和 mihomo 实际配置长时间不一致。
    context.services.write_runtime_config_validated().await?;
    reload_current_runtime_config_if_running(&context.services).await?;
    context.services.emit_notification(
        AppNotificationLevel::Success,
        if enabled {
            "覆写脚本已激活"
        } else {
            "覆写脚本已禁用"
        },
    );
    Ok(())
}

pub(super) async fn handle_save_override_script(
    context: &CommandExecutionContext,
    script: String,
    enabled: bool,
) -> AppResult<()> {
    context.services.save_override_script(&script, enabled)?;
    if enabled {
        context.services.write_runtime_config_validated().await?;
        reload_current_runtime_config_if_running(&context.services).await?;
        context.services.emit_notification(
            AppNotificationLevel::Success,
            "覆写脚本已保存，运行配置已重载",
        );
    } else {
        context
            .services
            .emit_notification(AppNotificationLevel::Success, "覆写脚本已保存");
    }
    Ok(())
}

pub(super) async fn handle_debug_override_script(
    context: &CommandExecutionContext,
    script: String,
) -> AppResult<()> {
    let contents = context.services.preview_override_script(&script)?;
    context
        .services
        .runtime
        .emit(AppEvent::OverridePreviewGenerated { contents });
    Ok(())
}
