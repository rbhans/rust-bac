#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
OUT_FILE="$ROOT_DIR/docs/INTEROP_RESULTS_LIVE.md"

IP=""
PORT="47808"
BBMD=""
FOREIGN_TTL="60"
DEVICE_INSTANCE="1"
AO_INSTANCE="1"
FILE_INSTANCE="1"
TRENDLOG_INSTANCE="1"
SUBCOV_LISTEN_SECONDS="20"

usage() {
  cat <<EOF
Usage: $0 --ip <target-ip> [options]

Options:
  --ip <ip>                     Target BACnet/IP device IP (required)
  --port <port>                 Target BACnet/IP port (default: 47808)
  --bbmd <ip:port>              Optional BBMD for foreign registration
  --foreign-ttl <seconds>       Foreign device TTL when using --bbmd (default: 60)
  --device-instance <id>        Device object instance for readprop (default: 1)
  --ao-instance <id>            Analog Output instance for writeprop (default: 1)
  --file-instance <id>          File object instance for read/write file (default: 1)
  --trendlog-instance <id>      TrendLog instance for readrange (default: 1)
  --subcov-listen-seconds <n>   SubCOV listen window (default: 20)
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --ip)
      IP="${2:-}"; shift 2 ;;
    --port)
      PORT="${2:-}"; shift 2 ;;
    --bbmd)
      BBMD="${2:-}"; shift 2 ;;
    --foreign-ttl)
      FOREIGN_TTL="${2:-}"; shift 2 ;;
    --device-instance)
      DEVICE_INSTANCE="${2:-}"; shift 2 ;;
    --ao-instance)
      AO_INSTANCE="${2:-}"; shift 2 ;;
    --file-instance)
      FILE_INSTANCE="${2:-}"; shift 2 ;;
    --trendlog-instance)
      TRENDLOG_INSTANCE="${2:-}"; shift 2 ;;
    --subcov-listen-seconds)
      SUBCOV_LISTEN_SECONDS="${2:-}"; shift 2 ;;
    -h|--help)
      usage; exit 0 ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 2 ;;
  esac
done

if [[ -z "$IP" ]]; then
  echo "--ip is required" >&2
  usage
  exit 2
fi

declare -a NET_ARGS=()
if [[ -n "$BBMD" ]]; then
  NET_ARGS+=(--bbmd "$BBMD" --foreign-ttl "$FOREIGN_TTL")
fi

run_case() {
  local label="$1"
  local required="$2"
  shift 2
  echo "Running: $label"
  if "$@"; then
    printf '| %s | %s | pass | |\n' "$TARGET_LABEL" "$label" >> "$OUT_FILE"
  else
    if [[ "$required" == "required" ]]; then
      printf '| %s | %s | fail | required check failed |\n' "$TARGET_LABEL" "$label" >> "$OUT_FILE"
      return 1
    fi
    printf '| %s | %s | fail | optional check failed |\n' "$TARGET_LABEL" "$label" >> "$OUT_FILE"
  fi
}

TARGET_LABEL="$IP:$PORT"
if [[ -n "$BBMD" ]]; then
  TARGET_LABEL="$TARGET_LABEL via $BBMD"
fi

if [[ ! -f "$OUT_FILE" ]]; then
  cat > "$OUT_FILE" <<EOF
# Live Interop Results

Execution date: $(date +%F)

| Target | Scenario | Result | Notes |
|---|---|---|---|
EOF
fi

pushd "$ROOT_DIR" >/dev/null

run_case "whois discovery" required \
  cargo run -q -p rustbac-tools --bin whois -- \
  --timeout-secs 5 ${NET_ARGS[@]+"${NET_ARGS[@]}"}

run_case "readprop object-name" required \
  cargo run -q -p rustbac-tools --bin readprop -- \
  --ip "$IP" --port "$PORT" --instance "$DEVICE_INSTANCE" ${NET_ARGS[@]+"${NET_ARGS[@]}"}

run_case "writeprop present-value 41.0" required \
  cargo run -q -p rustbac-tools --bin writeprop -- \
  --ip "$IP" --port "$PORT" --instance "$AO_INSTANCE" --value 41 ${NET_ARGS[@]+"${NET_ARGS[@]}"}

run_case "writeprop present-value 42.0" required \
  cargo run -q -p rustbac-tools --bin writeprop -- \
  --ip "$IP" --port "$PORT" --instance "$AO_INSTANCE" --value 42 ${NET_ARGS[@]+"${NET_ARGS[@]}"}

run_case "readrange by-position" optional \
  cargo run -q -p rustbac-tools --bin readrange -- \
  --ip "$IP" --port "$PORT" --object-type trend-log --instance "$TRENDLOG_INSTANCE" \
  --mode position --start-index 1 --count 2 ${NET_ARGS[@]+"${NET_ARGS[@]}"}

run_case "readfile stream" optional \
  cargo run -q -p rustbac-tools --bin readfile -- \
  --ip "$IP" --port "$PORT" --instance "$FILE_INSTANCE" --mode stream --start 0 --count 16 ${NET_ARGS[@]+"${NET_ARGS[@]}"}

run_case "writefile stream" optional \
  cargo run -q -p rustbac-tools --bin writefile -- \
  --ip "$IP" --port "$PORT" --instance "$FILE_INSTANCE" --mode stream --start 0 --data-hex 01020304 ${NET_ARGS[@]+"${NET_ARGS[@]}"}

run_case "subcov listen window" optional \
  cargo run -q -p rustbac-tools --bin subcov -- \
  --ip "$IP" --port "$PORT" --object-type analog-output --instance "$AO_INSTANCE" \
  --process-id 1 --lifetime-seconds 60 --listen-seconds "$SUBCOV_LISTEN_SECONDS" ${NET_ARGS[@]+"${NET_ARGS[@]}"}

popd >/dev/null

echo
echo "Live interop results written to $OUT_FILE"
