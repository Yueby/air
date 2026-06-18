use air_app::AppEvent;
use air_error::AppResult;

use super::context::CommandExecutionContext;
use super::shared::runtime_api_available;

pub(super) async fn handle_refresh_connections(context: &CommandExecutionContext) -> AppResult<()> {
    refresh_connections_snapshot(context).await
}

pub(super) async fn handle_close_connection(
    context: &CommandExecutionContext,
    id: String,
) -> AppResult<()> {
    if !runtime_api_available(&context.services) {
        return Ok(());
    }
    context
        .services
        .mihomo_clients
        .client()?
        .close_connection(&id)
        .await?;
    refresh_connections_snapshot(context).await
}

pub(super) async fn handle_close_all_connections(
    context: &CommandExecutionContext,
) -> AppResult<()> {
    if !runtime_api_available(&context.services) {
        return Ok(());
    }
    context
        .services
        .mihomo_clients
        .client()?
        .close_all_connections()
        .await?;
    refresh_connections_snapshot(context).await
}

async fn refresh_connections_snapshot(context: &CommandExecutionContext) -> AppResult<()> {
    if !runtime_api_available(&context.services) {
        return Ok(());
    }
    let response = context
        .services
        .mihomo_clients
        .client()?
        .connections()
        .await?;
    // 连接页的 HTTP 刷新和流式连接事件共用 UI reducer，但一次性刷新需要保留完整响应，
    // 因此在 app 层显式发事件，避免 command result 只能表达成功/失败而丢失响应体。
    context
        .services
        .runtime
        .emit(AppEvent::ConnectionsStateChanged(response));
    Ok(())
}
