use super::*;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use air_app::AppCommand;
use air_mihomo::ConnectionsResponse;
use air_mihomo::streams::StreamEvent;

#[test]
fn filters_and_sorts_connections_by_default_download_speed() {
    let mut state = ConnectionsPageState::fake_for_test();
    state.set_search_query("github");

    let model = state.view_model();

    assert_eq!(model.status, ConnectionStatusFilter::Active);
    assert_eq!(model.sort.field, ConnectionSortField::DownloadSpeed);
    assert!(!model.items.is_empty());
    assert!(
        model
            .items
            .iter()
            .all(|item| item.target.contains("github"))
    );
    assert!(
        model
            .items
            .windows(2)
            .all(|pair| pair[0].download_speed >= pair[1].download_speed)
    );
}

#[test]
fn stream_connections_replace_current_active_snapshot() {
    let mut state = ConnectionsPageState::default();
    state.apply_stream_event(StreamEvent::Connections(serde_json::json!({
        "connections": [{
            "id": "abc",
            "metadata": {"host": "example.test", "process": "chrome.exe", "network": "tcp"},
            "chains": ["DIRECT"],
            "upload": 10,
            "download": 20,
            "uploadSpeed": 1,
            "downloadSpeed": 2,
            "start": "2026-05-22T10:00:00+08:00"
        }]
    })));

    let model = state.view_model();

    assert_eq!(model.items.len(), 1);
    assert_eq!(model.items[0].target, "example.test");
    assert_eq!(model.items[0].app_name, "chrome");
    assert_eq!(model.total_upload_speed, 1);
    assert_eq!(model.total_download_speed, 2);
    assert_eq!(model.stream_state, ConnectionsStreamState::Ready);
}

#[test]
fn close_connection_dispatches_without_confirmation() {
    let mut state = ConnectionsPageState::fake_for_test();
    let id = state.view_model().items[0].id.clone();

    let command = state.request_close_connection(id.clone());

    assert!(matches!(
        command,
        Some(AppCommand::CloseConnection { id: ref command_id }) if command_id == &id
    ));
    assert_eq!(state.view_model().pending_close, None);
    state.set_status_filter(ConnectionStatusFilter::Closed);
    assert!(state.view_model().items.iter().any(|item| item.id == id));
}

#[test]
fn detail_modal_uses_compact_extracted_connection_json() {
    let mut state = ConnectionsPageState::fake_for_test();
    let item = state.view_model().items[0].clone();

    state.open_detail(item.id.clone());
    let model = state.view_model();

    let detail = model.detail.expect("detail should open for existing id");
    assert_eq!(detail.id, item.id);
    assert!(detail.title.contains(&item.app_name));
    assert!(detail.json.contains("\"app_name\""));
    assert!(detail.json.contains("\"target\""));
    assert!(detail.json.contains("\"chains\""));
    assert_eq!(state.detail_json(), detail.json);

    state.close_detail();
    assert_eq!(state.view_model().detail, None);
    assert!(state.detail_json().is_empty());
}

#[test]
fn close_all_targets_current_active_filter_only() {
    let mut state = ConnectionsPageState::fake_for_test();
    state.set_search_query("github");
    let expected = state.view_model().items.len();

    state.request_close_all();
    assert!(matches!(
        state.view_model().pending_close,
        Some(PendingClose::Filtered { count, .. }) if count == expected
    ));

    let commands = state.confirm_pending_close();

    assert_eq!(commands.len(), expected);
    assert!(
        commands
            .iter()
            .all(|command| matches!(command, AppCommand::CloseConnection { .. }))
    );
}

#[test]
fn relative_time_labels_cover_seconds_minutes_and_hours() {
    let now = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    let now_epoch = now.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let seconds = epoch_to_rfc3339_utc(now_epoch - 42);
    let minutes = epoch_to_rfc3339_utc(now_epoch - 120);
    let hours = epoch_to_rfc3339_utc(now_epoch - 7_200);

    assert_eq!(relative_time_label(&seconds, now), "几秒前");
    assert_eq!(relative_time_label(&minutes, now), "2 分钟前");
    assert_eq!(relative_time_label(&hours, now), "2 小时前");
}

#[test]
fn api_error_is_visible_and_refresh_returns_retry_command() {
    let mut state = ConnectionsPageState::default();
    state.set_error("controller refused");

    let model = state.view_model();
    assert_eq!(model.stream_state, ConnectionsStreamState::Error);
    assert!(model.notice.unwrap().message.contains("controller"));
    assert!(matches!(state.refresh(), AppCommand::RefreshConnections));
}

#[test]
fn poll_refresh_does_not_reopen_closed_connection_confirmation() {
    let mut state = ConnectionsPageState::fake_for_test();
    let id = state.view_model().items[0].id.clone();
    state.request_close_connection(id.clone());

    assert!(matches!(
        state.poll_refresh(),
        AppCommand::RefreshConnections
    ));
    assert_eq!(state.view_model().pending_close, None);
}

#[test]
fn stream_focus_state_transitions_without_fetching_http_snapshot() {
    let mut state = ConnectionsPageState::default();

    state.start_stream();
    assert_eq!(
        state.view_model().stream_state,
        ConnectionsStreamState::Reconnecting {
            attempt: 0,
            next_delay_ms: 0
        }
    );

    state.stop_stream();
    assert_eq!(
        state.view_model().stream_state,
        ConnectionsStreamState::Stopped
    );
}

#[test]
fn mihomo_connections_response_maps_runtime_metadata() {
    let mut state = ConnectionsPageState::default();

    state.apply_connections_response(sample_mihomo_connections_response(
        32_442, 55_430, 6_416, 9_086,
    ));

    let model = state.view_model();
    let item = &model.items[0];
    assert_eq!(model.total_upload, 32_442);
    assert_eq!(model.total_download, 55_430);
    assert_eq!(model.memory, 86_085_632);
    assert_eq!(
        item.target,
        "mobile.events.data.microsoft.com:443".to_string()
    );
    assert_eq!(item.app_name, "Code");
    assert_eq!(item.connection_type, "Tun(tcp)");
    assert_eq!(item.chain, "🇭🇰 Hong Kong丨01 / SSRDOG");
    assert_eq!(item.primary_chain, "🇭🇰 Hong Kong丨01");
    assert_eq!(item.rule, "Match");
    assert_eq!(item.inbound, "DEFAULT-TUN");
    assert_eq!(item.endpoint_line, "198.18.0.1:49631 -> 20.42.65.89:443");
    assert!(item.remote.contains("远端 120.240.178.59"));
    assert!(item.remote.contains("Geo US"));
    assert!(item.process_path.contains("Microsoft VS Code"));
    assert!(item.dns_mode.contains("normal"));
}

#[test]
fn empty_connection_host_falls_back_to_destination_ip() {
    let mut state = ConnectionsPageState::default();
    let mut response = sample_mihomo_connections_response(32_442, 55_430, 6_416, 9_086);
    response.connections[0]["metadata"]["host"] = serde_json::json!("");
    response.connections[0]["metadata"]["destinationIP"] = serde_json::json!("203.0.113.10");

    state.apply_connections_response(response);

    let model = state.view_model();
    assert_eq!(model.items[0].target, "203.0.113.10:443");
}

#[test]
fn missing_speed_fields_are_derived_from_previous_snapshot() {
    let mut state = ConnectionsPageState::default();
    state.apply_connections_response(sample_mihomo_connections_response(1_000, 2_000, 100, 200));
    state.last_response_at = Some(SystemTime::now() - Duration::from_secs(1));

    state.apply_connections_response(sample_mihomo_connections_response(1_500, 2_600, 350, 800));

    let model = state.view_model();
    assert_eq!(model.total_upload_speed, 500);
    assert_eq!(model.total_download_speed, 600);
    assert_eq!(model.items[0].upload_speed, 250);
    assert_eq!(model.items[0].download_speed, 600);
}

fn sample_mihomo_connections_response(
    upload_total: u64,
    download_total: u64,
    upload: u64,
    download: u64,
) -> ConnectionsResponse {
    serde_json::from_value(serde_json::json!({
        "downloadTotal": download_total,
        "uploadTotal": upload_total,
        "memory": 86085632_u64,
        "connections": [{
            "id": "819f317b-cdcf-408c-ac98-54bcd2bf8bae",
            "metadata": {
                "network": "tcp",
                "type": "Tun",
                "sourceIP": "198.18.0.1",
                "destinationIP": "20.42.65.89",
                "destinationGeoIP": ["us"],
                "sourcePort": "49631",
                "destinationPort": "443",
                "inboundName": "DEFAULT-TUN",
                "host": "mobile.events.data.microsoft.com",
                "dnsMode": "normal",
                "process": "Code.exe",
                "processPath": "C:\\Applications\\Microsoft VS Code\\Code.exe",
                "remoteDestination": "120.240.178.59",
                "sniffHost": "mobile.events.data.microsoft.com"
            },
            "upload": upload,
            "download": download,
            "start": "2026-05-24T19:30:32.5695659+08:00",
            "chains": ["🇭🇰 Hong Kong丨01", "SSRDOG"],
            "providerChains": ["", ""],
            "rule": "Match",
            "rulePayload": ""
        }]
    }))
    .unwrap()
}

fn epoch_to_rfc3339_utc(epoch: i64) -> String {
    // 测试只关心相对时间边界，日期转换复用被测解析器的 UTC 形态即可稳定构造样例。
    let days = epoch.div_euclid(86_400);
    let seconds = epoch.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds / 3_600,
        seconds % 3_600 / 60,
        seconds % 60
    )
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + i32::from(month <= 2);
    (year, month as u32, day as u32)
}
