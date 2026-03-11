//! MS/TP transport implementing the `DataLink` trait.
//!
//! Implements the ASHRAE 135 Clause 9 master node token-passing state machine.
//! The state machine runs inside `recv()`: it handles Token, PollForMaster, and
//! timeout-based token generation internally, only returning to the caller when
//! a BACnet data frame addressed to this node (or broadcast) is received.

use crate::frame::{FrameType, MstpFrame, PREAMBLE};
use rustbac_datalink::{DataLink, DataLinkAddress, DataLinkError};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// ASHRAE 135 Clause 9 constants
// ---------------------------------------------------------------------------

/// Time without seeing a token before we attempt to claim one.
const T_NO_TOKEN: Duration = Duration::from_millis(500);

/// Time to wait for a reply to PollForMaster before giving up.
const T_REPLY_TIMEOUT: Duration = Duration::from_millis(255);

/// Number of token passes before polling for a new master station.
const N_POLL: u8 = 50;

/// Maximum retries when passing the token with no successor response.
#[allow(dead_code)]
const MAX_RETRY_TOKEN: u8 = 1;

/// MS/TP broadcast MAC address.
const BROADCAST: u8 = 0xFF;

// ---------------------------------------------------------------------------
// Async serial trait
// ---------------------------------------------------------------------------

/// Combined async read/write trait for serial port abstraction.
pub trait AsyncSerial: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> AsyncSerial for T {}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the MS/TP transport.
#[derive(Debug, Clone)]
pub struct MstpConfig {
    /// Serial port path (e.g. "/dev/ttyUSB0" or "COM3").
    pub port: String,
    /// Baud rate (9600, 19200, 38400, or 76800).
    pub baud_rate: u32,
    /// This node's MAC address (0-127).
    pub mac_address: u8,
    /// Highest MAC address to poll for new masters.
    pub max_master: u8,
    /// Max info frames to send before passing the token.
    pub max_info_frames: u8,
}

impl Default for MstpConfig {
    fn default() -> Self {
        Self {
            port: String::new(),
            baud_rate: 38400,
            mac_address: 0,
            max_master: 127,
            max_info_frames: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Queued frame (outgoing)
// ---------------------------------------------------------------------------

/// A frame queued for transmission when the token is held.
#[derive(Debug, Clone)]
pub struct QueuedFrame {
    pub destination: u8,
    pub data: Vec<u8>,
    pub expecting_reply: bool,
}

// ---------------------------------------------------------------------------
// Token state
// ---------------------------------------------------------------------------

/// State for the Clause 9 token-passing state machine.
#[derive(Debug)]
pub struct TokenState {
    /// The next station we will pass the token to.
    pub next_station: u8,
    /// The next station to poll for new master presence.
    pub poll_station: u8,
    /// Number of token passes since we last polled for a new master.
    pub token_count: u8,
    /// True when we believe we are the only master on the bus.
    pub sole_master: bool,
    /// Instant when we last saw (or sent) a token on the bus.
    pub last_token_seen: Instant,
    /// Number of consecutive token-pass retries without a response.
    retry_count: u8,
}

impl TokenState {
    fn new(mac_address: u8, max_master: u8) -> Self {
        let next = next_station_after(mac_address, max_master);
        Self {
            next_station: next,
            poll_station: next,
            token_count: 0,
            sole_master: false,
            last_token_seen: Instant::now(),
            retry_count: 0,
        }
    }
}

/// Compute the next valid master address after `current`, wrapping at `max_master`.
fn next_station_after(current: u8, max_master: u8) -> u8 {
    let next = current + 1;
    if next > max_master {
        0
    } else {
        next
    }
}

// ---------------------------------------------------------------------------
// MstpTransport
// ---------------------------------------------------------------------------

/// MS/TP transport — implements the `DataLink` trait for BACnet over RS-485 serial.
///
/// The transport handles frame encoding/decoding and the master node state machine
/// (token passing, poll-for-master) internally. Users interact through the standard
/// `DataLink::send()` and `DataLink::recv()` methods.
///
/// `send()` queues outgoing frames; `recv()` runs the state machine which
/// transmits queued frames when this node holds the token and returns incoming
/// BACnet data frames to the caller.
pub struct MstpTransport {
    config: MstpConfig,
    // Serial port is wrapped in Arc<Mutex> for shared access between send/recv
    port: Arc<Mutex<Box<dyn AsyncSerial>>>,
    // Receive buffer for partial frame assembly
    rx_buf: Arc<Mutex<Vec<u8>>>,
    // Outgoing frame queue — send() pushes, recv() drains when we hold the token
    tx_queue: Arc<Mutex<VecDeque<QueuedFrame>>>,
    // Token-passing state machine state
    state: Arc<Mutex<TokenState>>,
}

impl MstpTransport {
    /// Create a new MS/TP transport with the given configuration.
    ///
    /// Opens the serial port and prepares the transport for communication.
    pub async fn new(config: MstpConfig) -> Result<Self, DataLinkError> {
        let builder = tokio_serial::new(&config.port, config.baud_rate)
            .data_bits(tokio_serial::DataBits::Eight)
            .parity(tokio_serial::Parity::None)
            .stop_bits(tokio_serial::StopBits::One);

        let port = tokio_serial::SerialStream::open(&builder).map_err(|e| {
            DataLinkError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to open serial port {}: {e}", config.port),
            ))
        })?;

        let state = TokenState::new(config.mac_address, config.max_master);

        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            tx_queue: Arc::new(Mutex::new(VecDeque::new())),
            port: Arc::new(Mutex::new(Box::new(port))),
            rx_buf: Arc::new(Mutex::new(Vec::with_capacity(1600))),
            config,
        })
    }

    /// Create a transport from an existing async reader/writer (for testing).
    pub fn from_stream(config: MstpConfig, stream: Box<dyn AsyncSerial>) -> Self {
        let state = TokenState::new(config.mac_address, config.max_master);
        Self {
            state: Arc::new(Mutex::new(state)),
            tx_queue: Arc::new(Mutex::new(VecDeque::new())),
            port: Arc::new(Mutex::new(stream)),
            rx_buf: Arc::new(Mutex::new(Vec::with_capacity(1600))),
            config,
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Write a raw frame to the serial port.
    async fn write_frame(
        port: &mut Box<dyn AsyncSerial>,
        frame: &MstpFrame,
    ) -> Result<(), DataLinkError> {
        let encoded = frame.encode();
        port.write_all(&encoded).await.map_err(|e| {
            DataLinkError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Serial write failed: {e}"),
            ))
        })?;
        port.flush().await.map_err(|e| {
            DataLinkError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Serial flush failed: {e}"),
            ))
        })?;
        Ok(())
    }

    /// Send a Token frame to `destination`.
    async fn send_token(
        port: &mut Box<dyn AsyncSerial>,
        source: u8,
        destination: u8,
    ) -> Result<(), DataLinkError> {
        let frame = MstpFrame {
            frame_type: FrameType::Token,
            destination,
            source,
            data: Vec::new(),
        };
        Self::write_frame(port, &frame).await
    }

    /// Send a PollForMaster frame to `destination`.
    async fn send_poll_for_master(
        port: &mut Box<dyn AsyncSerial>,
        source: u8,
        destination: u8,
    ) -> Result<(), DataLinkError> {
        let frame = MstpFrame {
            frame_type: FrameType::PollForMaster,
            destination,
            source,
            data: Vec::new(),
        };
        Self::write_frame(port, &frame).await
    }

    /// Send a ReplyToPollForMaster frame to `destination`.
    async fn send_reply_to_poll(
        port: &mut Box<dyn AsyncSerial>,
        source: u8,
        destination: u8,
    ) -> Result<(), DataLinkError> {
        let frame = MstpFrame {
            frame_type: FrameType::ReplyToPollForMaster,
            destination,
            source,
            data: Vec::new(),
        };
        Self::write_frame(port, &frame).await
    }

    /// Drain up to `max_info_frames` queued data frames onto the wire.
    /// Returns the number of frames sent.
    async fn send_queued_frames(
        port: &mut Box<dyn AsyncSerial>,
        queue: &mut VecDeque<QueuedFrame>,
        source: u8,
        max_info_frames: u8,
    ) -> Result<u8, DataLinkError> {
        let mut sent = 0u8;
        while sent < max_info_frames {
            let queued = match queue.pop_front() {
                Some(q) => q,
                None => break,
            };
            let frame_type = if queued.expecting_reply {
                FrameType::BacnetDataExpectingReply
            } else {
                FrameType::BacnetDataNotExpectingReply
            };
            let frame = MstpFrame {
                frame_type,
                destination: queued.destination,
                source,
                data: queued.data,
            };
            Self::write_frame(port, &frame).await?;
            sent += 1;
        }
        Ok(sent)
    }

    /// Try to read a complete frame from `rx_buf`, reading more bytes from the
    /// serial port if needed. Returns `None` on timeout.
    async fn read_frame_timeout(
        port: &mut Box<dyn AsyncSerial>,
        rx_buf: &mut Vec<u8>,
        timeout: Duration,
    ) -> Result<Option<MstpFrame>, DataLinkError> {
        let deadline = Instant::now() + timeout;
        loop {
            // Try to decode from buffer
            if let Some(frame_start) = find_preamble(rx_buf) {
                let after_preamble = frame_start + 2;
                if let Some((frame, consumed)) = MstpFrame::decode(&rx_buf[after_preamble..]) {
                    let total_consumed = after_preamble + consumed;
                    rx_buf.drain(..total_consumed);
                    return Ok(Some(frame));
                }
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }

            // Read more data with timeout
            let mut tmp = [0u8; 512];
            match tokio::time::timeout(remaining, port.read(&mut tmp)).await {
                Ok(Ok(0)) => {
                    return Err(DataLinkError::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Serial port closed",
                    )));
                }
                Ok(Ok(n)) => {
                    rx_buf.extend_from_slice(&tmp[..n]);
                }
                Ok(Err(e)) => {
                    return Err(DataLinkError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Serial read failed: {e}"),
                    )));
                }
                Err(_) => {
                    // Timeout expired
                    return Ok(None);
                }
            }

            // Prevent unbounded buffer growth
            if rx_buf.len() > 8192 {
                let drain_to = rx_buf.len() - 1600;
                rx_buf.drain(..drain_to);
            }
        }
    }
}

impl DataLink for MstpTransport {
    /// Queue a BACnet data frame for transmission.
    ///
    /// The frame will be sent the next time this node holds the token.
    async fn send(&self, address: DataLinkAddress, payload: &[u8]) -> Result<(), DataLinkError> {
        let destination = match address {
            DataLinkAddress::Mstp(mac) => mac,
            _ => BROADCAST,
        };

        let queued = QueuedFrame {
            destination,
            data: payload.to_vec(),
            expecting_reply: false,
        };

        self.tx_queue.lock().await.push_back(queued);
        Ok(())
    }

    /// Run the token-passing state machine, returning the next BACnet data
    /// frame addressed to this node (or broadcast).
    ///
    /// Internally handles:
    /// - **Token received**: drain queued frames (up to `max_info_frames`),
    ///   then pass the token to `next_station`.
    /// - **PollForMaster received**: reply with `ReplyToPollForMaster`.
    /// - **No-token timeout**: attempt to claim the token by polling for a
    ///   successor starting at `mac_address + 1`.
    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, DataLinkAddress), DataLinkError> {
        let my_mac = self.config.mac_address;
        let max_master = self.config.max_master;
        let max_info = self.config.max_info_frames;

        loop {
            // Determine how long until T_no_token expires
            let time_remaining = {
                let st = self.state.lock().await;
                T_NO_TOKEN.saturating_sub(st.last_token_seen.elapsed())
            };

            // Try to read a frame with the remaining no-token budget
            let frame_opt = {
                let mut port = self.port.lock().await;
                let mut rx_buf = self.rx_buf.lock().await;
                Self::read_frame_timeout(&mut port, &mut rx_buf, time_remaining).await?
            };

            match frame_opt {
                Some(frame) => {
                    // -------------------------------------------------------
                    // Token addressed to us
                    // -------------------------------------------------------
                    if frame.frame_type == FrameType::Token && frame.destination == my_mac {
                        log::trace!("MSTP({my_mac}): received Token from {}", frame.source);

                        {
                            let mut st = self.state.lock().await;
                            st.last_token_seen = Instant::now();
                            st.retry_count = 0;
                        }

                        // Send queued data frames
                        {
                            let mut port = self.port.lock().await;
                            let mut queue = self.tx_queue.lock().await;
                            Self::send_queued_frames(&mut port, &mut queue, my_mac, max_info)
                                .await?;
                        }

                        // Periodic poll-for-master (every N_POLL token passes)
                        let should_poll = {
                            let mut st = self.state.lock().await;
                            st.token_count += 1;
                            if st.token_count >= N_POLL {
                                st.token_count = 0;
                                true
                            } else {
                                false
                            }
                        };

                        if should_poll {
                            let poll_target = self.state.lock().await.poll_station;
                            if poll_target != my_mac {
                                // Send PollForMaster and wait for reply
                                {
                                    let mut port = self.port.lock().await;
                                    Self::send_poll_for_master(&mut port, my_mac, poll_target)
                                        .await?;
                                }

                                let reply = {
                                    let mut port = self.port.lock().await;
                                    let mut rx_buf = self.rx_buf.lock().await;
                                    Self::read_frame_timeout(
                                        &mut port,
                                        &mut rx_buf,
                                        T_REPLY_TIMEOUT,
                                    )
                                    .await?
                                };

                                if let Some(ref r) = reply {
                                    if r.frame_type == FrameType::ReplyToPollForMaster
                                        && r.source == poll_target
                                    {
                                        // New master found — insert it as our next_station
                                        let mut st = self.state.lock().await;
                                        st.next_station = poll_target;
                                        st.sole_master = false;
                                        log::debug!(
                                            "MSTP({my_mac}): discovered new master at {poll_target}"
                                        );
                                    }
                                }
                            }

                            // Advance poll_station for next time
                            {
                                let mut st = self.state.lock().await;
                                st.poll_station = next_station_after(st.poll_station, max_master);
                                if st.poll_station == my_mac {
                                    st.poll_station = next_station_after(my_mac, max_master);
                                }
                            }
                        }

                        // Pass the token to next_station
                        let next = self.state.lock().await.next_station;
                        {
                            let mut port = self.port.lock().await;
                            Self::send_token(&mut port, my_mac, next).await?;
                        }
                        {
                            let mut st = self.state.lock().await;
                            st.last_token_seen = Instant::now();
                        }

                        continue;
                    }

                    // -------------------------------------------------------
                    // PollForMaster addressed to us — reply immediately
                    // -------------------------------------------------------
                    if frame.frame_type == FrameType::PollForMaster && frame.destination == my_mac {
                        log::trace!(
                            "MSTP({my_mac}): PollForMaster from {}, replying",
                            frame.source
                        );
                        let mut port = self.port.lock().await;
                        Self::send_reply_to_poll(&mut port, my_mac, frame.source).await?;
                        continue;
                    }

                    // -------------------------------------------------------
                    // Token for another station — note that the bus is alive
                    // -------------------------------------------------------
                    if frame.frame_type == FrameType::Token {
                        let mut st = self.state.lock().await;
                        st.last_token_seen = Instant::now();
                        continue;
                    }

                    // -------------------------------------------------------
                    // BACnet data frame addressed to us or broadcast
                    // -------------------------------------------------------
                    if (frame.frame_type == FrameType::BacnetDataExpectingReply
                        || frame.frame_type == FrameType::BacnetDataNotExpectingReply)
                        && (frame.destination == my_mac || frame.destination == BROADCAST)
                    {
                        let len = frame.data.len().min(buf.len());
                        buf[..len].copy_from_slice(&frame.data[..len]);
                        let source = DataLinkAddress::Mstp(frame.source);
                        return Ok((len, source));
                    }

                    // Other frames (TestRequest, ReplyToPollForMaster not for us, etc.)
                    // — ignore and loop.
                    continue;
                }

                None => {
                    // -------------------------------------------------------
                    // T_no_token expired — attempt to claim the token
                    // -------------------------------------------------------
                    log::debug!("MSTP({my_mac}): T_no_token expired, claiming token");

                    let mut found_successor = false;
                    let mut poll_addr = next_station_after(my_mac, max_master);
                    let start = poll_addr;

                    // Walk all addresses from (my_mac+1) to max_master then 0..my_mac
                    loop {
                        if poll_addr == my_mac {
                            break;
                        }

                        {
                            let mut port = self.port.lock().await;
                            Self::send_poll_for_master(&mut port, my_mac, poll_addr).await?;
                        }

                        let reply = {
                            let mut port = self.port.lock().await;
                            let mut rx_buf = self.rx_buf.lock().await;
                            Self::read_frame_timeout(&mut port, &mut rx_buf, T_REPLY_TIMEOUT)
                                .await?
                        };

                        if let Some(ref r) = reply {
                            if r.frame_type == FrameType::ReplyToPollForMaster
                                && r.source == poll_addr
                            {
                                // Found a successor
                                let mut st = self.state.lock().await;
                                st.next_station = poll_addr;
                                st.sole_master = false;
                                st.last_token_seen = Instant::now();
                                found_successor = true;
                                log::debug!(
                                    "MSTP({my_mac}): found successor at {poll_addr} during claim"
                                );
                                break;
                            }
                        }

                        poll_addr = next_station_after(poll_addr, max_master);
                        if poll_addr == start {
                            // Wrapped all the way around
                            break;
                        }
                    }

                    if !found_successor {
                        // We are the sole master
                        let mut st = self.state.lock().await;
                        st.sole_master = true;
                        st.next_station = my_mac;
                        st.last_token_seen = Instant::now();
                        log::debug!("MSTP({my_mac}): sole master");
                    }

                    // Now use the token: send queued frames and pass token
                    {
                        let mut port = self.port.lock().await;
                        let mut queue = self.tx_queue.lock().await;
                        Self::send_queued_frames(&mut port, &mut queue, my_mac, max_info).await?;
                    }

                    let (next, sole) = {
                        let st = self.state.lock().await;
                        (st.next_station, st.sole_master)
                    };

                    if !sole {
                        let mut port = self.port.lock().await;
                        Self::send_token(&mut port, my_mac, next).await?;
                    }

                    {
                        let mut st = self.state.lock().await;
                        st.last_token_seen = Instant::now();
                    }

                    continue;
                }
            }
        }
    }
}

/// Find the start of an MS/TP preamble (0x55 0xFF) in a buffer.
fn find_preamble(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == PREAMBLE)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::ReadBuf;

    // -----------------------------------------------------------------------
    // Mock serial stream for testing
    // -----------------------------------------------------------------------

    /// A mock serial port backed by in-memory buffers.
    struct MockSerial {
        /// Data available to be read (simulates incoming serial bytes).
        rx_data: Vec<u8>,
        rx_pos: usize,
        /// Data written by the transport (captured for assertions).
        tx_data: Vec<u8>,
    }

    impl MockSerial {
        fn new(rx_data: Vec<u8>) -> Self {
            Self {
                rx_data,
                rx_pos: 0,
                tx_data: Vec::new(),
            }
        }
    }

    impl AsyncRead for MockSerial {
        fn poll_read(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            let remaining = &self.rx_data[self.rx_pos..];
            if remaining.is_empty() {
                // Return Pending to simulate waiting (avoids EOF error)
                return Poll::Pending;
            }
            let to_copy = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..to_copy]);
            self.rx_pos += to_copy;
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for MockSerial {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            self.tx_data.extend_from_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    // -----------------------------------------------------------------------
    // Original tests (preserved)
    // -----------------------------------------------------------------------

    #[test]
    fn find_preamble_at_start() {
        let buf = [0x55, 0xFF, 0x00, 0x01];
        assert_eq!(find_preamble(&buf), Some(0));
    }

    #[test]
    fn find_preamble_offset() {
        let buf = [0x00, 0x00, 0x55, 0xFF, 0x01];
        assert_eq!(find_preamble(&buf), Some(2));
    }

    #[test]
    fn find_preamble_missing() {
        let buf = [0x00, 0x55, 0x00, 0xFF];
        assert_eq!(find_preamble(&buf), None);
    }

    #[test]
    fn mstp_config_defaults() {
        let config = MstpConfig::default();
        assert_eq!(config.baud_rate, 38400);
        assert_eq!(config.mac_address, 0);
        assert_eq!(config.max_master, 127);
        assert_eq!(config.max_info_frames, 1);
    }

    // -----------------------------------------------------------------------
    // State machine tests
    // -----------------------------------------------------------------------

    #[test]
    fn token_state_initialization() {
        let state = TokenState::new(5, 127);
        assert_eq!(state.next_station, 6);
        assert_eq!(state.poll_station, 6);
        assert_eq!(state.token_count, 0);
        assert!(!state.sole_master);
    }

    #[test]
    fn token_state_wrap_around() {
        // mac_address == max_master: next_station wraps to 0
        let state = TokenState::new(127, 127);
        assert_eq!(state.next_station, 0);
        assert_eq!(state.poll_station, 0);
    }

    #[test]
    fn next_station_after_basic() {
        assert_eq!(next_station_after(0, 127), 1);
        assert_eq!(next_station_after(126, 127), 127);
        assert_eq!(next_station_after(127, 127), 0);
        assert_eq!(next_station_after(5, 10), 6);
        assert_eq!(next_station_after(10, 10), 0);
    }

    #[test]
    fn send_queues_frame() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let config = MstpConfig {
                mac_address: 3,
                max_master: 127,
                ..Default::default()
            };
            let mock = MockSerial::new(Vec::new());
            let transport = MstpTransport::from_stream(config, Box::new(mock));

            // send() should queue rather than write to serial
            transport
                .send(DataLinkAddress::Mstp(10), &[0x01, 0x02])
                .await
                .unwrap();

            let queue = transport.tx_queue.lock().await;
            assert_eq!(queue.len(), 1);
            assert_eq!(queue[0].destination, 10);
            assert_eq!(queue[0].data, vec![0x01, 0x02]);
            assert!(!queue[0].expecting_reply);
        });
    }

    #[test]
    fn send_broadcast_uses_0xff() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let config = MstpConfig {
                mac_address: 1,
                ..Default::default()
            };
            let mock = MockSerial::new(Vec::new());
            let transport = MstpTransport::from_stream(config, Box::new(mock));

            // Non-Mstp address should broadcast
            transport
                .send(
                    DataLinkAddress::Ip("192.168.1.1:47808".parse().unwrap()),
                    &[0xAA],
                )
                .await
                .unwrap();

            let queue = transport.tx_queue.lock().await;
            assert_eq!(queue[0].destination, 0xFF);
        });
    }

    #[tokio::test]
    async fn poll_for_master_generates_reply() {
        // Build a PollForMaster frame addressed to mac 5 from mac 1
        let poll_frame = MstpFrame {
            frame_type: FrameType::PollForMaster,
            destination: 5,
            source: 1,
            data: Vec::new(),
        };
        let encoded = poll_frame.encode();

        let config = MstpConfig {
            mac_address: 5,
            max_master: 127,
            ..Default::default()
        };
        let mock = MockSerial::new(encoded);
        let transport = MstpTransport::from_stream(config, Box::new(mock));

        // recv() should handle the PollForMaster internally (reply) and then
        // block waiting for more data. We use a timeout to verify it didn't
        // return to the caller with the poll frame.
        let result = tokio::time::timeout(Duration::from_millis(100), async {
            let mut buf = [0u8; 512];
            transport.recv(&mut buf).await
        })
        .await;

        // Should timeout (no data frame to return), meaning it handled the poll
        assert!(result.is_err(), "recv should not return for PollForMaster");

        // Verify a ReplyToPollForMaster was written to the serial port
        let port = transport.port.lock().await;
        // Downcast to access tx_data
        let mock_ref: &MockSerial = unsafe {
            // The Box<dyn AsyncSerial> points to a MockSerial
            &*(&**port as *const dyn AsyncSerial as *const MockSerial)
        };
        // Decode the reply from written bytes
        if let Some(start) = find_preamble(&mock_ref.tx_data) {
            let (reply, _) = MstpFrame::decode(&mock_ref.tx_data[start + 2..])
                .expect("should decode reply frame");
            assert_eq!(reply.frame_type, FrameType::ReplyToPollForMaster);
            assert_eq!(reply.source, 5);
            assert_eq!(reply.destination, 1);
        } else {
            panic!("No reply frame written to serial port");
        }
    }

    #[tokio::test]
    async fn token_triggers_queued_frame_send() {
        // Build a Token frame addressed to mac 3, followed by a BACnet data
        // frame so recv() has something to return.
        let token_frame = MstpFrame {
            frame_type: FrameType::Token,
            destination: 3,
            source: 1,
            data: Vec::new(),
        };
        let data_frame = MstpFrame {
            frame_type: FrameType::BacnetDataNotExpectingReply,
            destination: 3,
            source: 10,
            data: vec![0xDE, 0xAD],
        };

        let mut rx_bytes = token_frame.encode();
        rx_bytes.extend_from_slice(&data_frame.encode());

        let config = MstpConfig {
            mac_address: 3,
            max_master: 127,
            max_info_frames: 5,
            ..Default::default()
        };
        let mock = MockSerial::new(rx_bytes);
        let transport = MstpTransport::from_stream(config, Box::new(mock));

        // Queue a frame before recv()
        transport
            .send(DataLinkAddress::Mstp(10), &[0xCA, 0xFE])
            .await
            .unwrap();
        assert_eq!(transport.tx_queue.lock().await.len(), 1);

        // recv() should: handle Token → send queued frame + pass token → then
        // return the data frame.
        let mut buf = [0u8; 512];
        let result = tokio::time::timeout(Duration::from_secs(2), transport.recv(&mut buf)).await;

        match result {
            Ok(Ok((len, addr))) => {
                assert_eq!(len, 2);
                assert_eq!(&buf[..2], &[0xDE, 0xAD]);
                assert_eq!(addr, DataLinkAddress::Mstp(10));
            }
            Ok(Err(e)) => panic!("recv error: {e}"),
            Err(_) => panic!("recv timed out"),
        }

        // Queue should be drained
        assert_eq!(transport.tx_queue.lock().await.len(), 0);

        // Verify the transport wrote the queued data frame + a token pass
        let port = transport.port.lock().await;
        let mock_ref: &MockSerial =
            unsafe { &*(&**port as *const dyn AsyncSerial as *const MockSerial) };

        // Parse all written frames
        let mut written_frames = Vec::new();
        let mut pos = 0;
        while pos < mock_ref.tx_data.len() {
            if let Some(preamble_offset) = find_preamble(&mock_ref.tx_data[pos..]) {
                let abs = pos + preamble_offset + 2;
                if abs < mock_ref.tx_data.len() {
                    if let Some((f, consumed)) = MstpFrame::decode(&mock_ref.tx_data[abs..]) {
                        written_frames.push(f);
                        pos = abs + consumed;
                        continue;
                    }
                }
            }
            break;
        }

        // Should have written: queued data frame, then token pass
        assert!(
            written_frames.len() >= 2,
            "expected at least 2 written frames, got {}",
            written_frames.len()
        );
        assert_eq!(
            written_frames[0].frame_type,
            FrameType::BacnetDataNotExpectingReply
        );
        assert_eq!(written_frames[0].data, vec![0xCA, 0xFE]);
        assert_eq!(written_frames[0].destination, 10);

        // Last frame should be a Token pass
        let last = written_frames.last().unwrap();
        assert_eq!(last.frame_type, FrameType::Token);
    }
}
