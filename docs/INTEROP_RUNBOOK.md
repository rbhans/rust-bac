# Interop Runbook (Simple)

This is the practical path to complete the next two priorities:

1. Multi-vendor BACnet/IP interop hardening
2. BACnet/SC hardening

Use this as an operator checklist.

## Phase 2: BACnet/IP Interop (Do This First)

### Step 1: Verify baseline quality (local, no devices)

```bash
cargo test --workspace
bash scripts/run_interop_matrix.sh
```

This confirms the stack is stable before field testing.

### Step 2: Run one simulator target

Pick one simulator/device IP and run:

```bash
bash scripts/run_live_interop.sh --ip <TARGET_IP>
```

This executes discovery/property/file/range/COV smoke checks and writes results to:

- `docs/INTEROP_RESULTS_LIVE.md`

### Step 3: Run second simulator/alternate stack

Run the same command against a second simulator/device:

```bash
bash scripts/run_live_interop.sh --ip <TARGET_IP_2>
```

This validates interoperability beyond a single implementation.

### Step 4: Validate BBMD/FDR path

If you have a BBMD available:

```bash
bash scripts/run_live_interop.sh --ip <TARGET_IP> --bbmd <BBMD_IP:47808>
```

### Step 5: Fill matrix status

Update status lines in:

- `docs/INTEROP_MATRIX.md`
- `docs/INTEROP_RESULTS.md`

Mark simulator targets as pass/fail and add notes.

## Phase 3: BACnet/SC Hardening (After Phase 2)

### Step 1: Connectivity and framing

Use a BACnet/SC hub endpoint and validate client connects and exchanges frames:

- `BacnetClient::new_sc(...)`
- ensure ws/wss paths both work for your deployment

### Step 2: Repeat key service checks over BACnet/SC

Re-run discovery/property operations over SC transport and compare behavior with BACnet/IP:

- Who-Is/I-Am
- ReadProperty/WriteProperty
- COV/Event receive paths

### Step 3: Error-path validation

Test disconnect/reconnect and malformed frame handling. Confirm no panics and clear error mapping.

### Step 4: Performance and soak

Run a longer test (30-60 minutes) for notification streams and repeated confirmed requests.

## Recommended simulator setup (if you have no hardware)

Run two different simulators/stacks, not just one. Use one as baseline and one as alternate implementation. The important part is diversity of behavior, not one specific vendor.

## Done criteria

You are in strong release shape when:

- local automated matrix passes
- live interop passes on at least two independent BACnet/IP targets
- BBMD/FDR path is validated if you need cross-subnet
- BACnet/SC path is validated on your target hub
