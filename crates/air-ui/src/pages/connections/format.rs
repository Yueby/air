use std::time::{Duration, SystemTime, UNIX_EPOCH};

use air_ui::icons::Icon;

pub(super) fn join_label_parts(parts: impl IntoIterator<Item = String>, separator: &str) -> String {
    parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join(separator)
}

pub(super) fn empty_dash(value: &str) -> &str {
    if value.trim().is_empty() { "-" } else { value }
}

pub(super) fn short_id(id: &str) -> String {
    if id.chars().count() <= 16 {
        id.to_string()
    } else {
        format!("{}...", id.chars().take(13).collect::<String>())
    }
}

pub(super) fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

pub(super) fn process_display_name(value: &str) -> String {
    value
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(value)
        .trim_end_matches(".exe")
        .to_string()
}

pub(super) fn app_icon(app_name: &str) -> Icon {
    let lower = app_name.to_ascii_lowercase();
    if lower.contains("chrome") || lower.contains("edge") || lower.contains("firefox") {
        Icon::Globe
    } else if lower.contains("mihomo") || lower.contains("clash") {
        Icon::Cable
    } else if lower.contains("code") || lower.contains("terminal") {
        Icon::Terminal
    } else {
        Icon::AppWindow
    }
}

pub(super) fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub(super) fn format_bytes_per_second(bytes: u64) -> String {
    format!("{}/s", format_bytes(bytes))
}

pub(crate) fn relative_time_label(start: &str, now: SystemTime) -> String {
    let Some(start_epoch) = parse_rfc3339_epoch_seconds(start) else {
        return start.to_string();
    };
    let now_epoch = now
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs() as i64;
    let elapsed = now_epoch.saturating_sub(start_epoch);

    if elapsed < 60 {
        "几秒前".to_string()
    } else if elapsed < 3_600 {
        format!("{} 分钟前", elapsed / 60)
    } else if elapsed < 86_400 {
        format!("{} 小时前", elapsed / 3_600)
    } else {
        format!("{} 天前", elapsed / 86_400)
    }
}

pub(super) fn parse_rfc3339_epoch_seconds(input: &str) -> Option<i64> {
    let input = input.trim();
    let separator = input.find('T').or_else(|| input.find(' '))?;
    let (date, time_with_zone) = input.split_at(separator);
    let time_with_zone = &time_with_zone[1..];
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i32>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;

    let (time, offset_seconds) = if let Some(time) = time_with_zone.strip_suffix('Z') {
        (time, 0)
    } else {
        let offset_index = time_with_zone.rfind(['+', '-'])?;
        let (time, offset) = time_with_zone.split_at(offset_index);
        (time, parse_timezone_offset(offset)?)
    };
    let time = time.split('.').next().unwrap_or(time);
    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<i64>().ok()?;
    let minute = time_parts.next()?.parse::<i64>().ok()?;
    let second = time_parts.next()?.parse::<i64>().ok()?;

    // 使用 Howard Hinnant 的 civil date 算法，避免为了一个运行态展示字段新增时间库依赖。
    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second - offset_seconds)
}

pub(super) fn parse_timezone_offset(offset: &str) -> Option<i64> {
    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let offset = offset.strip_prefix(['+', '-'])?;
    let mut parts = offset.split(':');
    let hour = parts.next()?.parse::<i64>().ok()?;
    let minute = parts.next().unwrap_or("0").parse::<i64>().ok()?;
    Some(sign * (hour * 3_600 + minute * 60))
}

pub(super) fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i32;
    let day = day as i32;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) as i64
}
