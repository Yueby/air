use air_mihomo::subscriptions::{
    SubscriptionCacheMetadata, SubscriptionSource, SubscriptionUpdateOutcome,
    SubscriptionUpdateResult,
};
use air_ui::shell::ShellPalette;

use super::format::*;
use super::render::SubscriptionCacheState;
use super::state::SubscriptionUsageView;
pub(super) fn cache_state(
    cache: Option<&SubscriptionCacheMetadata>,
    last_update: Option<&SubscriptionUpdateResult>,
) -> SubscriptionCacheState {
    let Some(cache) = cache else {
        return SubscriptionCacheState::Empty;
    };
    if matches!(
        last_update.map(|result| result.outcome),
        Some(SubscriptionUpdateOutcome::Failed)
    ) {
        if cache.last_success_at.is_some() {
            SubscriptionCacheState::StaleAfterFailure
        } else {
            SubscriptionCacheState::FailedNoCache
        }
    } else if cache.last_success_at.is_some() {
        SubscriptionCacheState::Ready
    } else {
        SubscriptionCacheState::Empty
    }
}

pub(super) fn cache_label(
    cache: Option<&SubscriptionCacheMetadata>,
    last_update: Option<&SubscriptionUpdateResult>,
    state: SubscriptionCacheState,
) -> String {
    if matches!(
        last_update.map(|result| result.outcome),
        Some(SubscriptionUpdateOutcome::Canceled)
    ) {
        return "已取消".to_string();
    }
    match state {
        SubscriptionCacheState::Ready => match last_update.map(|result| result.outcome) {
            Some(SubscriptionUpdateOutcome::NotModified) => "未变化".to_string(),
            Some(SubscriptionUpdateOutcome::Imported) => "已导入".to_string(),
            _ => "可用".to_string(),
        },
        SubscriptionCacheState::StaleAfterFailure => "旧缓存".to_string(),
        SubscriptionCacheState::FailedNoCache => "失败".to_string(),
        SubscriptionCacheState::Empty => {
            if cache.is_some() {
                "无内容".to_string()
            } else {
                "无缓存".to_string()
            }
        }
    }
}

pub(super) fn cache_color(state: SubscriptionCacheState, palette: ShellPalette) -> gpui::Hsla {
    match state {
        SubscriptionCacheState::Ready => palette.active,
        SubscriptionCacheState::StaleAfterFailure => palette.warning,
        SubscriptionCacheState::FailedNoCache => palette.danger,
        SubscriptionCacheState::Empty => palette.muted,
    }
}

pub(crate) fn usage_from_cache(
    _source: &SubscriptionSource,
    cache: Option<&SubscriptionCacheMetadata>,
    state: SubscriptionCacheState,
    now: u64,
) -> SubscriptionUsageView {
    let user_info = cache
        .and_then(|cache| cache.last_update.as_ref())
        .and_then(|result| result.user_info.as_ref());
    let used_bytes = user_info.and_then(|info| info.used_bytes()).unwrap_or(0);
    let total_bytes = user_info.and_then(|info| info.total);
    let used_gb = bytes_to_gb(used_bytes);
    let total_gb = total_bytes.map(bytes_to_gb).unwrap_or(0.0);
    let percent = if state != SubscriptionCacheState::Empty {
        match total_bytes {
            Some(total) if total > 0 => {
                (used_bytes as f32 / total as f32 * 100.0).clamp(0.0, 100.0)
            }
            _ => 0.0,
        }
    } else {
        0.0
    };
    let expires_label = user_info
        .and_then(|info| info.expire)
        .map(|expire| format_relative_timestamp(expire, now));
    let expires_tooltip = user_info
        .and_then(|info| info.expire)
        .map(format_shanghai_timestamp);
    SubscriptionUsageView {
        used_gb,
        total_gb,
        percent,
        label: match total_bytes {
            Some(_) => format!("{used_gb:.1} / {total_gb:.1} GB"),
            None => "无限".to_string(),
        },
        expires_label,
        expires_tooltip,
    }
}
