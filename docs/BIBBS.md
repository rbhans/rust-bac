# BIBBs Coverage Map (Current)

This map tracks practical BACnet Interoperability Building Block (BIBB) coverage in `rust-bac`.

Status keys:
- `implemented`: available in core/client/CLI and test-covered
- `partial`: foundational pieces exist, but not full profile breadth
- `planned`: not implemented yet

## Data Sharing

| BIBB | Status | Notes |
|---|---|---|
| `DS-RP-B` (ReadProperty-B) | implemented | `read_property` API + CLI `readprop` |
| `DS-WP-B` (WriteProperty-B) | implemented | `write_property` API + CLI `writeprop` |
| `DS-RPM-B` (ReadPropertyMultiple-B) | implemented | `read_property_multiple` API |
| `DS-WPM-B` (WritePropertyMultiple-B) | implemented | `write_property_multiple` API |
| `DS-RP-A` / `DS-WP-A` | partial | request/response paths are strong; full profile assertions per object class are pending interop matrix execution |

## Alarm and Event

| BIBB | Status | Notes |
|---|---|---|
| `AE-ACK-B` | implemented | `AcknowledgeAlarm` |
| `AE-ASUM-B` | implemented | `GetAlarmSummary` |
| `AE-ESUM-B` | implemented | `GetEnrollmentSummary` |
| `AE-EI-B` | implemented | `GetEventInformation` |
| `AE-N-I-B` | implemented | confirmed/unconfirmed event-notification receive paths with confirmed SimpleAck handling |

## Device and Network Management

| BIBB | Status | Notes |
|---|---|---|
| `DM-DDB-B` | implemented | Who-Is / I-Am and Who-Has / I-Have |
| `DM-DCC-B` | implemented | `DeviceCommunicationControl` |
| `DM-RD-B` | implemented | `ReinitializeDevice` |
| `NM-BBMD-B` | implemented | BDT/FDT read/write/delete + CLI tools; serialized command path added |

## File and Trending

| BIBB | Status | Notes |
|---|---|---|
| `TF-RA-B` / `TF-WA-B` | implemented | `AtomicReadFile` / `AtomicWriteFile` stream+record |
| `T-ATR-B` | implemented | `ReadRange` selectors by position/sequence/time |

## Scheduling and Calendaring

| BIBB | Status | Notes |
|---|---|---|
| scheduling/calendar profiles | planned | broader services are the next major expansion target |

## Security / Transport

| BIBB | Status | Notes |
|---|---|---|
| BACnet/IP transport profile | implemented | UDP/BVLC/BIP |
| BACnet/SC transport profile | partial | ws/wss backend wiring is implemented in `rustbac-bacnet-sc`; full BACnet/SC hub profile hardening remains an interop task |
