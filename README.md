# rust-bac

Rust BACnet/IP workspace with a `no_std` core encoder/decoder, async BACnet/IP transport, high-level client API, server/responder scaffolding, and CLI tools.

## Crates

- `crates/rustbac-core`: BACnet encoding, NPDU/APDU, types, and service payloads.
- `crates/rustbac-datalink`: BACnet/IP datalink (BVLC/BIP), BBMD/FDR helpers.
- `crates/rustbac-bacnet-sc`: BACnet/SC WebSocket transport adapter (ws/wss backend wiring).
- `crates/rustbac-client`: high-level async client API, COV manager, server scaffolding.
- `crates/rustbac-tools`: CLI binaries (`whois`, `whohas`, `readprop`, `writeprop`, `writepropms`, `subcov`, `readrange`, `readfile`, `writefile`, `dcc`, `reinit`, `timesync`, `ackalarm`, `alarmsummary`, `enrollsummary`, `eventinfo`, `eventnotify`, `readbdt`, `writebdt`, `readfdt`, `deletefdt`, `createobj`, `deleteobj`, `addlist`, `removelist`, `listen`, `privatetransfer`, `simulator`, `walkdevice`).

## Quick Start

```bash
cargo test --workspace
```

```bash
bash scripts/run_interop_matrix.sh
```

```bash
cargo run -p rustbac-tools --bin whois -- --help
cargo run -p rustbac-tools --bin whohas -- --help
cargo run -p rustbac-tools --bin readprop -- --help
cargo run -p rustbac-tools --bin writeprop -- --help
cargo run -p rustbac-tools --bin writepropms -- --help
cargo run -p rustbac-tools --bin subcov -- --help
cargo run -p rustbac-tools --bin readrange -- --help
cargo run -p rustbac-tools --bin readfile -- --help
cargo run -p rustbac-tools --bin writefile -- --help
cargo run -p rustbac-tools --bin dcc -- --help
cargo run -p rustbac-tools --bin reinit -- --help
cargo run -p rustbac-tools --bin timesync -- --help
cargo run -p rustbac-tools --bin ackalarm -- --help
cargo run -p rustbac-tools --bin alarmsummary -- --help
cargo run -p rustbac-tools --bin enrollsummary -- --help
cargo run -p rustbac-tools --bin eventinfo -- --help
cargo run -p rustbac-tools --bin eventnotify -- --help
cargo run -p rustbac-tools --bin readbdt -- --help
cargo run -p rustbac-tools --bin writebdt -- --help
cargo run -p rustbac-tools --bin readfdt -- --help
cargo run -p rustbac-tools --bin deletefdt -- --help
cargo run -p rustbac-tools --bin createobj -- --help
cargo run -p rustbac-tools --bin deleteobj -- --help
cargo run -p rustbac-tools --bin addlist -- --help
cargo run -p rustbac-tools --bin removelist -- --help
cargo run -p rustbac-tools --bin listen -- --help
cargo run -p rustbac-tools --bin privatetransfer -- --help
cargo run -p rustbac-tools --bin simulator -- --help
cargo run -p rustbac-tools --bin walkdevice -- --help
```

## Current Highlights

### Protocol services

- Who-Is / I-Am discovery (deduplication by device instance, not source address)
- Who-Has / I-Have object discovery
- Read/Write Property
- Read/Write Property Multiple
- `read_many` / `write_many` convenience helpers (batch read/write in a single round-trip)
- ReadRange (by-position, by-sequence, by-time)
- Atomic Read File (stream + record)
- Atomic Write File (stream + record)
- Create Object / Delete Object
- Add List Element / Remove List Element
- Subscribe COV and Subscribe COV Property
- COV notification handling (confirmed + unconfirmed)
- Event notification handling (confirmed + unconfirmed)
- Event/Alarm services: AcknowledgeAlarm, GetAlarmSummary, GetEnrollmentSummary, GetEventInformation
- Device management: DeviceCommunicationControl, ReinitializeDevice
- Time synchronization: TimeSynchronization, UTCTimeSynchronization
- ConfirmedPrivateTransfer (vendor-specific service invocation)
- Foreign Device Registration + BBMD table operations (BDT/FDT)

### Segmentation

- Segmented ComplexAck reassembly with duplicate-segment tolerance
- Segmented confirmed-request transmit (configurable window, bounded retransmit retries)
- Adaptive segment window: default window size 16; server-side SegmentAck proposals honoured
- Device capability caching: `MaxAPDU` from I-Am responses is cached and used to right-size segments for each peer

### Transports

- BACnet/IP (UDP/BVLC) with BBMD/FDR support
- BACnet/SC WebSocket transport (`BacnetScTransport`, `BacnetClient::new_sc`) with concurrent-recv safety via broadcast fan-out

### Server/responder

- `BacnetServer` trait + `ObjectStore` in-memory property store
- Handles ReadProperty, WriteProperty, ReadPropertyMultiple, Who-Is → I-Am, unknown services → Reject

### COV manager

- `CovManager` background manager: automatic renewal, silent-subscription detection, polling fallback
- Silent subscription detection correctly anchored to first-subscribe time — renewals do not reset the silence window

### Observability

- `tracing` feature flag: spans/events for confirmed-request lifecycle (invoke ID, service choice, peer address)
- Confirmed-service receive loops tolerate transient invalid frames

### Types & ergonomics

- `PropertyId::from_name("present-value")` and `impl Display for PropertyId` (hyphenated BACnet names)
- `ObjectType::from_name("analog-input")` and `impl Display for ObjectType`
- Typed remote BACnet error detail mapping (class + code enums when recognised)
- `serde` feature flag on all public types

### Testing & quality

- End-to-end integration tests against an in-memory `SimulatedDevice` (8 scenarios)
- Golden packet fixtures in `crates/rustbac-core/tests/golden_packets.rs`
- Golden corpus fixture loader in `crates/rustbac-core/tests/golden_corpus.rs`
- `cargo fuzz` harness with 4 targets (`fuzz_npdu_decode`, `fuzz_apdu_confirmed_decode`, `fuzz_bvlc_decode`, `fuzz_service_decode`)
- BBMD admin race fixed: all BBMD admin methods hold `request_io_lock`
- NPDU encoder derives control bits from option fields (no more mismatched headers)
- Bounded notification channel (256 default) with backpressure; segmented confirmed notifications rejected with Abort PDU

### CLI tools

- All services above available as standalone binaries in `rustbac-tools`
- `writepropms`: batch write multiple properties in a single WritePropertyMultiple call

## Delivery Docs

- `docs/BIBBS.md`: current BIBBs coverage map
- `docs/INTEROP_MATRIX.md`: interop execution matrix template
- `docs/INTEROP_RESULTS.md`: latest automated interop matrix run output
- `docs/INTEROP_RUNBOOK.md`: step-by-step completion checklist for BACnet/IP + BACnet/SC hardening
- `docs/INTEROP_RESULTS_LIVE.md`: live simulator/device run results

## Live Interop Command

```bash
bash scripts/run_live_interop.sh --ip <TARGET_IP>
```

## MSRV

- Rust `1.75.0` (pinned by `rust-toolchain.toml`)
