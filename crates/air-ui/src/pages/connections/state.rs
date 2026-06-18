use std::collections::{BTreeMap, HashSet};
use std::time::SystemTime;

use gpui::ScrollHandle;
use serde_json::Value;

use air_app::AppCommand;
use air_mihomo::ConnectionsResponse;
use air_mihomo::streams::StreamEvent;
use air_ui::icons::Icon;

use super::data_mapping::*;
#[cfg(test)]
use super::fixtures::*;
use super::format::*;
use super::render::*;

pub const CONNECTION_STATUS_OPTIONS: [&str; 2] = ["活动中", "已关闭"];
pub const CONNECTION_SORT_FIELD_OPTIONS: [&str; 6] = [
    "下载速度",
    "上传速度",
    "下载量",
    "上传量",
    "时间",
    "进程名称",
];
#[derive(Clone, Debug)]
pub struct ConnectionsPageState {
    connections: Vec<ConnectionEntry>,
    query: ConnectionListQuery,
    sort: ConnectionSort,
    notice: Option<ConnectionNotice>,
    pending_close: Option<PendingClose>,
    detail: Option<ConnectionDetailModal>,
    stream_state: ConnectionsStreamState,
    pub(super) card_scroll_handle: ScrollHandle,
    pub(crate) last_response_at: Option<SystemTime>,
    runtime_upload_total: u64,
    runtime_download_total: u64,
    runtime_upload_speed: u64,
    runtime_download_speed: u64,
    runtime_memory: u64,
}

impl Default for ConnectionsPageState {
    fn default() -> Self {
        Self {
            connections: Vec::new(),
            query: ConnectionListQuery::default(),
            sort: ConnectionSort::default(),
            notice: None,
            pending_close: None,
            detail: None,
            stream_state: ConnectionsStreamState::Stopped,
            card_scroll_handle: ScrollHandle::default(),
            last_response_at: None,
            runtime_upload_total: 0,
            runtime_download_total: 0,
            runtime_upload_speed: 0,
            runtime_download_speed: 0,
            runtime_memory: 0,
        }
    }
}

impl ConnectionsPageState {
    #[cfg(test)]
    pub fn fake_for_test() -> Self {
        let mut state = Self::default();
        state.apply_connections_response(fake_connections_response());
        state
    }

    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.query.search = query.into();
        self.pending_close = None;
        self.detail = None;
    }

    pub(crate) fn set_status_filter(&mut self, status: ConnectionStatusFilter) {
        self.query.status = status;
        self.pending_close = None;
        self.detail = None;
    }

    pub(crate) fn set_sort_field(&mut self, field: ConnectionSortField) {
        self.sort.field = field;
        self.pending_close = None;
    }

    pub(crate) fn toggle_sort_direction(&mut self) {
        self.sort.direction = self.sort.direction.toggle();
        self.pending_close = None;
    }

    pub fn refresh(&mut self) -> AppCommand {
        self.notice = None;
        self.pending_close = None;
        AppCommand::RefreshConnections
    }

    pub fn poll_refresh(&mut self) -> AppCommand {
        AppCommand::RefreshConnections
    }

    pub fn start_stream(&mut self) {
        self.notice = None;
        self.pending_close = None;
        self.stream_state = ConnectionsStreamState::Reconnecting {
            attempt: 0,
            next_delay_ms: 0,
        };
    }

    pub fn stop_stream(&mut self) {
        self.stream_state = ConnectionsStreamState::Stopped;
    }

    pub fn apply_connections_response(&mut self, response: ConnectionsResponse) {
        let now = SystemTime::now();
        let elapsed = self
            .last_response_at
            .and_then(|previous| now.duration_since(previous).ok())
            .map(|duration| duration.as_secs_f64())
            .filter(|seconds| *seconds > 0.0);
        let previous_by_id = self
            .connections
            .iter()
            .filter(|connection| connection.status == ConnectionStatusFilter::Active)
            .map(|connection| {
                (
                    connection.id.clone(),
                    (connection.upload_total, connection.download_total),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let previous_upload_total = self.runtime_upload_total;
        let previous_download_total = self.runtime_download_total;
        let response_upload_total = response.upload_total;
        let response_download_total = response.download_total;
        let response_memory = response.memory;

        let active_entries = response
            .connections
            .into_iter()
            .filter_map(|value| {
                ConnectionEntry::from_value(
                    value,
                    ConnectionStatusFilter::Active,
                    &previous_by_id,
                    elapsed,
                )
            })
            .collect::<Vec<_>>();
        let active_ids = active_entries
            .iter()
            .map(|connection| connection.id.clone())
            .collect::<HashSet<_>>();

        // /connections 只返回当前活动连接。这里仅保留本会话内由 UI 关闭过的条目，
        // 不做历史持久化，避免把运行态页面变成连接历史仓库。
        self.connections = self
            .connections
            .iter()
            .filter(|connection| {
                connection.status == ConnectionStatusFilter::Closed
                    && !active_ids.contains(&connection.id)
            })
            .cloned()
            .chain(active_entries)
            .collect();
        let active_upload_total = self
            .connections
            .iter()
            .filter(|connection| connection.status == ConnectionStatusFilter::Active)
            .map(|connection| connection.upload_total)
            .sum::<u64>();
        let active_download_total = self
            .connections
            .iter()
            .filter(|connection| connection.status == ConnectionStatusFilter::Active)
            .map(|connection| connection.download_total)
            .sum::<u64>();
        let active_upload_speed = self
            .connections
            .iter()
            .filter(|connection| connection.status == ConnectionStatusFilter::Active)
            .map(|connection| connection.upload_speed)
            .sum::<u64>();
        let active_download_speed = self
            .connections
            .iter()
            .filter(|connection| connection.status == ConnectionStatusFilter::Active)
            .map(|connection| connection.download_speed)
            .sum::<u64>();
        let has_runtime_totals = response_upload_total > 0 || response_download_total > 0;
        self.runtime_upload_total = if has_runtime_totals {
            response_upload_total
        } else {
            active_upload_total
        };
        self.runtime_download_total = if has_runtime_totals {
            response_download_total
        } else {
            active_download_total
        };
        self.runtime_upload_speed = elapsed
            .and_then(|seconds| {
                rate_from_delta(response_upload_total, previous_upload_total, seconds)
            })
            .filter(|_| has_runtime_totals)
            .unwrap_or(active_upload_speed);
        self.runtime_download_speed = elapsed
            .and_then(|seconds| {
                rate_from_delta(response_download_total, previous_download_total, seconds)
            })
            .filter(|_| has_runtime_totals)
            .unwrap_or(active_download_speed);
        self.runtime_memory = response_memory;
        self.last_response_at = Some(now);
        self.notice = None;
        self.stream_state = ConnectionsStreamState::Ready;
    }

    pub fn apply_stream_event(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::Connections(value) => {
                match serde_json::from_value::<ConnectionsResponse>(value) {
                    Ok(response) => self.apply_connections_response(response),
                    Err(error) => self.set_error(format!("连接流数据无法解析: {error}")),
                }
            }
            StreamEvent::Disconnected {
                attempt,
                next_delay_ms,
            } => {
                self.stream_state = ConnectionsStreamState::Reconnecting {
                    attempt,
                    next_delay_ms,
                };
            }
            StreamEvent::Log { .. } | StreamEvent::Traffic { .. } | StreamEvent::Memory { .. } => {}
        }
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.notice = Some(ConnectionNotice::error(message));
        self.stream_state = ConnectionsStreamState::Error;
    }

    pub fn take_notice(&mut self) -> Option<ConnectionNotice> {
        self.notice.take()
    }

    pub fn request_close_connection(&mut self, id: impl Into<String>) -> Option<AppCommand> {
        let id = id.into();
        if let Some(connection) = self.connections.iter_mut().find(|connection| {
            connection.id == id && connection.status == ConnectionStatusFilter::Active
        }) {
            connection.status = ConnectionStatusFilter::Closed;
            self.pending_close = None;
            self.notice = None;
            Some(AppCommand::CloseConnection { id })
        } else {
            self.notice = Some(ConnectionNotice::error(
                "连接已经不处于活动状态，刷新后再试。",
            ));
            None
        }
    }

    pub fn open_detail(&mut self, id: impl Into<String>) {
        let id = id.into();
        let Some(connection) = self
            .connections
            .iter()
            .find(|connection| connection.id == id)
        else {
            self.notice = Some(ConnectionNotice::error(
                "连接已经从当前快照中移除，刷新后再查看详情。",
            ));
            return;
        };

        self.detail = Some(ConnectionDetailModal {
            id: connection.id.clone(),
            title: format!("{} -> {}", connection.app_name, connection.target),
            json: connection.detail_json_pretty(),
        });
    }

    pub fn close_detail(&mut self) {
        self.detail = None;
    }

    pub fn detail_json(&self) -> String {
        self.detail
            .as_ref()
            .map(|detail| detail.json.clone())
            .unwrap_or_default()
    }

    pub fn request_close_all(&mut self) {
        let ids = self.current_close_targets();
        if ids.is_empty() {
            self.notice = Some(ConnectionNotice::info(
                "当前筛选结果中没有可关闭的活动连接。",
            ));
            return;
        }

        self.pending_close = Some(PendingClose::Filtered {
            count: ids.len(),
            ids,
            status_label: self.query.status.label().to_string(),
            query_label: self.query.summary_label(),
        });
        self.notice = None;
    }

    pub fn cancel_pending_close(&mut self) {
        self.pending_close = None;
    }

    pub fn confirm_pending_close(&mut self) -> Vec<AppCommand> {
        let Some(pending) = self.pending_close.take() else {
            return Vec::new();
        };
        let ids = match pending {
            PendingClose::One { id, .. } => vec![id],
            PendingClose::Filtered { ids, .. } => ids,
        };
        let ids_to_close = ids.into_iter().collect::<HashSet<_>>();

        for connection in &mut self.connections {
            if ids_to_close.contains(&connection.id) {
                connection.status = ConnectionStatusFilter::Closed;
            }
        }

        ids_to_close
            .into_iter()
            .map(|id| AppCommand::CloseConnection { id })
            .collect()
    }

    pub fn view_model(&self) -> ConnectionsPageViewModel {
        let now = SystemTime::now();
        let mut items = self
            .connections
            .iter()
            .filter(|connection| self.query.matches(connection))
            .map(|connection| ConnectionListItem::from_entry(connection, now))
            .collect::<Vec<_>>();
        sort_connections(&mut items, self.sort);

        let active_count = self
            .connections
            .iter()
            .filter(|connection| connection.status == ConnectionStatusFilter::Active)
            .count();
        let closed_count = self.connections.len().saturating_sub(active_count);
        let closable_filtered_count = items
            .iter()
            .filter(|item| item.status == ConnectionStatusFilter::Active)
            .count();

        ConnectionsPageViewModel {
            items,
            total_count: self.connections.len(),
            filtered_count: self
                .connections
                .iter()
                .filter(|connection| self.query.matches(connection))
                .count(),
            active_count,
            closed_count,
            closable_filtered_count,
            total_upload: self.runtime_upload_total,
            total_download: self.runtime_download_total,
            total_upload_speed: self.runtime_upload_speed,
            total_download_speed: self.runtime_download_speed,
            memory: self.runtime_memory,
            status: self.query.status,
            sort: self.sort,
            stream_state: self.stream_state.clone(),
            pending_close: self.pending_close.clone(),
            detail: self.detail.clone(),
            notice: self.notice.clone(),
        }
    }

    fn current_close_targets(&self) -> Vec<String> {
        self.connections
            .iter()
            .filter(|connection| {
                connection.status == ConnectionStatusFilter::Active
                    && self.query.matches(connection)
            })
            .map(|connection| connection.id.clone())
            .collect()
    }
}

#[derive(Clone, Debug)]
struct ConnectionListQuery {
    pub(super) status: ConnectionStatusFilter,
    search: String,
}

impl Default for ConnectionListQuery {
    fn default() -> Self {
        Self {
            status: ConnectionStatusFilter::Active,
            search: String::new(),
        }
    }
}

impl ConnectionListQuery {
    fn matches(&self, connection: &ConnectionEntry) -> bool {
        connection.status == self.status && matches_text(&self.search, &connection.search_values())
    }

    fn summary_label(&self) -> String {
        let query = self.search.trim();
        if query.is_empty() {
            "当前筛选: 全部可见字段".to_string()
        } else {
            format!("当前筛选: “{query}”")
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConnectionSort {
    pub(super) field: ConnectionSortField,
    pub(super) direction: SortDirection,
}

impl Default for ConnectionSort {
    fn default() -> Self {
        Self {
            field: ConnectionSortField::DownloadSpeed,
            direction: SortDirection::Desc,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionStatusFilter {
    Active,
    Closed,
}

impl ConnectionStatusFilter {
    pub(crate) fn from_label(label: &str) -> Option<Self> {
        match label {
            "活动中" => Some(Self::Active),
            "已关闭" => Some(Self::Closed),
            _ => None,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Active => "活动中",
            Self::Closed => "已关闭",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionSortField {
    DownloadSpeed,
    UploadSpeed,
    DownloadTotal,
    UploadTotal,
    StartedAt,
    ProcessName,
}

impl ConnectionSortField {
    pub(crate) fn from_label(label: &str) -> Option<Self> {
        match label {
            "下载速度" => Some(Self::DownloadSpeed),
            "上传速度" => Some(Self::UploadSpeed),
            "下载量" => Some(Self::DownloadTotal),
            "上传量" => Some(Self::UploadTotal),
            "时间" => Some(Self::StartedAt),
            "进程名称" => Some(Self::ProcessName),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    fn toggle(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }

    pub(super) fn icon(self) -> Icon {
        match self {
            Self::Asc => Icon::ArrowUpNarrowWide,
            Self::Desc => Icon::ArrowDownWideNarrow,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Asc => "升序",
            Self::Desc => "降序",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionsStreamState {
    Ready,
    Reconnecting { attempt: usize, next_delay_ms: u64 },
    Stopped,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PendingClose {
    One {
        id: String,
        target: String,
    },
    Filtered {
        count: usize,
        ids: Vec<String>,
        status_label: String,
        query_label: String,
    },
}

#[derive(Clone, Debug)]
pub(super) struct ConnectionEntry {
    pub(super) id: String,
    pub(super) app_name: String,
    pub(super) process_path: String,
    pub(super) target: String,
    pub(super) connection_type: String,
    pub(super) chains: Vec<String>,
    pub(super) provider_chains: Vec<String>,
    pub(super) rule: String,
    pub(super) rule_payload: String,
    pub(super) inbound_name: String,
    pub(super) dns_mode: String,
    pub(super) source_endpoint: String,
    pub(super) destination_endpoint: String,
    pub(super) destination_geo: String,
    pub(super) remote_destination: String,
    pub(super) sniff_host: String,
    pub(super) upload_speed: u64,
    pub(super) download_speed: u64,
    pub(super) upload_total: u64,
    pub(super) download_total: u64,
    pub(super) start: String,
    pub(super) started_at_epoch: Option<i64>,
    pub(super) status: ConnectionStatusFilter,
}

impl ConnectionEntry {
    fn from_value(
        value: Value,
        status: ConnectionStatusFilter,
        previous_by_id: &BTreeMap<String, (u64, u64)>,
        elapsed: Option<f64>,
    ) -> Option<Self> {
        let object = value.as_object()?;
        let metadata = object.get("metadata").and_then(Value::as_object);
        let id = string_field(&value, &["id"])
            .or_else(|| metadata.and_then(|metadata| metadata_string(metadata, &["id"])))?;
        let destination_ip = metadata
            .and_then(|metadata| {
                metadata_string(metadata, &["destinationIP", "dstIP", "remoteDestination"])
            })
            .unwrap_or_default();
        let host = metadata
            .and_then(|metadata| metadata_string(metadata, &["host", "destinationHost"]))
            .or_else(|| string_field(&value, &["host", "destination"]))
            .filter(|value| !value.trim().is_empty())
            .or_else(|| (!destination_ip.trim().is_empty()).then(|| destination_ip.clone()))
            .unwrap_or_else(|| "-".to_string());
        let port = metadata
            .and_then(|metadata| {
                metadata_value_string(metadata, &["destinationPort", "dstPort", "port"])
            })
            .or_else(|| value_field_string(&value, &["port"]));
        let target = port
            .filter(|port| !port.is_empty() && port != "0")
            .map(|port| format!("{host}:{port}"))
            .unwrap_or(host);
        let process_path = metadata
            .and_then(|metadata| {
                metadata_string(metadata, &["processPath", "process_path", "sourceProcess"])
            })
            .unwrap_or_default();
        let app_name = metadata
            .and_then(|metadata| {
                metadata_string(
                    metadata,
                    &["process", "processName", "processPath", "sourceProcess"],
                )
            })
            .map(|value| process_display_name(&value))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "未知应用".to_string());
        let connection_type = metadata
            .and_then(|metadata| {
                let network = metadata_string(metadata, &["network"]).unwrap_or_default();
                let kind =
                    metadata_string(metadata, &["type", "connectionType"]).unwrap_or_default();
                if network.trim().is_empty() {
                    Some(kind)
                } else if kind.trim().is_empty() {
                    Some(network)
                } else {
                    Some(format!("{kind}({network})"))
                }
                .filter(|value| !value.is_empty())
            })
            .or_else(|| string_field(&value, &["network", "type"]))
            .unwrap_or_else(|| "TCP".to_string());
        let chains = string_array_field(&value, "chains")
            .or_else(|| string_array_field(&value, "chain"))
            .filter(|chains| !chains.is_empty())
            .unwrap_or_else(|| vec!["DIRECT".to_string()]);
        let provider_chains = string_array_field(&value, "providerChains")
            .unwrap_or_default()
            .into_iter()
            .filter(|value| !value.trim().is_empty())
            .collect::<Vec<_>>();
        let start = string_field(&value, &["start", "startTime", "startedAt"])
            .unwrap_or_else(|| "-".to_string());
        let upload_total = number_field(&value, &["upload", "up"]).unwrap_or_default();
        let download_total = number_field(&value, &["download", "down"]).unwrap_or_default();
        let previous = previous_by_id.get(&id).copied();
        let upload_speed = number_field(
            &value,
            &["uploadSpeed", "upSpeed", "curUpload", "currentUpload"],
        )
        .or_else(|| {
            previous.and_then(|(previous_upload, _)| {
                elapsed.and_then(|seconds| rate_from_delta(upload_total, previous_upload, seconds))
            })
        })
        .unwrap_or_default();
        let download_speed = number_field(
            &value,
            &[
                "downloadSpeed",
                "downSpeed",
                "curDownload",
                "currentDownload",
            ],
        )
        .or_else(|| {
            previous.and_then(|(_, previous_download)| {
                elapsed
                    .and_then(|seconds| rate_from_delta(download_total, previous_download, seconds))
            })
        })
        .unwrap_or_default();
        let rule = string_field(&value, &["rule"]).unwrap_or_default();
        let rule_payload =
            string_field(&value, &["rulePayload", "rule_payload"]).unwrap_or_default();
        let inbound_name = metadata
            .and_then(|metadata| metadata_string(metadata, &["inboundName", "inboundUser"]))
            .unwrap_or_default();
        let dns_mode = metadata
            .and_then(|metadata| metadata_string(metadata, &["dnsMode"]))
            .unwrap_or_default();
        let source_endpoint = metadata
            .map(|metadata| endpoint_label(metadata, "sourceIP", "sourcePort"))
            .filter(|value| !value.is_empty())
            .unwrap_or_default();
        let destination_endpoint = metadata
            .map(|metadata| endpoint_label(metadata, "destinationIP", "destinationPort"))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| destination_ip.clone());
        let destination_geo = metadata
            .and_then(|metadata| metadata_string_array(metadata, "destinationGeoIP"))
            .map(|values| {
                values
                    .into_iter()
                    .map(|value| value.to_ascii_uppercase())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default();
        let remote_destination = metadata
            .and_then(|metadata| metadata_string(metadata, &["remoteDestination"]))
            .unwrap_or_default();
        let sniff_host = metadata
            .and_then(|metadata| metadata_string(metadata, &["sniffHost"]))
            .unwrap_or_default();

        Some(Self {
            id,
            app_name,
            process_path,
            target,
            connection_type,
            chains,
            provider_chains,
            rule,
            rule_payload,
            inbound_name,
            dns_mode,
            source_endpoint,
            destination_endpoint,
            destination_geo,
            remote_destination,
            sniff_host,
            upload_speed,
            download_speed,
            upload_total,
            download_total,
            started_at_epoch: parse_rfc3339_epoch_seconds(&start),
            start,
            status,
        })
    }

    pub(super) fn chain_label(&self) -> String {
        if self.chains.is_empty() {
            "-".to_string()
        } else {
            self.chains.join(" / ")
        }
    }

    pub(super) fn primary_chain_label(&self) -> String {
        self.chains
            .first()
            .filter(|value| !value.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| "-".to_string())
    }

    pub(super) fn provider_chain_label(&self) -> String {
        self.provider_chains.join(" / ")
    }

    pub(super) fn rule_label(&self) -> String {
        match (
            self.rule.trim().is_empty(),
            self.rule_payload.trim().is_empty(),
        ) {
            (true, true) => "-".to_string(),
            (false, true) => self.rule.clone(),
            (true, false) => self.rule_payload.clone(),
            (false, false) => format!("{}: {}", self.rule, self.rule_payload),
        }
    }

    pub(super) fn remote_label(&self) -> String {
        join_label_parts(
            [
                (!self.remote_destination.trim().is_empty())
                    .then(|| format!("远端 {}", self.remote_destination)),
                (!self.destination_geo.trim().is_empty())
                    .then(|| format!("Geo {}", self.destination_geo)),
                (!self.sniff_host.trim().is_empty() && self.sniff_host != self.target)
                    .then(|| format!("SNI {}", self.sniff_host)),
            ]
            .into_iter()
            .flatten(),
            " · ",
        )
    }

    fn detail_json_pretty(&self) -> String {
        // 连接列表常驻状态不保留 mihomo 原始 JSON，避免高频连接流为每条连接重复持有大对象。
        // 详情弹窗只在用户打开时，用已提取字段临时组装诊断 JSON。
        serde_json::to_string_pretty(&serde_json::json!({
            "id": self.id,
            "app_name": self.app_name,
            "process_path": self.process_path,
            "target": self.target,
            "connection_type": self.connection_type,
            "chains": self.chains,
            "provider_chains": self.provider_chains,
            "rule": self.rule,
            "rule_payload": self.rule_payload,
            "inbound_name": self.inbound_name,
            "dns_mode": self.dns_mode,
            "source_endpoint": self.source_endpoint,
            "destination_endpoint": self.destination_endpoint,
            "destination_geo": self.destination_geo,
            "remote_destination": self.remote_destination,
            "sniff_host": self.sniff_host,
            "upload_speed": self.upload_speed,
            "download_speed": self.download_speed,
            "upload_total": self.upload_total,
            "download_total": self.download_total,
            "start": self.start,
            "status": self.status.label(),
        }))
        .unwrap_or_else(|_| "{}".to_string())
    }

    pub(super) fn endpoint_line(&self) -> String {
        if self.source_endpoint.is_empty() && self.destination_endpoint.is_empty() {
            return String::new();
        }
        format!(
            "{} -> {}",
            empty_dash(&self.source_endpoint),
            empty_dash(&self.destination_endpoint)
        )
    }

    fn search_values(&self) -> Vec<String> {
        vec![
            self.id.clone(),
            self.app_name.clone(),
            self.process_path.clone(),
            self.target.clone(),
            self.connection_type.clone(),
            self.chain_label(),
            self.provider_chain_label(),
            self.rule_label(),
            self.inbound_name.clone(),
            self.dns_mode.clone(),
            self.endpoint_line(),
            self.destination_geo.clone(),
            self.remote_destination.clone(),
            self.sniff_host.clone(),
            format_bytes(self.upload_speed),
            format_bytes(self.download_speed),
            format_bytes(self.upload_total),
            format_bytes(self.download_total),
            self.start.clone(),
            self.status.label().to_string(),
        ]
    }
}

#[derive(Clone, Debug)]
pub struct ConnectionsPageViewModel {
    pub items: Vec<ConnectionListItem>,
    pub total_count: usize,
    pub filtered_count: usize,
    pub active_count: usize,
    pub closed_count: usize,
    pub closable_filtered_count: usize,
    pub total_upload: u64,
    pub total_download: u64,
    pub total_upload_speed: u64,
    pub total_download_speed: u64,
    pub memory: u64,
    pub status: ConnectionStatusFilter,
    pub sort: ConnectionSort,
    pub stream_state: ConnectionsStreamState,
    pub pending_close: Option<PendingClose>,
    pub detail: Option<ConnectionDetailModal>,
    pub notice: Option<ConnectionNotice>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionDetailModal {
    pub id: String,
    pub title: String,
    pub json: String,
}
