use air_error::AppResult;
use air_mihomo::rules::RuleDisablePatch;

use super::context::CommandExecutionContext;
use super::shared::runtime_api_available;

pub(super) async fn handle_refresh_rules(context: &CommandExecutionContext) -> AppResult<()> {
    refresh_rules_snapshot(context).await
}

pub(super) async fn handle_disable_rule(
    context: &CommandExecutionContext,
    index: usize,
    disabled: bool,
) -> AppResult<()> {
    if !runtime_api_available(&context.services) {
        return Ok(());
    }
    // mihomo 的规则启停是纯运行态覆盖：请求体只提交索引到禁用布尔值的映射，
    // 成功后立即重新读取 `/rules`，让 UI 以核心返回的真实状态为准。
    let patch = RuleDisablePatch::from_iter([(index, disabled)]);
    context
        .services
        .mihomo_clients
        .client()?
        .disable_rules(&patch)
        .await?;
    refresh_rules_snapshot(context).await
}

pub(super) async fn handle_update_rule_provider(
    context: &CommandExecutionContext,
    name: String,
) -> AppResult<()> {
    context
        .services
        .mihomo_clients
        .client()?
        .update_rule_provider(&name)
        .await?;
    refresh_rules_snapshot(context).await
}

pub(super) async fn refresh_rules_snapshot(context: &CommandExecutionContext) -> AppResult<()> {
    if !runtime_api_available(&context.services) {
        return Ok(());
    }
    let response = context.services.mihomo_clients.client()?.rules().await?;
    context
        .services
        .runtime
        .emit(air_app::AppEvent::RulesStateChanged(response));
    Ok(())
}
