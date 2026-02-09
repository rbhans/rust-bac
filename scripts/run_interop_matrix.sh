#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
OUT_FILE="$ROOT_DIR/docs/INTEROP_RESULTS.md"

run_check() {
  local label="$1"
  shift
  echo "Running: $label"
  if "$@"; then
    printf '| %s | pass |\n' "$label" >> "$OUT_FILE"
  else
    printf '| %s | fail |\n' "$label" >> "$OUT_FILE"
    return 1
  fi
}

cat > "$OUT_FILE" <<EOF
# Interop Results

Automated matrix execution date: $(date +%F)

| Scenario | Result |
|---|---|
EOF

run_check \
  "core golden packet fixtures" \
  cargo test -p rustbac-core --test golden_packets -q

run_check \
  "core event notification decode" \
  cargo test -p rustbac-core event_notification -q

run_check \
  "core npdu vendor-id proprietary gating" \
  cargo test -p rustbac-core network_message_vendor_id_only_for_vendor_types -q

run_check \
  "core i-am segmentation enumerated tag" \
  cargo test -p rustbac-core i_am_segmentation_is_enumerated -q

run_check \
  "core atomic-read-file ack stream decode" \
  cargo test -p rustbac-core decode_atomic_read_file_ack_stream -q

run_check \
  "core golden corpus loader" \
  cargo test -p rustbac-core --test golden_corpus -q

run_check \
  "datalink BBMD serialization behavior" \
  cargo test -p rustbac-datalink bbmd_admin_commands_are_serialized -q

run_check \
  "client segmented window adaptation" \
  cargo test -p rustbac-client write_property_multiple_adapts_window_to_peer_ack_window -q

run_check \
  "client segmented retransmit on negative-ack" \
  cargo test -p rustbac-client write_property_multiple_retries_segment_batch_on_negative_ack -q

run_check \
  "client invalid-frame tolerance" \
  cargo test -p rustbac-client read_property_ignores_invalid_frames_until_valid_response -q

run_check \
  "client confirmed event notification ack handling" \
  cargo test -p rustbac-client recv_confirmed_event_notification_sends_simple_ack -q

run_check \
  "bacnet/sc websocket transport loopback" \
  cargo test -p rustbac-bacnet-sc send_and_recv_binary_payload -q

echo
echo "Interop matrix results written to $OUT_FILE"
