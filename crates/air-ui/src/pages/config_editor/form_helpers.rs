use std::collections::BTreeMap;

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px,
};
use gpui_component::StyledExt;

use air_config::model::{ExternalControllerCorsConfig, FallbackFilterConfig, GeoxUrlConfig};
use air_config::{DnsNameserverPolicySettings, SnifferProtocolSettings};
use air_ui::components;
use air_ui::icons::{self, Icon};
use air_ui::shell::{Shell, ShellPalette};

use super::render::ConfigBoolField;
use super::state::ConfigEditorGroup;
pub(crate) fn bool_chip_with_id(
    id: &'static str,
    label: &'static str,
    value: Option<bool>,
    field: ConfigBoolField,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    let (text, color) = match value {
        Some(true) => ("开启", palette.active),
        Some(false) => ("关闭", palette.warning),
        None => ("未配置", palette.muted),
    };

    div()
        .id(format!("config-bool-{id}"))
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .min_h(px(34.0))
        .px_3()
        .rounded_md()
        .bg(palette.subtle)
        .text_xs()
        .font_bold()
        .text_color(palette.text)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(label)
                .child(div().text_color(color).child(text)),
        )
        .child(
            components::app_switch(
                format!("config-bool-switch-{id}"),
                value.unwrap_or(false),
                false,
                "鍒囨崲 mihomo 閰嶇疆",
            )
            .on_click(cx.listener(move |shell, _, _, cx| {
                shell.cycle_config_bool(field);
                cx.notify();
            })),
        )
}

pub(crate) fn advanced_toggle(
    group: ConfigEditorGroup,
    open: bool,
    palette: ShellPalette,
    cx: &mut Context<Shell>,
) -> impl IntoElement {
    div()
        .id(format!("config-advanced-{}", group.label()))
        .flex()
        .items_center()
        .gap_2()
        .h(px(30.0))
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .bg(palette.subtle)
        .hover(move |this| this.bg(palette.hover))
        .child(icons::icon(
            if open {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            },
            palette.text,
        ))
        .child(
            div()
                .text_xs()
                .font_bold()
                .text_color(palette.text)
                .child("高级字段"),
        )
        .on_click(cx.listener(move |shell, _, _, cx| {
            shell.toggle_config_advanced(group);
            cx.notify();
        }))
}

pub(crate) fn risk_note(text: &'static str, palette: ShellPalette) -> gpui::Div {
    div()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(palette.warning)
        .bg(palette.subtle)
        .text_sm()
        .text_color(palette.text)
        .child(text)
}

pub(crate) fn section_title(
    icon: Icon,
    title: &'static str,
    palette: ShellPalette,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_sm()
        .font_bold()
        .text_color(palette.text)
        .child(icons::icon(icon, palette.active))
        .child(title)
}

pub(crate) fn cycle_bool(value: &mut Option<bool>) {
    *value = match value {
        None => Some(true),
        Some(true) => Some(false),
        Some(false) => None,
    };
}

pub(crate) fn optional_text(value: &str) -> Option<String> {
    value
        .trim()
        .is_empty()
        .then_some(())
        .and(None)
        .or_else(|| Some(value.trim().to_string()))
}

pub(crate) fn optional_u32(value: Option<u32>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

pub(crate) fn optional_u64(value: Option<u64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

pub(crate) fn optional_value(value: Option<&serde_yaml::Value>) -> String {
    match value {
        Some(serde_yaml::Value::Number(number)) => number.to_string(),
        Some(serde_yaml::Value::String(text)) => text.clone(),
        Some(value) => serde_yaml::to_string(value)
            .map(|text| text.trim().to_string())
            .unwrap_or_default(),
        None => String::new(),
    }
}

pub(crate) fn optional_value_from_text(value: &str) -> Option<serde_yaml::Value> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(
        trimmed
            .parse::<i64>()
            .map(|value| serde_yaml::Value::Number(value.into()))
            .unwrap_or_else(|_| serde_yaml::Value::String(trimmed.to_string())),
    )
}

pub(crate) fn parse_optional_u32(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<u32>().ok()
}

pub(crate) fn parse_optional_u64(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<u64>().ok()
}

pub(crate) fn split_lines(value: &str) -> Vec<String> {
    value
        .lines()
        .flat_map(|line| line.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn external_controller_cors_from_form(
    allow_origins: &str,
    allow_private_network: Option<bool>,
    original: Option<&ExternalControllerCorsConfig>,
) -> Option<ExternalControllerCorsConfig> {
    let allow_origins = split_lines(allow_origins);
    if allow_origins.is_empty()
        && allow_private_network.is_none()
        && original
            .map(|cors| cors.extensions.is_empty())
            .unwrap_or(true)
    {
        None
    } else {
        let mut cors = original.cloned().unwrap_or_default();
        cors.allow_origins = allow_origins;
        cors.allow_private_network = allow_private_network;
        Some(cors)
    }
}

pub(crate) fn geox_url_from_form(
    geoip: &str,
    geosite: &str,
    mmdb: &str,
    asn: &str,
    original: Option<&GeoxUrlConfig>,
) -> Option<GeoxUrlConfig> {
    let mut geox = original.cloned().unwrap_or_default();
    geox.geoip = optional_text(geoip);
    geox.geosite = optional_text(geosite);
    geox.mmdb = optional_text(mmdb);
    geox.asn = optional_text(asn);
    if geox.geoip.is_none()
        && geox.geosite.is_none()
        && geox.mmdb.is_none()
        && geox.asn.is_none()
        && geox.extensions.is_empty()
    {
        None
    } else {
        Some(geox)
    }
}

pub(crate) fn parse_protocol_lines(
    value: &str,
    original: &[SnifferProtocolSettings],
) -> Vec<SnifferProtocolSettings> {
    let mut by_name = original
        .iter()
        .cloned()
        .map(|protocol| (protocol.name.to_ascii_lowercase(), protocol))
        .collect::<BTreeMap<_, _>>();

    value
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (name, body) = line
                .split_once(':')
                .or_else(|| line.split_once('='))
                .map(|(name, body)| (name.trim(), body))
                .unwrap_or((line, ""));
            if name.is_empty() {
                return None;
            }
            let mut protocol = by_name
                .remove(&name.to_ascii_lowercase())
                .unwrap_or_else(|| SnifferProtocolSettings::new(name));
            protocol.name = name.to_string();
            let (ports, override_destination) = parse_sniffer_protocol_body(body);
            protocol.ports = ports;
            if let Some(override_destination) = override_destination {
                protocol.override_destination = Some(override_destination);
            }
            Some(protocol)
        })
        .collect()
}

pub(crate) fn format_sniffer_protocol_line(protocol: &SnifferProtocolSettings) -> String {
    let mut line = format!("{}: {}", protocol.name, protocol.ports.join(", "));
    if let Some(override_destination) = protocol.override_destination {
        line.push_str(&format!("; override-destination={override_destination}"));
    }
    line
}

pub(crate) fn parse_sniffer_protocol_body(body: &str) -> (Vec<String>, Option<bool>) {
    let mut segments = body.split(';');
    let ports = split_lines(segments.next().unwrap_or_default());
    let mut override_destination = None;

    for segment in segments {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let Some((key, value)) = segment.split_once('=').or_else(|| segment.split_once(':')) else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key != "override-destination" {
            continue;
        }
        match value {
            "true" => override_destination = Some(true),
            "false" => override_destination = Some(false),
            _ => {}
        }
    }

    (ports, override_destination)
}

pub(crate) fn parse_policy_lines(
    value: &str,
    original: &[DnsNameserverPolicySettings],
) -> Vec<DnsNameserverPolicySettings> {
    let mut by_matcher = original
        .iter()
        .cloned()
        .map(|policy| (policy.matcher.clone(), policy))
        .collect::<BTreeMap<_, _>>();

    value
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let Some((matcher, nameservers)) = line.split_once('=') else {
                return None;
            };
            let matcher = matcher.trim();
            let mut policy = by_matcher
                .remove(matcher)
                .unwrap_or_else(|| DnsNameserverPolicySettings::new(matcher, Vec::new()));
            policy.matcher = matcher.to_string();
            policy.nameservers = split_lines(nameservers);
            Some(policy)
        })
        .collect()
}

pub(crate) fn format_dns_policy_line(policy: &DnsNameserverPolicySettings) -> String {
    format!("{} = {}", policy.matcher, policy.nameservers.join(", "))
}

pub(crate) fn fallback_filter_from_form(
    geoip: Option<bool>,
    geoip_code: &str,
    geosite: &str,
    ipcidr: &str,
    domain: &str,
    original: Option<&FallbackFilterConfig>,
) -> Option<FallbackFilterConfig> {
    let mut filter = original.cloned().unwrap_or_default();
    filter.geoip = geoip;
    filter.geoip_code = optional_text(geoip_code);
    filter.geosite = split_lines(geosite);
    filter.ipcidr = split_lines(ipcidr);
    filter.domain = split_lines(domain);

    if filter.geoip.is_none()
        && filter.geoip_code.is_none()
        && filter.geosite.is_empty()
        && filter.ipcidr.is_empty()
        && filter.domain.is_empty()
        && filter.extensions.is_empty()
    {
        None
    } else {
        Some(filter)
    }
}
