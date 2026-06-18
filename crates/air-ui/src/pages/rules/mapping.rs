pub(super) fn display_payload_for(rule_type: &str, payload: &str) -> String {
    if payload.trim().is_empty() {
        match rule_type {
            "Match" | "MATCH" => "MATCH".to_string(),
            _ => "-".to_string(),
        }
    } else {
        payload.to_string()
    }
}

pub(super) fn target_line_for(rule_type: &str, proxy: &str) -> String {
    let proxy = if proxy.trim().is_empty() { "-" } else { proxy };
    format!("{rule_type} -> {proxy}")
}

pub(super) fn build_search_haystack(
    index: usize,
    rule_type: &str,
    payload: &str,
    proxy: &str,
    disabled: bool,
) -> String {
    format!(
        "{} {} {} {} {}",
        index,
        rule_type,
        payload,
        proxy,
        if disabled {
            "禁用 disabled"
        } else {
            "启用 enabled"
        }
    )
    .to_lowercase()
}
