#![allow(async_fn_in_trait)]

use futures_util::{SinkExt, StreamExt};
use rustbac_datalink::{DataLink, DataLinkAddress, DataLinkError};
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::lookup_host;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const CHANNEL_DEPTH: usize = 128;

#[derive(Debug, Clone)]
pub struct BacnetScTransport {
    endpoint: String,
    peer_address: DataLinkAddress,
    outbound: mpsc::Sender<Vec<u8>>,
    inbound: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
}

impl BacnetScTransport {
    pub async fn connect(endpoint: impl Into<String>) -> Result<Self, DataLinkError> {
        let endpoint = endpoint.into();
        let peer_address = resolve_peer_address(&endpoint).await?;

        let (socket, _) = connect_async(endpoint.as_str())
            .await
            .map_err(|err| ws_io_error(io::ErrorKind::ConnectionRefused, err))?;
        let (mut writer, mut reader) = socket.split();

        let (outbound_tx, mut outbound_rx) = mpsc::channel::<Vec<u8>>(CHANNEL_DEPTH);
        let (inbound_tx, inbound_rx) = mpsc::channel::<Vec<u8>>(CHANNEL_DEPTH);

        tokio::spawn(async move {
            while let Some(frame) = outbound_rx.recv().await {
                if writer.send(Message::Binary(frame)).await.is_err() {
                    return;
                }
            }
            let _ = writer.close().await;
        });

        tokio::spawn(async move {
            while let Some(next) = reader.next().await {
                let message = match next {
                    Ok(message) => message,
                    Err(_) => break,
                };

                match message {
                    Message::Binary(payload) => {
                        if inbound_tx.send(payload.to_vec()).await.is_err() {
                            break;
                        }
                    }
                    Message::Text(text) => {
                        log::debug!("ignoring non-binary BACnet/SC websocket frame: {text}");
                    }
                    _ => {}
                }
            }
        });

        Ok(Self {
            endpoint,
            peer_address,
            outbound: outbound_tx,
            inbound: Arc::new(Mutex::new(inbound_rx)),
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn peer_address(&self) -> DataLinkAddress {
        self.peer_address
    }
}

impl DataLink for BacnetScTransport {
    async fn send(&self, _address: DataLinkAddress, payload: &[u8]) -> Result<(), DataLinkError> {
        self.outbound.send(payload.to_vec()).await.map_err(|_| {
            DataLinkError::Io(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "BACnet/SC websocket sender task stopped",
            ))
        })
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, DataLinkAddress), DataLinkError> {
        let mut inbound = self.inbound.lock().await;
        let payload = inbound.recv().await.ok_or_else(|| {
            DataLinkError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "BACnet/SC websocket receiver task stopped",
            ))
        })?;
        if payload.len() > buf.len() {
            return Err(DataLinkError::FrameTooLarge);
        }
        buf[..payload.len()].copy_from_slice(&payload);
        Ok((payload.len(), self.peer_address))
    }
}

fn ws_io_error(kind: io::ErrorKind, err: impl std::fmt::Display) -> DataLinkError {
    DataLinkError::Io(io::Error::new(
        kind,
        format!("BACnet/SC websocket error: {err}"),
    ))
}

async fn resolve_peer_address(endpoint: &str) -> Result<DataLinkAddress, DataLinkError> {
    let (scheme, remainder) = endpoint.split_once("://").ok_or_else(|| {
        DataLinkError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid BACnet/SC endpoint '{endpoint}'"),
        ))
    })?;
    let default_port = match scheme {
        "ws" => 80,
        "wss" => 443,
        _ => {
            return Err(DataLinkError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported BACnet/SC endpoint scheme '{scheme}'"),
            )))
        }
    };
    let authority = remainder.split('/').next().unwrap_or_default();
    if authority.is_empty() {
        return Err(DataLinkError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("BACnet/SC endpoint '{endpoint}' is missing host"),
        )));
    }
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    if authority.is_empty() {
        return Err(DataLinkError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("BACnet/SC endpoint '{endpoint}' is missing host"),
        )));
    }

    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let (ipv6_host, suffix) = rest.split_once(']').ok_or_else(|| {
            DataLinkError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid IPv6 host in BACnet/SC endpoint '{endpoint}'"),
            ))
        })?;
        let port = if suffix.is_empty() {
            default_port
        } else if let Some(raw_port) = suffix.strip_prefix(':') {
            raw_port.parse::<u16>().map_err(|_| {
                DataLinkError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid BACnet/SC endpoint port in '{endpoint}'"),
                ))
            })?
        } else {
            return Err(DataLinkError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid BACnet/SC endpoint authority '{authority}'"),
            )));
        };
        (ipv6_host.to_string(), port)
    } else {
        match authority.rsplit_once(':') {
            Some((host, raw_port)) if !host.is_empty() && !raw_port.is_empty() => {
                let port = raw_port.parse::<u16>().map_err(|_| {
                    DataLinkError::Io(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("invalid BACnet/SC endpoint port in '{endpoint}'"),
                    ))
                })?;
                (host.to_string(), port)
            }
            _ => (authority.to_string(), default_port),
        }
    };

    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(DataLinkAddress::Ip(SocketAddr::new(ip, port)));
    }

    let mut addrs = lookup_host((host.as_str(), port))
        .await
        .map_err(DataLinkError::Io)?;
    addrs.next().map(DataLinkAddress::Ip).ok_or_else(|| {
        DataLinkError::Io(io::Error::new(
            io::ErrorKind::NotFound,
            format!("unable to resolve BACnet/SC host '{host}'"),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::BacnetScTransport;
    use futures_util::{SinkExt, StreamExt};
    use rustbac_datalink::{DataLink, DataLinkAddress, DataLinkError};
    use std::net::SocketAddr;
    use tokio::net::TcpListener;
    use tokio::time::{timeout, Duration};
    use tokio_tungstenite::accept_async;
    use tokio_tungstenite::tungstenite::Message;

    async fn spawn_echo_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(stream).await.unwrap();
            while let Some(next) = ws.next().await {
                let msg = next.unwrap();
                match msg {
                    Message::Binary(payload) => {
                        ws.send(Message::Binary(payload)).await.unwrap();
                    }
                    Message::Ping(payload) => {
                        ws.send(Message::Pong(payload)).await.unwrap();
                    }
                    Message::Close(frame) => {
                        let _ = ws.close(frame).await;
                        break;
                    }
                    Message::Pong(_) | Message::Text(_) => {}
                    _ => {}
                }
            }
        });
        (addr, task)
    }

    #[tokio::test]
    async fn connect_sets_endpoint_and_peer_address() {
        let (addr, server) = spawn_echo_server().await;
        let endpoint = format!("ws://{addr}/hub");
        let transport = BacnetScTransport::connect(endpoint.clone()).await.unwrap();
        assert_eq!(transport.endpoint(), endpoint);
        assert_eq!(transport.peer_address(), DataLinkAddress::Ip(addr));
        drop(transport);
        server.abort();
    }

    #[tokio::test]
    async fn send_and_recv_binary_payload() {
        let (addr, server) = spawn_echo_server().await;
        let transport = BacnetScTransport::connect(format!("ws://{addr}/hub"))
            .await
            .unwrap();

        transport
            .send(DataLinkAddress::Ip(addr), &[1, 2, 3, 4])
            .await
            .unwrap();

        let mut out = [0u8; 16];
        let (n, src) = timeout(Duration::from_secs(1), transport.recv(&mut out))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(n, 4);
        assert_eq!(&out[..4], &[1, 2, 3, 4]);
        assert_eq!(src, DataLinkAddress::Ip(addr));

        drop(transport);
        server.abort();
    }

    #[tokio::test]
    async fn recv_reports_frame_too_large() {
        let (addr, server) = spawn_echo_server().await;
        let transport = BacnetScTransport::connect(format!("ws://{addr}/hub"))
            .await
            .unwrap();
        transport
            .send(DataLinkAddress::Ip(addr), &[9, 8, 7, 6])
            .await
            .unwrap();

        let mut out = [0u8; 2];
        let err = transport.recv(&mut out).await.unwrap_err();
        assert!(matches!(err, DataLinkError::FrameTooLarge));

        drop(transport);
        server.abort();
    }

    #[tokio::test]
    async fn connect_rejects_invalid_endpoint() {
        let err = BacnetScTransport::connect("not a url").await.unwrap_err();
        assert!(matches!(err, DataLinkError::Io(_)));
    }
}
