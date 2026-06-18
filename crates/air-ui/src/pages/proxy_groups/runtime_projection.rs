use std::collections::{BTreeMap, BTreeSet};

use gpui::{Hsla, rgb};

use air_config::model::ProxyGroupKind;
use air_mihomo::groups::{
    ProxyGroupMemberReference, ProxyGroupMemberSource, ProxyGroupSelectionState,
};
use air_mihomo::proxies::ProxyDelayStatus;
use air_ui::shell::ShellPalette;

use super::format::*;
use super::render::{GroupDelaySnapshot, GroupMemberView};
use super::state::ActiveProxySelection;
pub(crate) fn delay_color(
    status: ProxyDelayStatus,
    delay_ms: Option<u64>,
    palette: ShellPalette,
) -> Hsla {
    match status {
        ProxyDelayStatus::Available | ProxyDelayStatus::Slow => match delay_ms {
            Some(delay) if delay < 100 => rgb(0x16a34a).into(),
            Some(delay) if delay < 300 => palette.active,
            Some(delay) if delay < 800 => palette.warning,
            Some(_) => palette.danger,
            None => palette.muted,
        },
        ProxyDelayStatus::Testing => palette.warning,
        ProxyDelayStatus::Failed => palette.danger,
        ProxyDelayStatus::Unknown | ProxyDelayStatus::Untested => palette.muted,
    }
}

pub(crate) fn delay_status_from_ms(delay_ms: u64) -> ProxyDelayStatus {
    if delay_ms <= 800 {
        ProxyDelayStatus::Available
    } else {
        ProxyDelayStatus::Slow
    }
}

pub(crate) fn displayed_member_delay(member: &GroupMemberView) -> GroupDelaySnapshot {
    // 手动测速中的结果优先级最高；只有当前节点还没有实时测速结果时，才回退显示
    if !matches!(
        member.delay_status,
        ProxyDelayStatus::Unknown | ProxyDelayStatus::Untested
    ) || member.delay_ms.is_some()
    {
        return GroupDelaySnapshot {
            status: member.delay_status,
            delay_ms: member.delay_ms,
        };
    }
    member.historical_delay
}

pub(crate) fn delay_label(status: ProxyDelayStatus, delay_ms: Option<u64>) -> String {
    match (status, delay_ms) {
        (ProxyDelayStatus::Available | ProxyDelayStatus::Slow, Some(delay)) => {
            format!("{delay} ms")
        }
        (ProxyDelayStatus::Testing, _) => "测试中".to_string(),
        (ProxyDelayStatus::Failed, _) => "失败".to_string(),
        (ProxyDelayStatus::Untested, _) => "未测".to_string(),
        (ProxyDelayStatus::Unknown, _) => "-".to_string(),
        (_, None) => "-".to_string(),
    }
}

pub(crate) fn history_delay_snapshot(history: &[serde_json::Value]) -> Option<GroupDelaySnapshot> {
    history.iter().rev().find_map(|entry| {
        let delay = entry.get("delay")?.as_u64()?;
        Some(if delay == 0 {
            GroupDelaySnapshot {
                status: ProxyDelayStatus::Failed,
                delay_ms: None,
            }
        } else {
            GroupDelaySnapshot {
                status: delay_status_from_ms(delay),
                delay_ms: Some(delay),
            }
        })
    })
}

pub(crate) fn group_members_for_view(
    group: &air_mihomo::groups::ProxyGroupSettings,
    runtime: Option<&ProxyGroupSelectionState>,
    references: &[ProxyGroupMemberReference],
    proxy_protocols: &BTreeMap<String, String>,
    member_delays: &BTreeMap<(String, String), GroupDelaySnapshot>,
) -> Vec<GroupMemberView> {
    if let Some(runtime) = runtime {
        return runtime
            .members
            .iter()
            .map(|member| {
                GroupMemberView::from_runtime(
                    &group.common.name,
                    member,
                    proxy_protocols,
                    member_delays,
                )
            })
            .collect();
    }

    references
        .iter()
        .map(|reference| {
            let delay = member_delays
                .get(&(group.common.name.clone(), reference.member_name.clone()))
                .copied()
                .unwrap_or_default();
            GroupMemberView::from_reference(reference, proxy_protocols, delay)
        })
        .collect()
}

pub(crate) fn current_runtime_member_view(
    group_name: &str,
    runtime: &ProxyGroupSelectionState,
    proxy_protocols: &BTreeMap<String, String>,
    member_delays: &BTreeMap<(String, String), GroupDelaySnapshot>,
) -> Option<GroupMemberView> {
    let selected = runtime.selected.as_deref()?.trim();
    if selected.is_empty() || selected == "-" {
        return None;
    }

    runtime
        .members
        .iter()
        .find(|member| member.name == selected)
        .map(|member| {
            GroupMemberView::from_runtime(group_name, member, proxy_protocols, member_delays)
        })
}

#[allow(dead_code)]
pub(crate) fn active_proxy_selection_from_state(
    group_name: &str,
    runtime: &BTreeMap<String, ProxyGroupSelectionState>,
    references: &BTreeMap<String, Vec<ProxyGroupMemberReference>>,
    proxy_protocols: &BTreeMap<String, String>,
    member_delays: &BTreeMap<(String, String), GroupDelaySnapshot>,
    visited: &mut BTreeSet<String>,
) -> Option<ActiveProxySelection> {
    if !visited.insert(group_name.to_string()) {
        return None;
    }

    let member = runtime
        .get(group_name)
        .and_then(|state| active_runtime_member(state, proxy_protocols, member_delays))
        .or_else(|| {
            references.get(group_name).and_then(|items| {
                let reference = items.first()?;
                let delay = member_delays
                    .get(&(group_name.to_string(), reference.member_name.clone()))
                    .copied()
                    .unwrap_or_default();
                Some(GroupMemberView::from_reference(
                    reference,
                    proxy_protocols,
                    delay,
                ))
            })
        })?;

    if matches!(member.source, ProxyGroupMemberSource::ProxyGroup) {
        return active_proxy_selection_from_state(
            &member.name,
            runtime,
            references,
            proxy_protocols,
            member_delays,
            visited,
        );
    }

    Some(ActiveProxySelection {
        group: group_name.to_string(),
        proxy: member.name,
        protocol: member.protocol,
        delay_status: member.delay_status,
        delay_ms: member.delay_ms,
    })
}

#[allow(dead_code)]
pub(crate) fn active_runtime_member(
    state: &ProxyGroupSelectionState,
    proxy_protocols: &BTreeMap<String, String>,
    member_delays: &BTreeMap<(String, String), GroupDelaySnapshot>,
) -> Option<GroupMemberView> {
    current_runtime_member_view(&state.group_name, state, proxy_protocols, member_delays)
}

pub(crate) fn configured_current_member(
    references: Option<&Vec<ProxyGroupMemberReference>>,
) -> Option<String> {
    // 核心未启动时没有运行态 now 字段；用配置中的第一个可选成员作为离线展示的当前项。
    references
        .and_then(|items| items.first())
        .map(|reference| reference.member_name.clone())
}

pub(crate) fn is_selectable_runtime_state(state: &ProxyGroupSelectionState) -> bool {
    matches!(state.configured_kind, ProxyGroupKind::Select)
        || state
            .api_kind
            .as_deref()
            .map(|kind| {
                let kind = kind.trim().to_ascii_lowercase();
                kind == "selector" || kind == "select"
            })
            .unwrap_or(false)
}

pub(crate) fn filter_group_members(
    members: Vec<GroupMemberView>,
    filter_query: &str,
) -> Vec<GroupMemberView> {
    let query = filter_query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return members;
    }

    members
        .into_iter()
        .filter(|member| member_matches_search(member, &query))
        .collect()
}

pub(crate) fn member_matches_search(member: &GroupMemberView, query: &str) -> bool {
    member.name.to_ascii_lowercase().contains(query)
        || member.protocol.to_ascii_lowercase().contains(query)
        || delay_label(
            displayed_member_delay(member).status,
            displayed_member_delay(member).delay_ms,
        )
        .to_ascii_lowercase()
        .contains(query)
}

pub(crate) fn sort_group_members_by_delay(members: &mut [GroupMemberView]) {
    // 测速结果属于运行态展示信息，只影响当前页面排序；没有延迟值的成员保留在已测速成员之后，
    members.sort_by_key(|member| match member.delay_ms {
        Some(delay) => (0_u8, delay),
        None => (1_u8, u64::MAX),
    });
}

pub(crate) fn visible_member_count(
    runtime: Option<&ProxyGroupSelectionState>,
    references: &[ProxyGroupMemberReference],
    total_member_count: usize,
    filter_query: &str,
) -> usize {
    let query = filter_query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return total_member_count;
    }

    runtime
        .map(|state| {
            state
                .members
                .iter()
                .map(|member| {
                    GroupMemberView::from_runtime(
                        &state.group_name,
                        member,
                        &BTreeMap::new(),
                        &BTreeMap::new(),
                    )
                })
                .filter(|member| member_matches_search(member, &query))
                .count()
        })
        .unwrap_or_else(|| {
            references
                .iter()
                .filter(|reference| {
                    reference.member_name.to_ascii_lowercase().contains(&query)
                        || format!("{:?}", reference.source)
                            .to_ascii_lowercase()
                            .contains(&query)
                })
                .count()
        })
}

pub(crate) fn member_protocol(
    name: &str,
    source: &ProxyGroupMemberSource,
    proxy_protocols: &BTreeMap<String, String>,
) -> String {
    match source {
        ProxyGroupMemberSource::ProxyNode => proxy_protocols
            .get(name)
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string()),
        ProxyGroupMemberSource::ProxyGroup => "Group".to_string(),
        ProxyGroupMemberSource::ProxyProvider => "Provider".to_string(),
        ProxyGroupMemberSource::BuiltInPolicy => proxy_type_display_label(name).to_string(),
        ProxyGroupMemberSource::Unresolved => "Unknown".to_string(),
    }
}
