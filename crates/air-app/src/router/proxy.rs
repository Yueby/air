use std::collections::BTreeMap;

use air_app::AppEvent;
use air_app::services::AppServices;
use air_config::ConfigDocument;
use air_error::{AppResult, ConfigError};
use air_mihomo::groups::ProxyGroupCollection;
use air_telemetry::redaction::redact_log_value;

use super::context::CommandExecutionContext;
use super::shared::{ensure_not_canceled, runtime_is_running};

pub(super) async fn handle_set_runtime_mode(
    context: &CommandExecutionContext,
    mode: String,
) -> AppResult<()> {
    ensure_not_canceled(&context.token)?;
    let mode = normalized_runtime_mode(&mode)?;
    context.services.save_runtime_mode(mode)?;
    if runtime_is_running(&context.services) {
        // mihomo 的运行模式是热更新项；本地通用配置先落盘，运行中的核心再通过 PATCH /configs 单字段同步。
        context
            .services
            .mihomo_clients
            .client()?
            .patch_configs(&serde_json::json!({ "mode": mode }))
            .await?;
        // 运行模式会改变 mihomo 暴露的可见代理组，尤其 global 模式只应显示 GLOBAL。
        // PATCH 成功后立即刷新投影，避免代理组页等到切换路由后才更新。
        refresh_proxy_group_projection_with_mode(&context.services, Some(mode)).await?;
    }
    Ok(())
}

pub(super) async fn handle_refresh_proxies(context: &CommandExecutionContext) -> AppResult<()> {
    refresh_proxy_group_projection(&context.services).await?;
    Ok(())
}

pub(super) async fn handle_select_proxy(
    context: &CommandExecutionContext,
    group: String,
    proxy: String,
) -> AppResult<()> {
    context
        .services
        .mihomo_clients
        .client()?
        .select_proxy(&group, &proxy)
        .await?;
    refresh_proxy_group_projection(&context.services).await
}

pub(super) async fn handle_test_proxy_delay(
    context: &CommandExecutionContext,
    name: String,
    timeout_ms: u64,
) -> AppResult<()> {
    let url = proxy_delay_test_url(&context.services)?;
    let delay = context
        .services
        .mihomo_clients
        .client()?
        .proxy_delay(&name, &url, timeout_ms)
        .await?;
    context.services.runtime.emit(AppEvent::ProxyDelayMeasured {
        name,
        delay_ms: delay.delay,
    });
    Ok(())
}

pub(super) async fn handle_test_proxy_group_delay(
    context: &CommandExecutionContext,
    name: String,
    timeout_ms: u64,
) -> AppResult<()> {
    let url = proxy_delay_test_url(&context.services)?;
    let delay = context
        .services
        .mihomo_clients
        .client()?
        .group_delay(&name, &url, timeout_ms)
        .await?;
    context
        .services
        .runtime
        .emit(AppEvent::ProxyGroupDelayMeasured {
            name,
            member_delays: delay.delays,
        });
    refresh_proxy_group_projection(&context.services).await
}

pub(super) async fn handle_clear_proxy_group_fixed(
    context: &CommandExecutionContext,
    name: String,
) -> AppResult<()> {
    context
        .services
        .mihomo_clients
        .client()?
        .clear_group_fixed(&name)
        .await?;
    refresh_proxy_group_projection(&context.services).await
}

pub(super) async fn refresh_proxy_group_projection(services: &AppServices) -> AppResult<()> {
    refresh_proxy_group_projection_with_mode(services, None).await
}

async fn refresh_proxy_group_projection_with_mode(
    services: &AppServices,
    mode_override: Option<&str>,
) -> AppResult<()> {
    let document = proxy_group_order_document(services)?;
    let collection = ProxyGroupCollection::from_document(&document.typed);
    let client = services.mihomo_clients.client()?;
    let configs = client.configs().await?;
    let proxies_response = client.proxies().await?;
    let groups = visible_runtime_proxy_groups(
        &proxies_response.proxies,
        mode_override
            .or_else(|| runtime_mode_from_configs(&configs.fields))
            .or_else(|| document.typed.global.mode.as_deref())
            .unwrap_or("rule"),
    );
    let projection = air_mihomo::groups::ProxyGroupRuntimeProjection {
        // `/proxies` 同时返回代理组和代理节点；app 层先划分类型，UI 只消费已经规整好的代理组投影。
        // 组顺序跟随内核 `GLOBAL.all` 权威顺序，与 Clash Verge / Sparkle 一致；订阅或运行配置文档
        // 只作为成员来源与配置组投影的解析锚点，不再作为组顺序的唯一依据。
        states: collection.selection_states_in_runtime_order(
            &document.typed,
            &groups,
            &proxies_response.proxies,
        ),
    };
    tracing::info!(
        groups = projection.states.len(),
        "refreshed proxy groups from mihomo runtime"
    );
    services
        .runtime
        .emit(AppEvent::ProxyGroupStateChanged(projection));
    Ok(())
}

fn proxy_group_order_document(services: &AppServices) -> AppResult<ConfigDocument> {
    if let Some(document) = services.selected_subscription_document()? {
        return Ok(document);
    }
    services
        .runtime_or_current_profile_document()?
        .ok_or_else(|| ConfigError::Validation("当前配置不存在，无法解析代理组顺序".into()).into())
}

fn runtime_mode_from_configs(fields: &BTreeMap<String, serde_json::Value>) -> Option<&str> {
    fields.get("mode").and_then(serde_json::Value::as_str)
}

fn visible_runtime_groups(
    groups: BTreeMap<String, air_mihomo::dto::ProxyResponse>,
    mode: &str,
) -> BTreeMap<String, air_mihomo::dto::ProxyResponse> {
    let global_mode = mode.trim().eq_ignore_ascii_case("global");
    groups
        .into_iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("GLOBAL") == global_mode)
        .collect()
}

fn visible_runtime_proxy_groups(
    proxies: &BTreeMap<String, air_mihomo::dto::ProxyResponse>,
    mode: &str,
) -> BTreeMap<String, air_mihomo::dto::ProxyResponse> {
    // mihomo `/proxies` 的代理组响应带有 `all` 成员列表，普通节点和内置策略没有成员列表。
    // 这里显式拆分，避免页面继续依赖 `/group` 或把节点误当成可选分组。
    let groups = proxies
        .iter()
        .filter(|(_, response)| is_proxy_group_response(response))
        .map(|(name, response)| (name.clone(), response.clone()))
        .collect::<BTreeMap<_, _>>();
    visible_runtime_groups(groups, mode)
}

fn is_proxy_group_response(response: &air_mihomo::dto::ProxyResponse) -> bool {
    if !response.all.is_empty() {
        return true;
    }
    matches!(
        response.kind.trim().to_ascii_lowercase().as_str(),
        "selector"
            | "select"
            | "urltest"
            | "url-test"
            | "fallback"
            | "loadbalance"
            | "load-balance"
            | "relay"
    )
}

fn normalized_runtime_mode(mode: &str) -> AppResult<&'static str> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "rule" => Ok("rule"),
        "global" => Ok("global"),
        "direct" => Ok("direct"),
        other => Err(ConfigError::Validation(format!("不支持的运行模式: {other}")).into()),
    }
}

fn proxy_delay_test_url(services: &AppServices) -> AppResult<String> {
    let settings = services.settings_store.load()?;
    let url = settings.normalized_proxy_delay_test_url();
    let parsed = url::Url::parse(url)
        .map_err(|error| ConfigError::Validation(format!("代理测速地址格式无效: {error}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(ConfigError::Validation("代理测速地址仅支持 http 或 https".into()).into());
    }
    tracing::info!(
        url = %redact_log_value(url),
        "using configured proxy delay test url"
    );
    Ok(url.to_string())
}
