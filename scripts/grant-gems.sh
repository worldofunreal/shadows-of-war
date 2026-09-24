#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -f "$ROOT/sow-dist/.env" ]]; then
    set -a
    source "$ROOT/sow-dist/.env"
    set +a
fi

usage() {
    echo "usage: $0 <account_id> <gems> [request_id]" >&2
    exit 64
}

[[ $# -ge 2 && $# -le 3 ]] || usage
account_id="$1"
gems="$2"
if [[ ! "$account_id" =~ ^([[:xdigit:]]{32}|[[:xdigit:]]{8}-[[:xdigit:]]{4}-[[:xdigit:]]{4}-[[:xdigit:]]{4}-[[:xdigit:]]{12})$ ]]; then
    echo "error: account_id must be a canonical account ID" >&2
    exit 64
fi
if [[ ! "$gems" =~ ^[1-9][0-9]*$ ]]; then
    echo "error: gems must be a positive whole number" >&2
    exit 64
fi

request_id="${3:-$(od -An -N16 -tx1 /dev/urandom | tr -d '[:space:]')}"
if [[ ! "$request_id" =~ ^[[:xdigit:]]{32}$ ]]; then
    echo "error: request_id must contain 32 hexadecimal characters" >&2
    exit 64
fi

: "${SOW_DB_SECRET:?SOW_DB_SECRET is missing from sow-dist/.env}"
if [[ ! "$SOW_DB_SECRET" =~ ^[[:xdigit:]]{64}$ ]]; then
    echo "error: SOW_DB_SECRET must be 64 hexadecimal characters" >&2
    exit 64
fi
control_host="${SOW_CONTROL_HOST:-ionos}"
database_url="${SOW_DB_OPERATOR_URL:-http://127.0.0.1:25585}"
if [[ ! "$control_host" =~ ^[[:alnum:]_.@-]+$ || ! "$database_url" =~ ^http://[[:alnum:]_.:-]+$ ]]; then
    echo "error: invalid database operator connection settings" >&2
    exit 64
fi

curl_config="$(printf 'url = "%s/internal/profile/grant-gems"\nrequest = "POST"\nheader = "Authorization: Bearer %s"\nheader = "Content-Type: application/json"\ndata = "{\\"account_id\\":\\"%s\\",\\"gems\\":%s,\\"request_id\\":\\"%s\\"}"\n' \
    "$database_url" "$SOW_DB_SECRET" "$account_id" "$gems" "$request_id")"
remote_command="curl --config - --silent --show-error --retry 2 --max-time 20 --write-out '\\n%{http_code}'"
if response="$(printf '%s' "$curl_config" | ssh -T "$control_host" "$remote_command")"; then
    http_status="${response##*$'\n'}"
    body="${response%$'\n'*}"
    printf '%s\n' "$body"
    if [[ ! "$http_status" =~ ^2[0-9][0-9]$ ]]; then
        printf 'HTTP %s\n' "$http_status" >&2
        printf 'request_id=%s\n' "$request_id" >&2
        exit 1
    fi
else
    printf 'grant request failed; retry with request_id=%s\n' "$request_id" >&2
    exit 1
fi
