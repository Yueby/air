use super::*;
use air_mihomo::streams::StreamEvent;

#[test]
fn stream_events_are_redacted_and_searchable() {
    let mut state = MonitorPageState::default();
    state.apply_stream_event(StreamEvent::Log {
        level: "info".to_string(),
        message: "fetch https://example.test/sub?token=abc secret=def".to_string(),
    });
    state.set_search_query("fetch");

    let model = state.view_model();

    assert_eq!(model.visible_logs.len(), 1);
    assert!(model.visible_logs[0].message.contains("token=***"));
    assert!(!model.visible_logs[0].message.contains("abc"));
    assert!(!model.visible_logs[0].message.contains("def"));
}

#[test]
fn log_filter_and_query_are_applied_before_render_limit() {
    let mut state = MonitorPageState::default();
    state.apply_stream_event(StreamEvent::Log {
        level: "info".to_string(),
        message: "normal route selected".to_string(),
    });
    state.apply_stream_event(StreamEvent::Log {
        level: "error".to_string(),
        message: "provider failed".to_string(),
    });

    state.set_filter(LogLevelFilter::Error);
    state.set_search_query("provider");
    let model = state.view_model();

    assert_eq!(model.filtered_count, 1);
    assert_eq!(model.visible_logs[0].level, LogLevel::Error);
}

#[test]
fn stream_state_caps_log_and_traffic_memory() {
    let mut state = MonitorPageState::default();
    for index in 0..(MAX_LOG_ENTRIES + 50) {
        state.apply_stream_event(StreamEvent::Log {
            level: "debug".to_string(),
            message: format!("line {index}"),
        });
    }
    for index in 0..(MAX_METRIC_POINTS + 20) {
        state.apply_stream_event(StreamEvent::Traffic {
            upload: index as u64,
            download: index as u64,
        });
    }

    let model = state.view_model();

    assert_eq!(state.logs.len(), MAX_LOG_ENTRIES);
    assert_eq!(state.traffic_points.len(), MAX_METRIC_POINTS);
    assert_eq!(model.rendered_count, MAX_LOG_ENTRIES);
}

#[test]
fn disconnected_stream_event_is_silent_in_view_model() {
    let mut state = MonitorPageState::default();
    state.apply_stream_event(StreamEvent::Disconnected {
        attempt: 2,
        next_delay_ms: 1_500,
    });
    assert_eq!(
        state.view_model().connection,
        StreamConnectionState::Stopped
    );

    state.apply_stream_event(StreamEvent::Traffic {
        upload: 10,
        download: 20,
    });
    state.apply_stream_event(StreamEvent::Disconnected {
        attempt: 3,
        next_delay_ms: 3_000,
    });
    assert_eq!(
        state.view_model().connection,
        StreamConnectionState::Streaming
    );

    state.stop_streams();
    assert_eq!(
        state.view_model().connection,
        StreamConnectionState::Stopped
    );
}

#[test]
fn format_bytes_uses_binary_units() {
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1536), "1.5 KiB");
    assert_eq!(format_bytes(2 * 1024 * 1024), "2.0 MiB");
}
