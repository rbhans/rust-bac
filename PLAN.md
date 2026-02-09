# BACnet Rust Protocol Stack Plan and Status

## Goal
Build a robust, production-grade BACnet/IP Rust stack with strong interoperability, predictable error behavior, and clear CLI tooling.

## Current Status Snapshot

| Phase | Scope | Status |
|---|---|---|
| Phase 1 | Core BACnet/IP client stack and tools | Complete |
| Phase 2 | Cross-subnet discovery + segmented response support | Complete |
| Phase 3 | Segmented request transmit + BBMD table operations + FDR ergonomics | Complete |
| Phase 4 | Full "go-to" breadth across BACnet services/profiles | Complete in-repo; external vendor-lab validation remains |

## Phase 1 (Complete)

- Workspace and crate layout established (`rustbac-core`, `rustbac-datalink`, `rustbac-client`, `rustbac-tools`).
- `no_std` core encoding/decoding and key BACnet types.
- NPDU/APDU primitives and service headers.
- Who-Is / I-Am discovery.
- Read/Write Property and Read/Write Property Multiple.
- BACnet/IP datalink transport over tokio UDP.
- CLI tools: `whois`, `readprop`, `writeprop`.

## Phase 2 (Complete)

- Typed remote response handling (`Error`, `Reject`, `Abort`) surfaced via client errors.
- Segmented ComplexAck reassembly in client read paths.
- Forwarded-NPDU decode path.
- Foreign-device mode in CLI (`--bbmd`, `--foreign-ttl`).

## Phase 3 (Complete)

- Outbound segmented confirmed-request transmit (configurable window flow, default 1).
- SegmentAck handling for segmented request exchange.
- Dynamic request encode growth for larger payloads.
- BBMD/FDR operations in datalink:
  - Register Foreign Device (blocking and no-wait variants)
  - Read Broadcast Distribution Table (BDT)
  - Write Broadcast Distribution Table (BDT)
  - Read Foreign Device Table (FDT)
  - Delete Foreign Device Table entry
- BBMD/FDR operations exposed at client layer (BACnet/IP transport client specialization):
  - `read_broadcast_distribution_table`
  - `write_broadcast_distribution_table`
  - `read_foreign_device_table`
  - `delete_foreign_device_table_entry`
- BBMD/FDR CLI operations:
  - `bacnet-read-bdt`
  - `bacnet-write-bdt`
  - `bacnet-read-fdt`
  - `bacnet-delete-fdt`
- Auto-renew helper for foreign registration:
  - `BacnetClient::start_foreign_device_renewal(...)`
- BBMD command serialization:
  - BBMD command/response operations are mutex-serialized in transport to prevent cross-talk under concurrent admin traffic

## Verification Matrix (Passing)

1. `cargo test --workspace`
2. `cargo test -p rustbac-core --no-default-features`
3. `cargo test -p rustbac-core --no-default-features --features alloc`
4. `cargo +1.75.0 check --workspace`
5. `cargo clippy --workspace --all-targets -- -D warnings`

## Phase 4 (Complete In-Repo)

- COV:
  - `SubscribeCOV` and `SubscribeCOVProperty`
  - confirmed/unconfirmed COV notification receive + confirmed SimpleAck
  - CLI: `bacnet-subcov`
- ReadRange:
  - by-position, by-sequence, by-time request/decode support
  - CLI: `bacnet-readrange`
- Discovery:
  - `WhoHas` encode + `IHave` decode
  - CLI: `bacnet-whohas`
- Device management:
  - `DeviceCommunicationControl`, `ReinitializeDevice`
  - CLI: `bacnet-dcc`, `bacnet-reinit`
- Time sync:
  - `TimeSynchronization`, `UTCTimeSynchronization`
  - CLI: `bacnet-timesync`
- File services:
  - `AtomicReadFile`/`AtomicWriteFile` (stream + record)
  - CLI: `bacnet-readfile`, `bacnet-writefile`
- Event and alarm:
  - `AcknowledgeAlarm`, `GetAlarmSummary`, `GetEnrollmentSummary`, `GetEventInformation`
  - confirmed/unconfirmed event-notification receive + confirmed SimpleAck
  - CLI: `bacnet-ackalarm`, `bacnet-alarmsummary`, `bacnet-enrollsummary`, `bacnet-eventinfo`, `bacnet-eventnotify`
- Object/list management:
  - `CreateObject`, `DeleteObject`, `AddListElement`, `RemoveListElement`
  - CLI: `bacnet-createobj`, `bacnet-deleteobj`, `bacnet-addlist`, `bacnet-removelist`
- Segmentation resiliency:
  - duplicate segmented ComplexAck tolerance
  - adaptive outbound request window/backoff behavior
  - bounded retransmit on timeout/negative SegmentAck
  - invalid-frame-tolerant receive loops
- BBMD/FDR resiliency:
  - in-process serialized BBMD admin command channel
- BACnet/SC:
  - dedicated crate (`rustbac-bacnet-sc`) with ws/wss transport backend wiring
  - `BacnetClient::new_sc(...)` constructor added
- Coverage assets:
  - BIBBs map (`docs/BIBBS.md`)
  - interop matrix + scripted run output (`docs/INTEROP_MATRIX.md`, `scripts/run_interop_matrix.sh`, `docs/INTEROP_RESULTS.md`)
  - golden packet fixtures + golden corpus loader (`crates/rustbac-core/tests/golden_packets.rs`, `crates/rustbac-core/tests/golden_corpus.rs`, `fixtures/golden/`)

## Remaining External Validation (Ops/Lab)

- Execute the interop matrix against real multi-vendor simulators/hardware and record results.
- Expand `fixtures/golden/` with additional real capture-derived packet corpus data.
- Run extended soak tests for segmentation and BBMD admin operations under sustained lossy/jittery network conditions.
