use std::rc::Rc;

use air_app::AppCommand;
use air_mihomo::RulesResponse;

use super::mapping::{build_search_haystack, display_payload_for, target_line_for};
#[derive(Clone, Debug, Default)]
pub(crate) struct RulesProxyPageState {
    search_query: String,
    rules: Rc<Vec<RuntimeRuleItem>>,
}

impl RulesProxyPageState {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(crate) fn sample_for_test() -> Self {
        let mut state = Self::empty();
        state.apply_rules_response(
            serde_json::from_value(serde_json::json!({
                "rules": [
                    {
                        "index": 0,
                        "type": "DomainSuffix",
                        "payload": "🇭🇰 example.com",
                        "proxy": "🇭🇰 Hong Kong丨01",
                        "extra": {"disabled": false}
                    },
                    {
                        "index": 1,
                        "type": "Match",
                        "payload": "",
                        "proxy": "DIRECT",
                        "extra": {"disabled": true}
                    }
                ]
            }))
            .expect("runtime rules fixture should parse"),
        );
        state
    }

    pub(crate) fn set_search_query(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
    }

    pub(crate) fn apply_rules_response(&mut self, response: RulesResponse) {
        self.rules = Rc::new(
            response
                .rules
                .into_iter()
                .enumerate()
                .map(|(order, value)| RuntimeRuleItem::from_value(order, value))
                .collect(),
        );
    }

    pub(crate) fn request_rule_enabled(
        &mut self,
        index: usize,
        enabled: bool,
    ) -> Option<AppCommand> {
        let Some(rule) = Rc::make_mut(&mut self.rules)
            .iter_mut()
            .find(|rule| rule.index == index)
        else {
            return None;
        };
        // 开关状态完全来自 mihomo 运行态；这里仅做乐观展示，成功后会由 `/rules` 刷新覆盖。
        rule.disabled = !enabled;
        rule.refresh_search_haystack();
        Some(AppCommand::DisableRule {
            index,
            disabled: !enabled,
        })
    }

    pub(crate) fn view_model(&self) -> RulesProxyPageViewModel {
        let search = self.search_query.trim().to_lowercase();
        let visible_rule_indices = self
            .rules
            .iter()
            .enumerate()
            .filter(|(_, rule)| search.is_empty() || rule.matches_search(&search))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let filtered_count = visible_rule_indices.len();

        RulesProxyPageViewModel {
            rules: Rc::clone(&self.rules),
            visible_rule_indices,
            total_count: self.rules.len(),
            filtered_count,
            search_query: self.search_query.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RulesProxyPageViewModel {
    pub(crate) rules: Rc<Vec<RuntimeRuleItem>>,
    pub(crate) visible_rule_indices: Vec<usize>,
    pub(crate) total_count: usize,
    pub(crate) filtered_count: usize,
    pub(crate) search_query: String,
}

impl RulesProxyPageViewModel {
    pub(crate) fn visible_rule(&self, visible_index: usize) -> Option<&RuntimeRuleItem> {
        self.visible_rule_indices
            .get(visible_index)
            .and_then(|rule_index| self.rules.get(*rule_index))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeRuleItem {
    pub(crate) index: usize,
    pub(crate) rule_type: String,
    pub(crate) payload: String,
    pub(crate) proxy: String,
    pub(crate) disabled: bool,
    display_payload: String,
    target_line: String,
    search_haystack: String,
}

impl RuntimeRuleItem {
    pub(crate) fn from_value(order: usize, value: serde_json::Value) -> Self {
        let index = value
            .get("index")
            .and_then(serde_json::Value::as_u64)
            .map(|index| index as usize)
            .unwrap_or(order);
        let rule_type = string_field(&value, &["type", "ruleType"]).unwrap_or_else(|| "-".into());
        let payload = string_field(&value, &["payload"]).unwrap_or_default();
        let proxy = string_field(&value, &["proxy", "target", "policy"]).unwrap_or_default();
        let disabled = value
            .get("extra")
            .and_then(|extra| extra.get("disabled").or_else(|| extra.get("disable")))
            .or_else(|| value.get("disabled").or_else(|| value.get("disable")))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let display_payload = display_payload_for(&rule_type, &payload);
        let target_line = target_line_for(&rule_type, &proxy);
        let search_haystack = build_search_haystack(index, &rule_type, &payload, &proxy, disabled);

        Self {
            index,
            rule_type,
            payload,
            proxy,
            disabled,
            display_payload,
            target_line,
            search_haystack,
        }
    }

    pub(crate) fn display_payload(&self) -> &str {
        self.display_payload.as_str()
    }

    pub(crate) fn target_line(&self) -> &str {
        self.target_line.as_str()
    }

    fn matches_search(&self, search: &str) -> bool {
        self.search_haystack.contains(search)
    }

    fn refresh_search_haystack(&mut self) {
        // 运行态启停只会改变 disabled 状态；搜索文本预计算后需要同步刷新，
        // 避免用户按 enabled/disabled 过滤时看到旧状态。
        self.search_haystack = build_search_haystack(
            self.index,
            &self.rule_type,
            &self.payload,
            &self.proxy,
            self.disabled,
        );
    }
}

fn string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
        .map(ToOwned::to_owned)
}
