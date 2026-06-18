use super::state::MonitorViewModel;
pub(super) fn visible_log_text(view_model: &MonitorViewModel) -> String {
    view_model
        .visible_logs
        .iter()
        .map(|entry| {
            format!(
                "{} [{}] {}",
                entry.sequence_label, entry.level_label, entry.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn format_bytes(bytes: u64) -> String {
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
