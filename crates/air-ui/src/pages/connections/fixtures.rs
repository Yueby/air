#[cfg(test)]
use air_mihomo::ConnectionsResponse;

#[cfg(test)]
pub(super) fn fake_connections_response() -> ConnectionsResponse {
    let processes = [
        "chrome.exe",
        "Code.exe",
        "mihomo.exe",
        "Telegram.exe",
        "powershell.exe",
    ];
    let connections = (0..64)
        .map(|index| {
            let host = match index % 5 {
                0 => "api.github.com",
                1 => "www.gstatic.com",
                2 => "cdn.example.test",
                3 => "dns.google",
                _ => "updates.air.test",
            };
            let chain = match index % 3 {
                0 => vec!["Proxy", "Hong Kong 01"],
                1 => vec!["Auto", "Japan 02"],
                _ => vec!["DIRECT"],
            };
            serde_json::json!({
                "id": format!("conn-{index:04}-demo-session"),
                "metadata": {
                    "host": host,
                    "destinationIP": format!("198.18.0.{}", index + 1),
                    "destinationPort": 443 + (index % 3),
                    "network": if index % 7 == 0 { "udp" } else { "tcp" },
                    "process": processes[index % processes.len()]
                },
                "chains": chain,
                "upload": 90_000 + index as u64 * 1777,
                "download": 800_000 + index as u64 * 9111,
                "uploadSpeed": 900 + index as u64 * 117,
                "downloadSpeed": 8_000 + index as u64 * 911,
                "start": format!("2026-05-22T10:{:02}:00+08:00", index % 60)
            })
        })
        .collect();

    ConnectionsResponse {
        connections,
        upload_total: 0,
        download_total: 0,
        memory: 0,
        extra: Default::default(),
    }
}
