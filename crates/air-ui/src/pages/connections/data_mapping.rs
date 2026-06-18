use std::cmp::Ordering;

use serde_json::Value;

use super::render::ConnectionListItem;
use super::state::{ConnectionSort, ConnectionSortField, SortDirection};

pub(super) fn sort_connections(items: &mut [ConnectionListItem], sort: ConnectionSort) {
    items.sort_by(|left, right| {
        let ordering = match sort.field {
            ConnectionSortField::DownloadSpeed => left.download_speed.cmp(&right.download_speed),
            ConnectionSortField::UploadSpeed => left.upload_speed.cmp(&right.upload_speed),
            ConnectionSortField::DownloadTotal => left.download_total.cmp(&right.download_total),
            ConnectionSortField::UploadTotal => left.upload_total.cmp(&right.upload_total),
            ConnectionSortField::StartedAt => left
                .started_at_epoch
                .unwrap_or_default()
                .cmp(&right.started_at_epoch.unwrap_or_default()),
            ConnectionSortField::ProcessName => text_cmp(&left.app_name, &right.app_name),
        };
        let ordering = match sort.direction {
            SortDirection::Asc => ordering,
            SortDirection::Desc => ordering.reverse(),
        };
        ordering.then_with(|| left.id.cmp(&right.id))
    });
}

pub(super) fn text_cmp(left: &str, right: &str) -> Ordering {
    left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase())
}

pub(super) fn matches_text(query: &str, values: &[String]) -> bool {
    let query = query.trim().to_ascii_lowercase();
    query.is_empty()
        || values
            .iter()
            .any(|value| value.to_ascii_lowercase().contains(&query))
}

pub(super) fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

pub(super) fn value_field_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .or_else(|| value.as_u64().map(|number| number.to_string()))
        })
    })
}

pub(super) fn metadata_string(
    metadata: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter()
        .find_map(|key| metadata.get(*key).and_then(Value::as_str))
        .map(ToOwned::to_owned)
}

pub(super) fn metadata_value_string(
    metadata: &serde_json::Map<String, Value>,
    keys: &[&str],
) -> Option<String> {
    keys.iter().find_map(|key| {
        metadata.get(*key).and_then(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .or_else(|| value.as_u64().map(|number| number.to_string()))
        })
    })
}

pub(super) fn metadata_string_array(
    metadata: &serde_json::Map<String, Value>,
    key: &str,
) -> Option<Vec<String>> {
    metadata.get(key).and_then(Value::as_array).map(|values| {
        values
            .iter()
            .filter_map(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(ToOwned::to_owned)
            .collect()
    })
}

pub(super) fn string_array_field(value: &Value, key: &str) -> Option<Vec<String>> {
    value.get(key).and_then(Value::as_array).map(|values| {
        values
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect()
    })
}

pub(super) fn number_field(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
}

pub(super) fn endpoint_label(
    metadata: &serde_json::Map<String, Value>,
    ip_key: &str,
    port_key: &str,
) -> String {
    let ip = metadata_string(metadata, &[ip_key]).unwrap_or_default();
    let port = metadata_value_string(metadata, &[port_key]).unwrap_or_default();
    match (ip.trim().is_empty(), port.trim().is_empty()) {
        (true, true) => String::new(),
        (false, true) => ip,
        (true, false) => port,
        (false, false) => format!("{ip}:{port}"),
    }
}

pub(super) fn rate_from_delta(current: u64, previous: u64, elapsed_seconds: f64) -> Option<u64> {
    if current < previous || elapsed_seconds <= 0.0 {
        return None;
    }
    Some(((current - previous) as f64 / elapsed_seconds).round() as u64)
}
