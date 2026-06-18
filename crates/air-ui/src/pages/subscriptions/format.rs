use std::collections::BTreeMap;
use std::path::Path;

use air_mihomo::subscriptions::SubscriptionSource;

use super::render::SubscriptionYamlImportValidation;
pub(super) fn is_valid_subscription_url(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    url::Url::parse(value)
        .map(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
        .unwrap_or(false)
}

pub(crate) fn validate_yaml_file_selection(path: &Path) -> SubscriptionYamlImportValidation {
    if !is_yaml_path(path) {
        return SubscriptionYamlImportValidation::rejected("只能选择 .yaml 或 .yml 配置文件");
    }
    // UI 只做文件类型预检；读取、解析、校验和缓存写入统一交给 app/service 层，
    // 避免文件路径和解析细节在界面回调里泄漏或形成第二套导入语义。
    SubscriptionYamlImportValidation::accepted(0, Vec::new())
}

pub(super) fn is_yaml_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "yaml" | "yml"))
        .unwrap_or(false)
}

pub(super) fn parse_positive_u64(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok().filter(|value| *value > 0)
}

pub(super) fn parse_positive_usize(value: &str) -> Option<usize> {
    value.trim().parse::<usize>().ok()
}

pub(super) fn bytes_to_gb(bytes: u64) -> f32 {
    bytes as f32 / 1024.0 / 1024.0 / 1024.0
}

pub(super) fn now_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(super) fn format_relative_past_timestamp(timestamp: u64, now: u64) -> String {
    if timestamp >= now {
        return "刚刚".to_string();
    }
    let delta_seconds = (now - timestamp) / 1000;
    match delta_seconds {
        0..=59 => "刚刚".to_string(),
        60..=3_599 => format!("{}分钟前", delta_seconds / 60),
        3_600..=86_399 => format!("{}小时前", delta_seconds / 3_600),
        _ => format!("{}天前", delta_seconds / 86_400),
    }
}

pub(super) fn format_relative_timestamp(timestamp: u64, now: u64) -> String {
    if timestamp <= now {
        return format_relative_past_timestamp(timestamp, now);
    }
    let delta_seconds = (timestamp - now) / 1000;
    match delta_seconds {
        0..=59 => "即将".to_string(),
        60..=3_599 => format!("{}分钟后", delta_seconds / 60),
        3_600..=86_399 => format!("{}小时后", delta_seconds / 3_600),
        _ => format!("{}天后", delta_seconds / 86_400),
    }
}

pub(super) fn format_shanghai_timestamp(timestamp: u64) -> String {
    // 固定按 Asia/Shanghai 的 UTC+8 展示订阅时间，避免依赖系统本地时区造成跨平台差异。
    let seconds = timestamp / 1000 + 8 * 3600;
    let days = seconds / 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let day_seconds = seconds % 86_400;
    let hour = day_seconds / 3600;
    let minute = (day_seconds % 3600) / 60;
    let second = day_seconds % 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} Asia/Shanghai")
}

pub(super) fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    // Howard Hinnant 的 civil-from-days 算法，避免为订阅列表日期格式化额外引入时间库依赖。
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year as i32, m as u32, d as u32)
}

pub(super) fn parse_headers(value: &str) -> BTreeMap<String, String> {
    value
        .lines()
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .filter(|(name, _)| !name.is_empty())
        .collect()
}

pub(super) fn optional_text(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub(super) fn next_source_id(sources: &[SubscriptionSource]) -> String {
    let mut index = sources.len() + 1;
    loop {
        let candidate = format!("subscription-{index}");
        if sources.iter().all(|source| source.id != candidate) {
            return candidate;
        }
        index += 1;
    }
}
