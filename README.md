# rust-bac

Rust BACnet/IP workspace with a `no_std` core encoder/decoder, async BACnet/IP transport, high-level client API, and CLI tools.

## Crates

- `crates/rustbac-core`: BACnet encoding, NPDU/APDU, types, and service payloads.
- `crates/rustbac-datalink`: BACnet/IP datalink (BVLC/BIP), BBMD/FDR helpers.
- `crates/rustbac-bacnet-sc`: BACnet/SC WebSocket transport adapter (ws/wss backend wiring).
- `crates/rustbac-client`: high-level async client API.
- `crates/rustbac-tools`: CLI binaries (`whois`, `whohas`, `readprop`, `writeprop`, `subcov`, `readrange`, `readfile`, `writefile`, `dcc`, `reinit`, `timesync`, `ackalarm`, `alarmsummary`, `enrollsummary`, `eventinfo`, `eventnotify`, `readbdt`, `writebdt`, `readfdt`, `deletefdt`, `createobj`, `deleteobj`, `addlist`, `removelist`).

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
```

## Current Highlights

- Who-Is / I-Am discovery
- Who-Has / I-Have object discovery
- Read/Write Property
- Read/Write Property Multiple
- ReadRange (by-position, by-sequence, by-time)
- Atomic Read File (stream + record)
- Atomic Write File (stream + record)
- Create Object / Delete Object
- Add List Element / Remove List Element
- Subscribe COV and Subscribe COV Property
- COV notification handling (confirmed + unconfirmed)
- Event notification handling (confirmed + unconfirmed)
- Event/Alarm services:
  - AcknowledgeAlarm
  - GetAlarmSummary
  - GetEnrollmentSummary
  - GetEventInformation
- Device management services:
  - DeviceCommunicationControl
  - ReinitializeDevice
- Time synchronization services:
  - TimeSynchronization
  - UTCTimeSynchronization
- Segmented ComplexAck reassembly
- Duplicate segmented ComplexAck tolerance during reassembly
- Segmented confirmed-request transmit (for oversized confirmed requests, configurable window + bounded retransmit retries)
- Confirmed-service receive loops tolerate transient invalid frames while awaiting responses
- Foreign Device Registration + BBMD table operations (BDT/FDT)
- BBMD/FDR admin CLI tools (`readbdt`, `writebdt`, `readfdt`, `deletefdt`)
- BACnet/SC websocket transport (`BacnetScTransport`, `BacnetClient::new_sc`)
- Typed remote BACnet error detail mapping (class/code when present)
- Golden packet fixtures in `crates/rustbac-core/tests/golden_packets.rs`
- Golden corpus fixture loader test in `crates/rustbac-core/tests/golden_corpus.rs` (loads `fixtures/golden/*.hex`)

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
