#[cfg(test)]
use std::collections::BTreeMap;

#[cfg(test)]
use air_config::model::MihomoConfigDocument;
#[cfg(test)]
use air_mihomo::dto::ProxyResponse;
#[cfg(test)]
use air_mihomo::groups::{ProxyGroupCollection, ProxyGroupSelectionState};
#[cfg(test)]
use air_mihomo::proxies::ProxyDelayStatus;

#[cfg(test)]
use super::render::GroupDelaySnapshot;
#[cfg(test)]
pub(crate) fn seed_fake_runtime(
    document: &MihomoConfigDocument,
    collection: &ProxyGroupCollection,
) -> BTreeMap<String, ProxyGroupSelectionState> {
    collection
        .groups
        .iter()
        .map(|group| {
            let mut all = group.members.proxies.clone();
            for provider in &group.members.use_providers {
                all.push(format!("{provider}-expanded"));
            }
            if all.is_empty() {
                all.push("DIRECT".to_string());
            }
            let now = all.first().cloned().unwrap_or_default();
            let response = ProxyResponse {
                name: group.common.name.clone(),
                all,
                now,
                kind: group.common.kind.as_str().to_string(),
                history: Vec::new(),
                extra: BTreeMap::new(),
            };
            let responses = BTreeMap::from([(group.common.name.clone(), response.clone())]);
            let state = group.selection_state(&response, &responses, document);
            (group.common.name.clone(), state)
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn seed_fake_member_delays(
    runtime: &BTreeMap<String, ProxyGroupSelectionState>,
) -> BTreeMap<(String, String), GroupDelaySnapshot> {
    runtime
        .iter()
        .flat_map(|(group, state)| {
            state
                .members
                .iter()
                .enumerate()
                .map(move |(index, member)| {
                    let delay_ms = (index % 4 != 0).then_some(45 + index as u64 * 23);
                    let status = match delay_ms {
                        Some(delay) if delay < 260 => ProxyDelayStatus::Available,
                        Some(_) => ProxyDelayStatus::Slow,
                        None => ProxyDelayStatus::Untested,
                    };
                    (
                        (group.clone(), member.name.clone()),
                        GroupDelaySnapshot { status, delay_ms },
                    )
                })
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn seed_fake_group_delays(
    collection: &ProxyGroupCollection,
) -> BTreeMap<String, GroupDelaySnapshot> {
    collection
        .groups
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let delay_ms = (index % 4 != 0).then_some(60 + index as u64 * 29);
            let status = match delay_ms {
                Some(delay) if delay < 300 => ProxyDelayStatus::Available,
                Some(_) => ProxyDelayStatus::Slow,
                None => ProxyDelayStatus::Untested,
            };
            (
                group.common.name.clone(),
                GroupDelaySnapshot { status, delay_ms },
            )
        })
        .collect()
}
