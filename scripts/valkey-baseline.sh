#!/usr/bin/env bash
# Valkey/Redis saturation baseline — Step 0 of the DB bottleneck plan.
#
# Run this BEFORE and AFTER the connection/SCAN changes and diff the two
# captures. "Feels faster" is not a result; these numbers are.
#
# Usage:
#   scripts/valkey-baseline.sh before
#   scripts/valkey-baseline.sh after
#   scripts/valkey-baseline.sh diff before after
#
# Env:
#   SOW_REDIS_URL   default redis://127.0.0.1/   (same var the servers use)

set -euo pipefail

CLI="$(command -v valkey-cli || command -v redis-cli || true)"
URL="${SOW_REDIS_URL:-redis://127.0.0.1/}"
OUT_DIR="${SOW_BASELINE_DIR:-target/valkey-baseline}"

if [[ -z "$CLI" ]]; then
    echo "error: neither valkey-cli nor redis-cli found in PATH" >&2
    exit 1
fi

# valkey-cli/redis-cli take host/port, not a redis:// URL.
HOST="$(printf '%s' "$URL" | sed -E 's#^[a-z]+://(.*@)?([^:/]+).*#\2#')"
PORT="$(printf '%s' "$URL" | sed -nE 's#^[a-z]+://(.*@)?[^:/]+:([0-9]+).*#\2#p')"
PORT="${PORT:-6379}"

run() { "$CLI" -h "$HOST" -p "$PORT" "$@"; }

if ! run ping >/dev/null 2>&1; then
    echo "error: no Valkey/Redis answering at ${HOST}:${PORT} (from SOW_REDIS_URL=${URL})" >&2
    echo "       start the server, or point SOW_REDIS_URL at the real instance." >&2
    exit 2
fi

capture() {
    local label="$1"
    local dir="${OUT_DIR}/${label}"
    mkdir -p "$dir"

    # Server identity + config snapshot.
    run info server        > "${dir}/info_server.txt"
    run info clients       > "${dir}/info_clients.txt"
    run info stats         > "${dir}/info_stats.txt"
    run info commandstats  > "${dir}/info_commandstats.txt"
    run info memory        > "${dir}/info_memory.txt"
    run config get maxmemory maxmemory-policy appendonly save > "${dir}/config.txt" 2>/dev/null || true

    # The expensive-command paper trail: SCAN loops show up here.
    run slowlog get 32     > "${dir}/slowlog.txt" 2>/dev/null || true
    run slowlog len        > "${dir}/slowlog_len.txt" 2>/dev/null || true

    # Keyspace shape: how much of it a full sweep would have to walk.
    run dbsize             > "${dir}/dbsize.txt"
    run info keyspace      > "${dir}/info_keyspace.txt"

    echo "captured '${label}' -> ${dir}"
}

summary() {
    local dir="$1"
    echo
    echo "--- ${dir} ---"
    grep -E '^(total_connections_received|total_commands_processed|instantaneous_ops_per_sec|rejected_connections|expired_keys|keyspace_hits|keyspace_misses|total_net_input_bytes):' \
        "${dir}/info_stats.txt" 2>/dev/null || true
    grep -E '^used_memory_human:|^maxmemory_human:' "${dir}/info_memory.txt" 2>/dev/null || true
    echo "dbsize: $(cat "${dir}/dbsize.txt" 2>/dev/null)"
    echo "slowlog len: $(cat "${dir}/slowlog_len.txt" 2>/dev/null)"
    echo "top commands by calls:"
    grep -E '^cmdstat_' "${dir}/info_commandstats.txt" 2>/dev/null \
        | sed -E 's/^cmdstat_([^:]+):calls=([0-9]+).*/\2 \1/' \
        | sort -rn | head -12 || true
    echo "SCAN calls: $(grep -oE '^cmdstat_scan:calls=[0-9]+' "${dir}/info_commandstats.txt" 2>/dev/null || echo 'cmdstat_scan:calls=0')"
}

case "${1:-}" in
    before|after)
        capture "$1"
        summary "${OUT_DIR}/$1"
        ;;
    diff)
        a="${OUT_DIR}/${2:?usage: diff <before> <after>}"
        b="${OUT_DIR}/${3:?usage: diff <before> <after>}"
        summary "$a"
        summary "$b"
        echo
        echo "--- delta (calls) ---"
        join <(grep -E '^cmdstat_' "$a/info_commandstats.txt" | sed -E 's/^cmdstat_([^:]+):calls=([0-9]+).*/\1 \2/' | sort) \
             <(grep -E '^cmdstat_' "$b/info_commandstats.txt" | sed -E 's/^cmdstat_([^:]+):calls=([0-9]+).*/\1 \2/' | sort) \
            | awk '{printf "%-16s %10d -> %10d\n", $1, $2, $3}'
        ;;
    *)
        echo "usage: $0 {before|after|diff <before> <after>}" >&2
        exit 64
        ;;
esac
