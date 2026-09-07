#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION_NAME="${SOW_APPLE_VERSION_NAME:-$(tr -d '[:space:]' < "$ROOT/.version")}"
BUILD_NUMBER="${SOW_APPLE_BUILD_NUMBER:-$(git -C "$ROOT" rev-list --count HEAD)}"
IOS_RUN_ROOT="${SOW_IOS_RUN_ROOT:-$ROOT/dist/ios/runs/$VERSION_NAME-$BUILD_NUMBER}"
MACOS_RUN_ROOT="${SOW_MACOS_RUN_ROOT:-$ROOT/dist/macos-testflight/$VERSION_NAME-$BUILD_NUMBER}"
ASC_API_KEY="${SOW_ASC_API_KEY:-}"
ASC_API_ISSUER="${SOW_ASC_API_ISSUER:-}"
ASC_P8_PATH="${SOW_ASC_P8_PATH:-}"
ASC_APP_ID="${SOW_ASC_APP_ID:-}"

die() {
    echo "ERROR: $*" >&2
    exit 1
}

usage() {
    cat >&2 <<'EOF'
usage: scripts/apple-testflight.sh prepare|upload|status

prepare  Build and validate iOS and macOS artifacts without uploading.
upload   Prepare both platforms, then upload both validated artifacts.
status   Read the real App Store Connect build-processing state.
EOF
    exit 2
}

require_asc_credentials() {
    [[ -n "$ASC_API_KEY" && -n "$ASC_API_ISSUER" && -n "$ASC_P8_PATH" ]] \
        || die "App Store Connect status/upload requires SOW_ASC_API_KEY, SOW_ASC_API_ISSUER, and SOW_ASC_P8_PATH"
    [[ -f "$ASC_P8_PATH" ]] || die "App Store Connect private key not found: $ASC_P8_PATH"
    command -v jq >/dev/null || die "jq is required for App Store Connect status"
    command -v curl >/dev/null || die "curl is required for App Store Connect status"
    command -v xcrun >/dev/null || die "xcrun is required for App Store Connect status"
    command -v column >/dev/null || die "column is required for App Store Connect status"
}

base64url() {
    base64 | tr -d '\n' | tr '+/' '-_' | tr -d '='
}

asc_token() {
    local issued_at expiration header payload signing_input signature
    issued_at="$(date +%s)"
    expiration=$((issued_at + 1200))
    header="$(printf '{"'"'alg'"'":"'"'ES256'"'","'"'kid'"'":"'"'%s'"'","'"'typ'"'":"'"'JWT'"'}' "$ASC_API_KEY" | base64url)"
    payload="$(printf '{"'"'iss'"'":"'"'%s'"'","'"'iat'"'":%s,"'"'exp'"'":%s,"'"'aud'"'":"'"'appstoreconnect-v1'"'"}' "$ASC_API_ISSUER" "$issued_at" "$expiration" | base64url)"
    signing_input="$header.$payload"
    signature="$(printf '%s' "$signing_input" | openssl dgst -sha256 -sign "$ASC_P8_PATH" | base64url)"
    printf '%s.%s' "$signing_input" "$signature"
}

asc_token_official() {
    local output token
    output="$(xcrun altool \
        --generate-jwt \
        --api-key "$ASC_API_KEY" \
        --api-issuer "$ASC_API_ISSUER" \
        --p8-file-path "$ASC_P8_PATH" 2>&1)" \
        || die "xcrun altool could not generate the App Store Connect JWT"
    token="$(printf '%s\n' "$output" | awk '/^eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/ { print; exit }')"
    [[ -n "$token" ]] || die "xcrun altool returned no App Store Connect JWT"
    printf '%s' "$token"
}

asc_get() {
    local token
    token="$(asc_token_official)"
    curl -fsS \
        -H "Authorization: Bearer $token" \
        -H 'Accept: application/json' \
        "$@"
}

resolve_app_id() {
    [[ -n "$ASC_APP_ID" ]] && return
    local response
    response="$(asc_get --get \
        --data-urlencode 'filter[bundleId]=games.shadowsofwar.app' \
        'https://api.appstoreconnect.apple.com/v1/apps')"
    ASC_APP_ID="$(jq -r '.data[0].id // empty' <<<"$response")"
    [[ -n "$ASC_APP_ID" ]] || die "App Store Connect has no app for bundle ID games.shadowsofwar.app"
}

prepare() {
    echo "==> Prepare iOS"
    SOW_IOS_RUN_ROOT="$IOS_RUN_ROOT" "$ROOT/scripts/ios-testflight.sh"
    echo "==> Prepare macOS"
    SOW_MACOS_RUN_ROOT="$MACOS_RUN_ROOT" "$ROOT/scripts/macos-testflight.sh"
    echo "PASS: iOS and macOS artifacts passed local distribution gates"
}

upload() {
    require_asc_credentials
    prepare
    local ipa pkg
    ipa="$IOS_RUN_ROOT/export/ShadowsOfWar.ipa"
    pkg="$MACOS_RUN_ROOT/export/ShadowsOfWar.pkg"
    [[ -s "$ipa" ]] || die "validated iOS IPA is missing: $ipa"
    [[ -s "$pkg" ]] || die "validated macOS package is missing: $pkg"
    echo "==> Upload iOS"
    xcrun altool \
        --upload-package "$ipa" \
        --api-key "$ASC_API_KEY" \
        --api-issuer "$ASC_API_ISSUER" \
        --p8-file-path "$ASC_P8_PATH"
    echo "==> Upload macOS"
    xcrun altool \
        --upload-package "$pkg" \
        --api-key "$ASC_API_KEY" \
        --api-issuer "$ASC_API_ISSUER" \
        --p8-file-path "$ASC_P8_PATH"
    echo "PASS: both uploads were accepted by App Store Connect; processing still requires status verification"
}

status() {
    require_asc_credentials
    resolve_app_id
    local response build_id version processing_state audience encryption uploaded pre_release platform beta_detail internal_state external_state
    response="$(asc_get \
        --get \
        --data-urlencode 'fields[builds]=version,uploadedDate,processingState,buildAudienceType,usesNonExemptEncryption' \
        --data-urlencode 'limit=200' \
        "https://api.appstoreconnect.apple.com/v1/apps/$ASC_APP_ID/builds")"
    if ! jq -e '.data | length > 0' >/dev/null <<<"$response"; then
        echo "App Store Connect has not listed a processed or processing build yet for app $ASC_APP_ID"
        return 0
    fi
    echo "App Store Connect builds for $ASC_APP_ID"
    printf 'PLATFORM\tVERSION\tPROCESSING\tINTERNAL_BETA\tEXTERNAL_BETA\tNON_EXEMPT_ENCRYPTION\tUPLOADED\n'
    while IFS=$'\t' read -r build_id version processing_state audience encryption uploaded; do
        pre_release="$(asc_get "https://api.appstoreconnect.apple.com/v1/builds/$build_id/preReleaseVersion")"
        platform="$(jq -r '.data.attributes.platform // "UNKNOWN"' <<<"$pre_release")"
        beta_detail="$(asc_get "https://api.appstoreconnect.apple.com/v1/builds/$build_id/buildBetaDetail")"
        internal_state="$(jq -r '.data.attributes.internalBuildState // "-"' <<<"$beta_detail")"
        external_state="$(jq -r '.data.attributes.externalBuildState // "-"' <<<"$beta_detail")"
        printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
            "$platform" "$version" "$processing_state" "$internal_state" "$external_state" "$encryption" "$uploaded"
    done < <(jq -r '
        .data[] |
        [ .id,
          .attributes.version,
          .attributes.processingState,
          (.attributes.buildAudienceType // "-"),
          (if .attributes.usesNonExemptEncryption == null then "-" else (.attributes.usesNonExemptEncryption | tostring) end),
          (.attributes.uploadedDate // "-") ] |
        @tsv
    ' <<<"$response") | column -t -s $'\t'
}

case "${1:-}" in
    prepare) prepare ;;
    upload) upload ;;
    status) status ;;
    *) usage ;;
esac
