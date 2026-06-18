use std::path::PathBuf;

use air_app::AppEvent;
use air_app::events::AppNotificationLevel;
use air_error::{AppResult, ConfigError, RuntimeError};
use air_mihomo::subscriptions::SubscriptionSource;

use super::context::CommandExecutionContext;
use super::proxy::refresh_proxy_group_projection;
use super::shared::{
    current_core_version, ensure_not_canceled, reload_runtime_config_if_running,
    runtime_is_running, wait_for_cancellation,
};

pub(super) async fn handle_update_subscription(
    context: &CommandExecutionContext,
    subscription_id: String,
) -> AppResult<()> {
    ensure_not_canceled(&context.token)?;
    let source = context
        .services
        .subscription_store
        .load_sources()?
        .into_iter()
        .find(|source| source.id == subscription_id)
        .ok_or_else(|| ConfigError::Subscription(format!("订阅源不存在: {subscription_id}")))?;
    let controller = context.services.subscription_controller();
    let core_version = current_core_version(&context.services).await?;
    let update = controller.refresh_cache_with_core_version(&source, core_version.as_deref());
    tokio::pin!(update);
    let projection = tokio::select! {
        _ = wait_for_cancellation(context.token.clone()) => {
            let projection = controller.mark_canceled(&subscription_id)?;
            context.services.runtime.emit(AppEvent::SubscriptionUpdateCanceled {
                subscription_id: subscription_id.clone(),
            });
            context.services.runtime.emit(AppEvent::SubscriptionStateChanged(projection));
            context.cancellations.remove(&format!("subscription:{subscription_id}"));
            return Err(RuntimeError::Canceled.into());
        }
        result = &mut update => result?,
    };
    context
        .services
        .runtime
        .emit(AppEvent::SubscriptionStateChanged(projection));
    context
        .services
        .emit_notification(AppNotificationLevel::Success, "订阅已更新");
    context
        .cancellations
        .remove(&format!("subscription:{subscription_id}"));
    Ok(())
}

pub(super) async fn handle_refresh_due_subscriptions(
    context: &CommandExecutionContext,
) -> AppResult<()> {
    ensure_not_canceled(&context.token)?;
    let controller = context.services.subscription_controller();
    let due_sources = controller.due_sources()?;
    if due_sources.is_empty() {
        return Ok(());
    }
    let core_version = current_core_version(&context.services).await?;
    for source in due_sources {
        ensure_not_canceled(&context.token)?;
        if let Err(error) = controller
            .update_with_core_version(&source, core_version.as_deref())
            .await
        {
            // 定时更新失败已经写入订阅缓存元数据；这里不中断后续订阅，避免一个失效源阻塞其他源。
            tracing::warn!(
                subscription_id = %source.id,
                error = %error,
                "scheduled subscription update failed"
            );
        }
    }
    context.services.emit_subscription_projection()?;
    context.cancellations.remove("subscriptions-scheduler");
    Ok(())
}

pub(super) async fn handle_save_subscription_source(
    context: &CommandExecutionContext,
    source: SubscriptionSource,
) -> AppResult<()> {
    ensure_not_canceled(&context.token)?;
    let projection = context
        .services
        .subscription_controller()
        .save_source(source)?;
    context
        .services
        .runtime
        .emit(AppEvent::SubscriptionStateChanged(projection));
    context
        .services
        .emit_notification(AppNotificationLevel::Success, "订阅已保存");
    Ok(())
}

pub(super) async fn handle_load_subscription_yaml(
    context: &CommandExecutionContext,
    subscription_id: String,
) -> AppResult<()> {
    ensure_not_canceled(&context.token)?;
    let contents = context
        .services
        .subscription_controller()
        .cached_yaml(&subscription_id)?;
    context
        .services
        .runtime
        .emit(AppEvent::SubscriptionYamlLoaded {
            subscription_id,
            contents,
        });
    Ok(())
}

pub(super) async fn handle_reorder_subscriptions(
    context: &CommandExecutionContext,
    ordered_ids: Vec<String>,
) -> AppResult<()> {
    ensure_not_canceled(&context.token)?;
    let projection = context
        .services
        .subscription_controller()
        .reorder_sources(&ordered_ids)?;
    context
        .services
        .runtime
        .emit(AppEvent::SubscriptionStateChanged(projection));
    Ok(())
}

pub(super) async fn handle_select_subscription(
    context: &CommandExecutionContext,
    subscription_id: String,
) -> AppResult<()> {
    ensure_not_canceled(&context.token)?;
    let projection = context
        .services
        .subscription_controller()
        .select(&subscription_id)?;
    context
        .services
        .runtime
        .emit(AppEvent::SubscriptionStateChanged(projection));
    reload_runtime_config_if_running(&context.services).await?;
    if runtime_is_running(&context.services) {
        // 切换激活订阅后运行配置已经重载；立即刷新代理组投影，避免页面继续显示旧订阅的组。
        refresh_proxy_group_projection(&context.services).await?;
    }
    context
        .services
        .emit_notification(AppNotificationLevel::Success, "订阅已选中");
    Ok(())
}

pub(super) async fn handle_import_subscription_url(
    context: &CommandExecutionContext,
    subscription_id: String,
    url: String,
) -> AppResult<()> {
    let core_version = current_core_version(&context.services).await?;
    let projection = context
        .services
        .subscription_controller()
        .import_url_with_core_version(subscription_id, url, core_version.as_deref())
        .await?;
    context
        .services
        .runtime
        .emit(AppEvent::SubscriptionStateChanged(projection));
    context
        .services
        .emit_notification(AppNotificationLevel::Success, "订阅链接已导入");
    Ok(())
}

pub(super) async fn handle_import_subscription_file(
    context: &CommandExecutionContext,
    path: PathBuf,
) -> AppResult<()> {
    let projection = context
        .services
        .subscription_controller()
        .import_file(&path)?;
    context
        .services
        .runtime
        .emit(AppEvent::SubscriptionStateChanged(projection));
    context
        .services
        .emit_notification(AppNotificationLevel::Success, "YAML 订阅文件已导入");
    Ok(())
}

pub(super) async fn handle_delete_subscription(
    context: &CommandExecutionContext,
    subscription_id: String,
) -> AppResult<()> {
    ensure_not_canceled(&context.token)?;
    let projection = context
        .services
        .subscription_controller()
        .delete(&subscription_id)?;
    context
        .services
        .runtime
        .emit(AppEvent::SubscriptionStateChanged(projection));
    context
        .services
        .emit_notification(AppNotificationLevel::Success, "订阅已删除");
    Ok(())
}

pub(super) async fn handle_cancel_subscription_update(
    context: &CommandExecutionContext,
    subscription_id: String,
) -> AppResult<()> {
    context
        .cancellations
        .cancel(&format!("subscription:{subscription_id}"));
    Ok(())
}
