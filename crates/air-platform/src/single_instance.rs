use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use air_error::{AppResult, PlatformError};
use air_telemetry::redaction::redact_log_value;

const SINGLE_INSTANCE_ADDR: &str = "127.0.0.1:47683";
const SHOW_WINDOW_REQUEST: &[u8] = b"air.show-window.v1\n";
const IPC_TIMEOUT: Duration = Duration::from_millis(600);

#[derive(Debug)]
pub enum SingleInstance {
    Primary(SingleInstanceServer),
    AlreadyRunning,
}

#[derive(Debug)]
pub struct SingleInstanceServer {
    receiver: Receiver<SingleInstanceEvent>,
}

impl SingleInstanceServer {
    pub fn into_receiver(self) -> Receiver<SingleInstanceEvent> {
        self.receiver
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SingleInstanceEvent {
    ShowWindow,
}

pub fn acquire_or_notify_existing() -> AppResult<SingleInstance> {
    match TcpListener::bind(SINGLE_INSTANCE_ADDR) {
        Ok(listener) => {
            let (sender, receiver) = mpsc::channel();
            spawn_single_instance_listener(listener, sender);
            tracing::info!(
                addr = SINGLE_INSTANCE_ADDR,
                "single instance listener acquired"
            );
            Ok(SingleInstance::Primary(SingleInstanceServer { receiver }))
        }
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            notify_existing_instance()?;
            Ok(SingleInstance::AlreadyRunning)
        }
        Err(error) => Err(PlatformError::OperationFailed(format!(
            "无法创建单实例监听 {SINGLE_INSTANCE_ADDR}: {error}"
        ))
        .into()),
    }
}

fn spawn_single_instance_listener(
    listener: TcpListener,
    sender: mpsc::Sender<SingleInstanceEvent>,
) {
    thread::Builder::new()
        .name("air-single-instance".to_string())
        .spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        if let Err(error) = stream.set_read_timeout(Some(IPC_TIMEOUT)) {
                            tracing::warn!(%error, "failed to set single instance read timeout");
                        }
                        let mut buffer = [0_u8; 64];
                        match stream.read(&mut buffer) {
                            Ok(read) if is_show_window_request(&buffer[..read]) => {
                                if sender.send(SingleInstanceEvent::ShowWindow).is_err() {
                                    tracing::debug!(
                                        "single instance receiver dropped; listener exiting"
                                    );
                                    break;
                                }
                            }
                            Ok(read) => {
                                tracing::warn!(
                                    request = %redact_log_value(
                                        &String::from_utf8_lossy(&buffer[..read])
                                    ),
                                    "ignored unknown single instance request"
                                );
                            }
                            Err(error) => {
                                tracing::warn!(%error, "failed to read single instance request");
                            }
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "single instance listener accept failed");
                    }
                }
            }
        })
        .expect("single instance listener thread should spawn");
}

fn notify_existing_instance() -> AppResult<()> {
    let addr: SocketAddr = SINGLE_INSTANCE_ADDR.parse().map_err(|error| {
        PlatformError::OperationFailed(format!(
            "单实例监听地址无效 {SINGLE_INSTANCE_ADDR}: {error}"
        ))
    })?;
    let mut stream = TcpStream::connect_timeout(&addr, IPC_TIMEOUT).map_err(|error| {
        PlatformError::OperationFailed(format!("Air 已在运行，但无法通知已有窗口恢复: {error}"))
    })?;
    stream
        .set_write_timeout(Some(IPC_TIMEOUT))
        .map_err(|error| {
            PlatformError::OperationFailed(format!("无法设置单实例写入超时: {error}"))
        })?;
    stream.write_all(SHOW_WINDOW_REQUEST).map_err(|error| {
        PlatformError::OperationFailed(format!("无法发送已有窗口恢复请求: {error}"))
    })?;
    tracing::info!("existing Air instance notified to show window");
    Ok(())
}

fn is_show_window_request(buffer: &[u8]) -> bool {
    buffer == SHOW_WINDOW_REQUEST
        || buffer.strip_suffix(b"\r\n")
            == Some(&SHOW_WINDOW_REQUEST[..SHOW_WINDOW_REQUEST.len() - 1])
}

#[cfg(test)]
mod tests {
    use super::is_show_window_request;

    #[test]
    fn show_window_request_matches_exact_protocol_line() {
        assert!(is_show_window_request(b"air.show-window.v1\n"));
        assert!(is_show_window_request(b"air.show-window.v1\r\n"));
        assert!(!is_show_window_request(b"air.show-window.v2\n"));
    }
}
