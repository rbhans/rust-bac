# Interop Matrix

This matrix is the execution guide for multi-vendor validation runs.

## Test Dimensions

| Dimension | Values |
|---|---|
| network scope | same-subnet, BBMD foreign-device, forwarded NPDU |
| payload size | small APDU, segmented request, segmented response |
| service families | discovery, property access, file, COV, alarm/event, object/list mgmt |
| error behavior | error, reject, abort, malformed frame noise |

## Target Matrix

| Target | Role | Coverage focus | Status |
|---|---|---|---|
| local simulator A | virtual device | baseline functional sanity | runbook + live script ready |
| local simulator B | alternate stack | segmentation and error mapping parity | runbook + live script ready |
| hardware device A | field-like controller | BBMD/FDR + file + COV long-run | planned |
| hardware device B | field-like controller | alarm/event + object management behaviors | planned |

## Minimum Pass Criteria

- discovery returns consistent device/object identity across targets
- property read/write matches expected values with typed errors on failures
- segmented transactions complete under induced packet loss and jitter
- BBMD table operations remain correct with concurrent traffic
- no panics under malformed frame injection

## Run Log Template

| Date | Target | Scenario | Result | Notes |
|---|---|---|---|---|
| YYYY-MM-DD | simulator/hardware id | scenario id | pass/fail | details |
