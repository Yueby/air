use std::collections::{BTreeMap, BTreeSet};

use air_app::AppCommand;
#[cfg(test)]
use air_config::ConfigDocument;
use air_config::model::{MihomoConfigDocument, ProxyGroupKind};
use air_mihomo::groups::{
    ProxyGroupCollection, ProxyGroupMemberReference, ProxyGroupRuntimeProjection,
    ProxyGroupSelectionState,
};
use air_mihomo::proxies::{ProxyDelayStatus, ProxyNodeCollection};

#[cfg(test)]
use crate::pages::proxy_groups::fixtures::{
    seed_fake_group_delays, seed_fake_member_delays, seed_fake_runtime,
};

use super::format::*;
use super::render::{GroupDelaySnapshot, GroupMemberView, GroupNotice, item_matches_search};
use super::runtime_projection::*;

pub(crate) const DEFAULT_GROUP_DELAY_URL: &str = "https://www.gstatic.com/generate_204";
pub(crate) const DEFAULT_GROUP_DELAY_TIMEOUT_MS: u64 = 5_000;
pub(crate) const PROXY_CARD_WIDTH: f32 = 248.0;
// 右侧节点卡片只承担节点名称、协议和延迟信息，可比左侧代理组卡片更窄，以提升默认窗口的信息密度。
pub(crate) const PROXY_MEMBER_CARD_MIN_WIDTH: f32 = 232.0;
pub(crate) const PROXY_CARD_GAP: f32 = 8.0;
// 左侧代理组列表需要给常显滚动条留出额外槽位，避免滚动条覆盖卡片右侧内容。
pub(crate) const PROXY_GROUP_COLUMN_WIDTH: f32 = PROXY_CARD_WIDTH + 32.0;
pub(crate) const PROXY_SELECTED_CARD_ALPHA: f32 = 0.14;

#[derive(Clone, Debug)]
pub struct GroupPageState {
    pub(crate) document: MihomoConfigDocument,
    pub(crate) collection: ProxyGroupCollection,
    runtime: BTreeMap<String, ProxyGroupSelectionState>,
    runtime_order: BTreeMap<String, usize>,
    runtime_projection_loaded: bool,
    group_delays: BTreeMap<String, GroupDelaySnapshot>,
    member_delays: BTreeMap<(String, String), GroupDelaySnapshot>,
    selected_name: Option<String>,
    search_query: String,
    sort_members_by_delay: bool,
    modal: GroupModalState,
    form: GroupFormState,
    pub(crate) notice: Option<GroupNotice>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SelectedProxyDelayTarget {
    pub(crate) group: String,
    pub(crate) proxy: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ActiveProxySelection {
    pub(crate) group: String,
    pub(crate) proxy: String,
    pub(crate) protocol: String,
    pub(crate) delay_status: ProxyDelayStatus,
    pub(crate) delay_ms: Option<u64>,
}

impl GroupPageState {
    pub fn empty() -> Self {
        // 内核未运行或尚未收到 `/proxies` 投影时保持空页面，避免展示过期订阅缓存。
        Self::from_document(MihomoConfigDocument::default())
    }

    pub fn from_document(document: MihomoConfigDocument) -> Self {
        let collection = ProxyGroupCollection::from_document(&document);
        let selected_name = collection
            .groups
            .first()
            .map(|group| group.common.name.clone());
        let form = selected_name
            .as_deref()
            .and_then(|name| collection.find(name))
            .map(GroupFormState::from_group)
            .unwrap_or_default();

        Self {
            document,
            collection,
            runtime: BTreeMap::new(),
            runtime_order: BTreeMap::new(),
            runtime_projection_loaded: false,
            group_delays: BTreeMap::new(),
            member_delays: BTreeMap::new(),
            selected_name,
            search_query: String::new(),
            sort_members_by_delay: false,
            modal: GroupModalState::None,
            form,
            notice: None,
        }
    }

    pub fn replace_document(&mut self, document: MihomoConfigDocument) {
        let previous_selected = self.selected_name.clone();
        let search_query = self.search_query.clone();
        let group_delays = self.group_delays.clone();
        let member_delays = self.member_delays.clone();

        let mut next = Self::from_document(document);
        next.search_query = search_query;
        next.sort_members_by_delay = self.sort_members_by_delay;
        next.selected_name = previous_selected
            .filter(|name| next.collection.find(name).is_some())
            .or_else(|| {
                next.collection
                    .groups
                    .first()
                    .map(|group| group.common.name.clone())
            });
        if let Some(name) = next.selected_name.clone() {
            if let Some(group) = next.collection.find(&name) {
                next.form = GroupFormState::from_group(group);
            }
        }
        next.group_delays = group_delays
            .into_iter()
            .filter(|(name, _)| next.collection.find(name).is_some())
            .collect();
        next.member_delays = member_delays
            .into_iter()
            .filter(|((group, _), _)| next.collection.find(group).is_some())
            .collect();

        *self = next;
    }

    #[cfg(test)]
    pub fn fake_for_test() -> Self {
        let document = ConfigDocument::parse(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/config.yaml"
        )))
        .expect("docs/config.yaml should remain a valid mihomo fixture")
        .typed;
        let mut state = Self::from_document(document.clone());
        let collection = state.collection.clone();
        let runtime = seed_fake_runtime(&document, &collection);
        let group_delays = seed_fake_group_delays(&collection);
        let member_delays = seed_fake_member_delays(&runtime);

        state.runtime = runtime;
        state.runtime_projection_loaded = true;
        state.group_delays = group_delays;
        state.member_delays = member_delays;
        state
    }

    pub fn apply_runtime_projection(&mut self, projection: ProxyGroupRuntimeProjection) {
        let mut next_runtime = BTreeMap::new();
        let mut next_runtime_order = BTreeMap::new();
        for mut state in projection.states {
            if let Some(group) = self.collection.find_case_insensitive(&state.group_name) {
                state.group_name = group.common.name.clone();
            }
            if self.selected_name.is_none() {
                self.selected_name = Some(state.group_name.clone());
            }
            let order = next_runtime.len();
            next_runtime_order.insert(state.group_name.clone(), order);
            next_runtime.insert(state.group_name.clone(), state);
        }
        self.runtime = next_runtime;
        self.runtime_order = next_runtime_order;
        self.runtime_projection_loaded = true;
        if self
            .selected_name
            .as_deref()
            .is_some_and(|name| !self.has_group(name))
        {
            self.selected_name = self.first_visible_group_name();
        }
        self.notice = None;
    }

    pub fn clear_runtime_projection(&mut self) -> bool {
        // 内核停止后运行态成员、延迟和当前选择都不再可信，必须一次性清空页面投影。
        let had_runtime_data = self.runtime_projection_loaded
            || !self.runtime.is_empty()
            || !self.runtime_order.is_empty()
            || !self.group_delays.is_empty()
            || !self.member_delays.is_empty();
        self.runtime.clear();
        self.runtime_order.clear();
        self.group_delays.clear();
        self.member_delays.clear();
        self.runtime_projection_loaded = false;
        had_runtime_data
    }

    pub fn apply_proxy_delay_result(&mut self, name: &str, delay_ms: u64) {
        let status = delay_status_from_ms(delay_ms);
        for group in self.groups_containing_member(name) {
            self.member_delays.insert(
                (group, name.to_string()),
                GroupDelaySnapshot {
                    status,
                    delay_ms: Some(delay_ms),
                },
            );
        }
    }

    pub fn apply_group_delay_result(&mut self, name: &str, member_delays: BTreeMap<String, u64>) {
        if !self.has_group(name) {
            return;
        }
        let group_delay = member_delays.values().copied().min();
        self.group_delays.insert(
            name.to_string(),
            GroupDelaySnapshot {
                status: group_delay
                    .map(delay_status_from_ms)
                    .unwrap_or(ProxyDelayStatus::Unknown),
                delay_ms: group_delay,
            },
        );
        for (member, delay_ms) in member_delays {
            self.member_delays.insert(
                (name.to_string(), member),
                GroupDelaySnapshot {
                    status: delay_status_from_ms(delay_ms),
                    delay_ms: Some(delay_ms),
                },
            );
        }
    }

    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
    }

    pub fn toggle_delay_sort(&mut self) {
        self.sort_members_by_delay = !self.sort_members_by_delay;
    }

    pub fn select_group(&mut self, name: impl Into<String>) {
        self.selected_name = Some(name.into());
        self.modal = GroupModalState::None;
    }

    pub fn begin_edit_selected(&mut self) -> GroupFormState {
        self.modal = GroupModalState::EditMembers;
        if let Some(group) = self
            .selected_name
            .as_deref()
            .and_then(|name| self.collection.find(name))
        {
            self.form = GroupFormState::from_group(group);
        }
        self.form.clone()
    }

    pub fn close_modal(&mut self) {
        self.modal = GroupModalState::None;
    }

    pub fn update_form_field(&mut self, field: GroupFormField, value: impl Into<String>) {
        let value = value.into();
        match field {
            GroupFormField::Proxies => self.form.proxies = value,
            GroupFormField::Providers => self.form.providers = value,
            GroupFormField::Filter => self.form.filter = value,
            GroupFormField::ExcludeFilter => self.form.exclude_filter = value,
        }
    }

    pub fn take_notice(&mut self) -> Option<GroupNotice> {
        self.notice.take()
    }

    pub fn save_form(&mut self) -> Option<AppCommand> {
        let Some(group_name) = self.selected_name.clone() else {
            self.notice = Some(GroupNotice::error("请先选择要编辑的代理组"));
            return None;
        };
        let Some(group) = self.collection.find_mut(&group_name) else {
            self.notice = Some(GroupNotice::error("代理组不存在"));
            return None;
        };

        group.members.proxies = split_member_lines(&self.form.proxies);
        group.members.use_providers = split_member_lines(&self.form.providers);
        group.members.filter = optional_form_text(&self.form.filter);
        group.members.exclude_filter = optional_form_text(&self.form.exclude_filter);
        self.collection.apply_to_document(&mut self.document);
        self.collection = ProxyGroupCollection::from_document(&self.document);
        self.modal = GroupModalState::None;
        match serde_yaml::to_string(&self.document) {
            Ok(profile) => {
                self.notice = Some(GroupNotice::success(
                    "已保存代理组配置，运行态选择会继续通过 mihomo API 回填",
                ));
                Some(AppCommand::SaveConfig { profile })
            }
            Err(error) => {
                self.notice = Some(GroupNotice::error(format!(
                    "保存失败：无法序列化 YAML：{error}"
                )));
                None
            }
        }
    }

    pub fn select_member(
        &mut self,
        group: impl Into<String>,
        member: impl Into<String>,
    ) -> Option<AppCommand> {
        let group = group.into();
        let member = member.into();
        if !self.is_selectable_group(&group) {
            self.notice = Some(GroupNotice::warning("只有 select 组允许手动选择当前节点"));
            return None;
        }
        let Some(selection) = self.runtime.get_mut(&group) else {
            self.notice = Some(GroupNotice::error("尚未读取该组的运行态成员"));
            return None;
        };
        if !selection.members.iter().any(|item| item.name == member) {
            self.notice = Some(GroupNotice::error("该成员不在运行态 API 返回列表中"));
            return None;
        }

        // 运行态选择必须调用 mihomo `/proxies/{group}`；配置成员只表示候选项。
        selection.selected = Some(member.clone());
        for item in &mut selection.members {
            item.selected = item.name == member;
        }
        self.selected_name = Some(group.clone());
        self.notice = None;
        Some(AppCommand::SelectProxy {
            group,
            proxy: member,
        })
    }

    pub fn test_member_delay(
        &mut self,
        group: impl Into<String>,
        member: impl Into<String>,
    ) -> Option<AppCommand> {
        let group = group.into();
        let member = member.into();
        let Some(selection) = self.runtime.get(&group) else {
            self.notice = Some(GroupNotice::error("尚未读取该组的运行态成员"));
            return None;
        };
        if !selection.members.iter().any(|item| item.name == member) {
            self.notice = Some(GroupNotice::error("该成员不在运行态 API 返回列表中"));
            return None;
        }

        self.member_delays.insert(
            (group.clone(), member.clone()),
            GroupDelaySnapshot {
                status: ProxyDelayStatus::Testing,
                delay_ms: None,
            },
        );
        self.notice = None;
        Some(AppCommand::TestProxyDelay {
            name: member,
            url: DEFAULT_GROUP_DELAY_URL.to_string(),
            timeout_ms: DEFAULT_GROUP_DELAY_TIMEOUT_MS,
        })
    }

    #[allow(dead_code)]
    pub(crate) fn selected_proxy_delay_target(&self) -> Option<SelectedProxyDelayTarget> {
        self.active_proxy_selection()
            .map(|selection| SelectedProxyDelayTarget {
                group: selection.group,
                proxy: selection.proxy,
            })
    }

    #[allow(dead_code)]
    pub(crate) fn active_proxy_selection(&self) -> Option<ActiveProxySelection> {
        let proxy_protocols = proxy_protocols_from_document(&self.document);
        let references = group_references_by_name(&self.collection, &self.document);
        let root_name = self
            .runtime
            .values()
            .find(|state| state.group_name.eq_ignore_ascii_case("proxy"))
            .map(|state| state.group_name.as_str())
            .or_else(|| {
                self.collection
                    .groups
                    .iter()
                    .find(|group| group.common.name.eq_ignore_ascii_case("proxy"))
                    .map(|group| group.common.name.as_str())
            })
            .or_else(|| {
                self.runtime
                    .values()
                    .find(|state| is_selectable_runtime_state(state))
                    .map(|state| state.group_name.as_str())
            })
            .or_else(|| {
                self.collection
                    .groups
                    .iter()
                    .find(|group| matches!(group.common.kind, ProxyGroupKind::Select))
                    .map(|group| group.common.name.as_str())
            })?;

        active_proxy_selection_from_state(
            root_name,
            &self.runtime,
            &references,
            &proxy_protocols,
            &self.member_delays,
            &mut BTreeSet::new(),
        )
    }

    #[allow(dead_code)]
    pub(crate) fn test_selected_proxy_delay(&mut self) -> Option<AppCommand> {
        let target = self.selected_proxy_delay_target()?;
        self.test_member_delay(target.group, target.proxy)
    }

    pub fn test_selected_delay(&mut self) -> Option<AppCommand> {
        let name = self.selected_name.clone()?;
        self.test_group_delay(name)
    }

    pub fn test_group_delay(&mut self, name: impl Into<String>) -> Option<AppCommand> {
        let name = name.into();
        if !self.has_group(&name) {
            self.notice = Some(GroupNotice::error("代理组不存在"));
            return None;
        }

        self.selected_name = Some(name.clone());
        self.group_delays.insert(
            name.clone(),
            GroupDelaySnapshot {
                status: ProxyDelayStatus::Testing,
                delay_ms: None,
            },
        );
        for member in self.member_names_for_group(&name) {
            self.member_delays.insert(
                (name.clone(), member),
                GroupDelaySnapshot {
                    status: ProxyDelayStatus::Testing,
                    delay_ms: None,
                },
            );
        }
        self.notice = None;
        Some(AppCommand::TestProxyGroupDelay {
            name,
            url: DEFAULT_GROUP_DELAY_URL.to_string(),
            timeout_ms: DEFAULT_GROUP_DELAY_TIMEOUT_MS,
        })
    }

    pub fn clear_selected_fixed(&mut self) -> Option<AppCommand> {
        let name = self.selected_name.clone()?;
        self.clear_fixed_for(name)
    }

    pub fn clear_fixed_for(&mut self, name: impl Into<String>) -> Option<AppCommand> {
        let name = name.into();
        if !self.has_group(&name) {
            self.notice = Some(GroupNotice::error("代理组不存在"));
            return None;
        }
        if let Some(selection) = self.runtime.get_mut(&name) {
            selection.selected = None;
            for item in &mut selection.members {
                item.selected = false;
            }
        }
        self.notice = Some(GroupNotice::success(format!(
            "已派发 fixed 选择清理：{name}"
        )));
        Some(AppCommand::ClearProxyGroupFixed { name })
    }

    pub fn view_model(&self) -> GroupPageViewModel {
        let search = self.search_query.trim().to_ascii_lowercase();
        if !self.runtime_projection_loaded {
            return GroupPageViewModel {
                items: Vec::new(),
                selected: None,
                modal: self.modal,
                form: self.form.clone(),
                notice: self.notice.clone(),
                search_query: self.search_query.clone(),
                sort_members_by_delay: self.sort_members_by_delay,
            };
        }
        let proxy_protocols = proxy_protocols_from_document(&self.document);
        let references = group_references_by_name(&self.collection, &self.document);
        let mut items = self
            .collection
            .groups
            .iter()
            .enumerate()
            .filter(|(_, group)| {
                !self.runtime_projection_loaded || self.runtime.contains_key(&group.common.name)
            })
            .map(|(document_index, group)| {
                let name = group.common.name.clone();
                let delay = self.group_delays.get(&name).copied().unwrap_or_default();
                let filter_query = self.search_query.clone();
                let runtime = self.runtime.get(&name);
                let group_references = references.get(&name).map(Vec::as_slice).unwrap_or(&[]);
                let total_member_count =
                    runtime.map(|state| state.members.len()).unwrap_or_else(|| {
                        group.members.proxies.len() + group.members.use_providers.len()
                    });
                let member_count = visible_member_count(
                    runtime,
                    group_references,
                    total_member_count,
                    &filter_query,
                );
                let mut members = filter_group_members(
                    group_members_for_view(
                        group,
                        runtime,
                        group_references,
                        &proxy_protocols,
                        &self.member_delays,
                    ),
                    &filter_query,
                );
                if self.sort_members_by_delay {
                    sort_group_members_by_delay(&mut members);
                }
                let current_member = runtime.and_then(|state| {
                    current_runtime_member_view(&name, state, &proxy_protocols, &self.member_delays)
                });
                GroupListItem {
                    name: name.clone(),
                    kind: proxy_group_type_display_label(group.common.kind.as_str()).to_string(),
                    document_index: self
                        .runtime_order
                        .get(&name)
                        .copied()
                        .unwrap_or(document_index),
                    current: self
                        .runtime
                        .get(&name)
                        .and_then(|state| state.selected.clone())
                        .or_else(|| configured_current_member(references.get(&name)))
                        .unwrap_or_else(|| "-".to_string()),
                    member_count,
                    total_member_count,
                    provider_count: group.members.use_providers.len(),
                    delay_status: delay.status,
                    delay_ms: delay.delay_ms,
                    selectable: matches!(group.common.kind, ProxyGroupKind::Select),
                    automatic: is_automatic_group(&group.common.kind),
                    health_url: group.health_check.url.clone(),
                    expanded: self.selected_name.as_deref() == Some(&name),
                    filter_query,
                    current_member,
                    members,
                }
            })
            .filter(|item| item_matches_search(item, &search))
            .collect::<Vec<_>>();
        let runtime_base_index = items.len();
        let runtime_only_items = self
            .runtime
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| self.collection.find(name).is_none())
            .map(|(runtime_index, (name, state))| {
                let delay = self.group_delays.get(name).copied().unwrap_or_default();
                let filter_query = self.search_query.clone();
                let member_count =
                    visible_member_count(Some(state), &[], state.members.len(), &filter_query);
                let all_members = state
                    .members
                    .iter()
                    .map(|member| {
                        GroupMemberView::from_runtime(
                            name,
                            member,
                            &proxy_protocols,
                            &self.member_delays,
                        )
                    })
                    .collect::<Vec<_>>();
                let mut members = filter_group_members(all_members, &filter_query);
                if self.sort_members_by_delay {
                    sort_group_members_by_delay(&mut members);
                }
                let current_member =
                    current_runtime_member_view(name, state, &proxy_protocols, &self.member_delays);
                let raw_kind = state
                    .api_kind
                    .as_deref()
                    .unwrap_or_else(|| state.configured_kind.as_str());
                GroupListItem {
                    name: name.clone(),
                    kind: proxy_group_type_display_label(raw_kind).to_string(),
                    document_index: runtime_base_index + runtime_index,
                    current: state.selected.clone().unwrap_or_else(|| "-".to_string()),
                    member_count,
                    total_member_count: state.members.len(),
                    provider_count: 0,
                    delay_status: delay.status,
                    delay_ms: delay.delay_ms,
                    selectable: is_selectable_runtime_state(state),
                    automatic: is_automatic_group(&state.configured_kind),
                    health_url: None,
                    expanded: self.selected_name.as_deref() == Some(name),
                    filter_query,
                    current_member,
                    members,
                }
            })
            .filter(|item| item_matches_search(item, &search));
        items.extend(runtime_only_items);
        items.sort_by_key(|item| item.document_index);
        let selected = self.selected_name.as_deref().and_then(|name| {
            self.collection.find(name).map(|group| {
                let mut detail = GroupDetailView::from_group(
                    group,
                    self.runtime.get(name),
                    references.get(name).map(Vec::as_slice).unwrap_or(&[]),
                    self.group_delays.get(name).copied().unwrap_or_default(),
                    &proxy_protocols,
                    &self.member_delays,
                );
                if self.sort_members_by_delay {
                    // 详情面板必须和成员卡片共享同一套运行态延迟排序，否则工具栏切换后右侧列表不会实时变化。
                    sort_group_members_by_delay(&mut detail.proxies);
                    sort_group_members_by_delay(&mut detail.providers);
                    sort_group_members_by_delay(&mut detail.runtime_members);
                }
                detail
            })
        });

        GroupPageViewModel {
            items,
            selected,
            modal: self.modal,
            form: self.form.clone(),
            notice: self.notice.clone(),
            search_query: self.search_query.clone(),
            sort_members_by_delay: self.sort_members_by_delay,
        }
    }

    fn member_names_for_group(&self, group: &str) -> Vec<String> {
        if let Some(state) = self.runtime.get(group) {
            return state
                .members
                .iter()
                .map(|member| member.name.clone())
                .collect();
        }
        self.collection
            .find(group)
            .map(|settings| {
                settings
                    .members
                    .proxies
                    .iter()
                    .chain(settings.members.use_providers.iter())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn has_group(&self, name: &str) -> bool {
        if self.runtime_projection_loaded {
            self.runtime.contains_key(name)
        } else {
            self.collection.find(name).is_some() || self.runtime.contains_key(name)
        }
    }

    fn is_selectable_group(&self, name: &str) -> bool {
        if self.runtime_projection_loaded {
            return self
                .runtime
                .get(name)
                .map(is_selectable_runtime_state)
                .unwrap_or(false);
        }
        self.collection
            .find(name)
            .map(|group| matches!(group.common.kind, ProxyGroupKind::Select))
            .or_else(|| self.runtime.get(name).map(is_selectable_runtime_state))
            .unwrap_or(false)
    }

    fn first_visible_group_name(&self) -> Option<String> {
        if self.runtime_projection_loaded {
            return self
                .runtime_order
                .iter()
                .min_by_key(|(_, order)| **order)
                .map(|(name, _)| name.clone());
        }
        self.collection
            .groups
            .first()
            .map(|group| group.common.name.clone())
            .or_else(|| self.runtime.keys().next().cloned())
    }

    fn groups_containing_member(&self, name: &str) -> Vec<String> {
        let mut groups = self
            .runtime
            .iter()
            .filter(|(_, state)| state.members.iter().any(|member| member.name == name))
            .map(|(group, _)| group.clone())
            .collect::<Vec<_>>();
        for group in self
            .collection
            .groups
            .iter()
            .filter(|group| {
                !self.runtime.contains_key(&group.common.name)
                    && (group.members.proxies.iter().any(|member| member == name)
                        || group
                            .members
                            .use_providers
                            .iter()
                            .any(|member| member == name))
            })
            .map(|group| group.common.name.clone())
        {
            groups.push(group);
        }
        groups
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GroupModalState {
    #[default]
    None,
    EditMembers,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupFormField {
    Proxies,
    Providers,
    Filter,
    ExcludeFilter,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GroupFormState {
    pub group_name: String,
    pub kind: String,
    pub proxies: String,
    pub providers: String,
    pub filter: String,
    pub exclude_filter: String,
}

impl GroupFormState {
    fn from_group(group: &air_mihomo::groups::ProxyGroupSettings) -> Self {
        Self {
            group_name: group.common.name.clone(),
            kind: proxy_group_type_display_label(group.common.kind.as_str()).to_string(),
            proxies: group.members.proxies.join("\n"),
            providers: group.members.use_providers.join("\n"),
            filter: group.members.filter.clone().unwrap_or_default(),
            exclude_filter: group.members.exclude_filter.clone().unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct GroupPageViewModel {
    pub items: Vec<GroupListItem>,
    pub selected: Option<GroupDetailView>,
    pub modal: GroupModalState,
    pub form: GroupFormState,
    pub notice: Option<GroupNotice>,
    pub search_query: String,
    pub sort_members_by_delay: bool,
}

fn proxy_protocols_from_document(document: &MihomoConfigDocument) -> BTreeMap<String, String> {
    ProxyNodeCollection::from_document(document)
        .nodes
        .iter()
        .map(|node| {
            (
                node.common.name.clone(),
                proxy_type_display_label(node.protocol.protocol_name()).to_string(),
            )
        })
        .collect()
}

fn group_references_by_name(
    collection: &ProxyGroupCollection,
    document: &MihomoConfigDocument,
) -> BTreeMap<String, Vec<ProxyGroupMemberReference>> {
    collection.resolve_references(document).into_iter().fold(
        BTreeMap::<String, Vec<ProxyGroupMemberReference>>::new(),
        |mut map, item| {
            map.entry(item.group_name.clone()).or_default().push(item);
            map
        },
    )
}

#[derive(Clone, Debug)]
pub struct GroupListItem {
    pub name: String,
    pub kind: String,
    document_index: usize,
    pub current: String,
    pub member_count: usize,
    pub total_member_count: usize,
    pub provider_count: usize,
    pub delay_status: ProxyDelayStatus,
    pub delay_ms: Option<u64>,
    pub selectable: bool,
    pub automatic: bool,
    pub health_url: Option<String>,
    pub expanded: bool,
    pub filter_query: String,
    pub current_member: Option<GroupMemberView>,
    pub members: Vec<GroupMemberView>,
}

#[derive(Clone, Debug)]
pub struct GroupDetailView {
    pub name: String,
    pub kind: String,
    pub current: Option<String>,
    pub selectable: bool,
    pub automatic: bool,
    pub proxies: Vec<GroupMemberView>,
    pub providers: Vec<GroupMemberView>,
    pub runtime_members: Vec<GroupMemberView>,
    pub health_url: Option<String>,
    pub health_interval: Option<u64>,
    pub strategy: Option<String>,
    pub delay_status: ProxyDelayStatus,
    pub delay_ms: Option<u64>,
}
