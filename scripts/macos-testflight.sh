#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT="$ROOT/sow-dist/deploy/macos/sow_macos.xcodeproj"
VERSION_NAME="${SOW_MACOS_VERSION_NAME:-$(tr -d '[:space:]' < "$ROOT/.version")}"
BUILD_NUMBER="${SOW_MACOS_BUILD_NUMBER:-$(git -C "$ROOT" rev-list --count HEAD)}"
MIN_BUILD_NUMBER="${SOW_MACOS_MIN_BUILD_NUMBER:-408}"
RUN_ROOT="${SOW_MACOS_RUN_ROOT:-$ROOT/dist/macos-testflight/$VERSION_NAME-$BUILD_NUMBER}"
ARCHIVE="${SOW_MACOS_ARCHIVE_PATH:-$RUN_ROOT/ShadowsOfWar.xcarchive}"
EXPORT_DIR="${SOW_MACOS_EXPORT_DIR:-$RUN_ROOT/export}"
DERIVED_DATA_PATH="${SOW_MACOS_DERIVED_DATA_PATH:-$RUN_ROOT/DerivedData}"
TEAM_ID="${SOW_MACOS_TEAM_ID:-HS8F4NGXWN}"
USER_HOME="${HOME:?}"
PROFILE_DIR="${SOW_MACOS_PROFILE_DIR:-$USER_HOME/Library/Developer/Xcode/UserData/Provisioning Profiles}"
MACOS_PROFILE_UUID="${SOW_MACOS_PROVISIONING_PROFILE_UUID:-}"
ASC_API_KEY="${SOW_ASC_API_KEY:-}"
ASC_API_ISSUER="${SOW_ASC_API_ISSUER:-}"
ASC_P8_PATH="${SOW_ASC_P8_PATH:-}"
SIGNING_IDENTITY="${SOW_MACOS_CODE_SIGN_IDENTITY:-}"
INSTALLER_SIGNING_IDENTITY="${SOW_MACOS_INSTALLER_SIGNING_IDENTITY:-}"
BUILD_SETTINGS=""
PACKAGE_STAGE=""
PROFILE_PLIST=""
PROFILE_CERTIFICATE=""

cleanup() {
    [[ -z "$BUILD_SETTINGS" ]] || rm -f "$BUILD_SETTINGS"
    [[ -z "$PACKAGE_STAGE" ]] || rm -rf "$PACKAGE_STAGE"
    [[ -z "$PROFILE_PLIST" ]] || rm -f "$PROFILE_PLIST"
    [[ -z "$PROFILE_CERTIFICATE" ]] || rm -f "$PROFILE_CERTIFICATE"
}
trap cleanup EXIT

die() {
    echo "ERROR: $*" >&2
    exit 1
}

[[ "${1:-}" == "" || "${1:-}" == "--upload" ]] || die "usage: scripts/macos-testflight.sh [--upload]"
[[ "$VERSION_NAME" =~ ^[0-9]+(\.[0-9]+){1,2}$ ]] || die "invalid macOS version name: $VERSION_NAME"
[[ "$BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]] || die "invalid macOS build number: $BUILD_NUMBER"
[[ "$MIN_BUILD_NUMBER" =~ ^[1-9][0-9]*$ ]] || die "invalid minimum macOS build number: $MIN_BUILD_NUMBER"
(( BUILD_NUMBER >= MIN_BUILD_NUMBER )) \
    || die "macOS build $BUILD_NUMBER is not newer than the known failed build floor $MIN_BUILD_NUMBER"

command -v xcodebuild >/dev/null || die "xcodebuild is required"
command -v xcrun >/dev/null || die "xcrun is required"
command -v cargo >/dev/null || die "cargo is required"
command -v productbuild >/dev/null || die "productbuild is required"
command -v pkgutil >/dev/null || die "pkgutil is required"
security find-identity -v \
    | grep -Eq '"Apple Distribution:' \
    || die "no Apple Distribution identity found"
security find-identity -v \
    | grep -Eq '"3rd Party Mac Developer Installer:' \
    || die "no Mac App Store installer identity found"

if [[ -z "$MACOS_PROFILE_UUID" ]]; then
    PROFILE_PLIST="$(mktemp "${TMPDIR:-/tmp}/ShadowsOfWar-mac-profile.XXXXXX.plist")"
    selected_profile=""
    selected_creation=""
    for profile in "$PROFILE_DIR"/*.provisionprofile "$PROFILE_DIR"/*.mobileprovision; do
        [[ -f "$profile" ]] || continue
        security cms -D -i "$profile" > "$PROFILE_PLIST" 2>/dev/null || continue
        profile_platform="$(plutil -extract Platform.0 raw -o - "$PROFILE_PLIST" 2>/dev/null || true)"
        profile_app_id="$(plutil -extract Entitlements.application-identifier raw -o - "$PROFILE_PLIST" 2>/dev/null || true)"
        if [[ -z "$profile_app_id" ]]; then
            profile_app_id="$(/usr/libexec/PlistBuddy -c 'Print :Entitlements:com.apple.application-identifier' "$PROFILE_PLIST" 2>/dev/null || true)"
        fi
        profile_get_task_allow="$(plutil -extract Entitlements.get-task-allow raw -o - "$PROFILE_PLIST" 2>/dev/null || true)"
        [[ -n "$profile_get_task_allow" ]] || profile_get_task_allow="false"
        if [[ "$profile_platform" == "OSX" || "$profile_platform" == "macOS" ]] \
            && [[ "$profile_app_id" == "$TEAM_ID.games.shadowsofwar.app" \
            && "$profile_get_task_allow" != "true" ]]; then
            profile_creation="$(plutil -extract CreationDate raw -o - "$PROFILE_PLIST" 2>/dev/null || true)"
            if [[ -z "$selected_profile" || "$profile_creation" > "$selected_creation" ]]; then
                selected_profile="$profile"
                selected_creation="$profile_creation"
            fi
        fi
    done
    if [[ -n "$selected_profile" ]]; then
        security cms -D -i "$selected_profile" > "$PROFILE_PLIST" 2>/dev/null
        MACOS_PROFILE_UUID="$(plutil -extract UUID raw -o - "$PROFILE_PLIST" 2>/dev/null || true)"
    fi
fi
[[ -n "$MACOS_PROFILE_UUID" ]] \
    || die "no Mac App Store provisioning profile found for games.shadowsofwar.app; sign in to Xcode or set SOW_MACOS_PROVISIONING_PROFILE_UUID"

if [[ -z "$PROFILE_PLIST" || ! -s "$PROFILE_PLIST" \
    || "$(plutil -extract UUID raw -o - "$PROFILE_PLIST" 2>/dev/null || true)" != "$MACOS_PROFILE_UUID" ]]; then
    PROFILE_PLIST="$(mktemp "${TMPDIR:-/tmp}/ShadowsOfWar-mac-profile.XXXXXX.plist")"
    profile_loaded=false
    for profile in "$PROFILE_DIR"/*.provisionprofile "$PROFILE_DIR"/*.mobileprovision; do
        [[ -f "$profile" ]] || continue
        security cms -D -i "$profile" > "$PROFILE_PLIST" 2>/dev/null || continue
        profile_uuid="$(plutil -extract UUID raw -o - "$PROFILE_PLIST" 2>/dev/null || true)"
        if [[ "$profile_uuid" == "$MACOS_PROFILE_UUID" ]]; then
            profile_loaded=true
            break
        fi
    done
    [[ "$profile_loaded" == true ]] \
        || die "Mac App Store profile $MACOS_PROFILE_UUID is not installed in $PROFILE_DIR"
fi

if [[ -z "$SIGNING_IDENTITY" ]]; then
    PROFILE_CERTIFICATE="$(mktemp "${TMPDIR:-/tmp}/ShadowsOfWar-mac-profile-cert.XXXXXX.der")"
    certificate_xml="$(plutil -extract DeveloperCertificates.0 xml1 -o - "$PROFILE_PLIST" 2>/dev/null || true)"
    certificate_b64="$(printf '%s\n' "$certificate_xml" \
        | sed -n '/<data>/,/<\/data>/p' | sed '1d;$d' | tr -d '[:space:]')"
    [[ -n "$certificate_b64" ]] \
        || die "Mac App Store profile does not contain a distribution certificate"
    printf '%s' "$certificate_b64" | base64 -D > "$PROFILE_CERTIFICATE"
    SIGNING_IDENTITY="$(openssl x509 -inform DER -in "$PROFILE_CERTIFICATE" \
        -fingerprint -sha1 -noout | sed 's/.*=//;s/://g')"
fi
[[ -n "$SIGNING_IDENTITY" ]] || die "could not resolve the Mac distribution signing identity"
security find-identity -v -p codesigning \
    | grep -Fq "$SIGNING_IDENTITY" \
    || die "the Mac App Store profile certificate $SIGNING_IDENTITY is not installed in the keychain"

if [[ "${1:-}" == "--upload" ]]; then
    [[ -n "$ASC_API_KEY" && -n "$ASC_API_ISSUER" && -n "$ASC_P8_PATH" ]] \
        || die "--upload requires SOW_ASC_API_KEY, SOW_ASC_API_ISSUER, and SOW_ASC_P8_PATH"
    [[ -f "$ASC_P8_PATH" ]] || die "App Store Connect private key not found: $ASC_P8_PATH"
fi

mkdir -p "$(dirname "$ARCHIVE")" "$EXPORT_DIR" "$DERIVED_DATA_PATH"
rm -rf "$ARCHIVE" "$EXPORT_DIR"

BUILD_SETTINGS="$(mktemp "${TMPDIR:-/tmp}/ShadowsOfWar-mac-build-settings.XXXXXX")"
xcodebuild \
    -project "$PROJECT" \
    -scheme ShadowsOfWar \
    -configuration Release \
    -destination "generic/platform=macOS" \
    -showBuildSettings >"$BUILD_SETTINGS"
grep -Eq '^    ARCHS = arm64$' "$BUILD_SETTINGS" \
    || die "macOS distribution target is not arm64"
grep -Eq '^    CODE_SIGN_STYLE = (Automatic|Manual)$' "$BUILD_SETTINGS" \
    || die "macOS distribution target has an unsupported signing mode"

echo "==> Archive macOS"
echo "==> version=$VERSION_NAME build=$BUILD_NUMBER team=$TEAM_ID architecture=arm64"
xcodebuild \
    -project "$PROJECT" \
    -scheme ShadowsOfWar \
    -configuration Release \
    -destination "generic/platform=macOS" \
    -derivedDataPath "$DERIVED_DATA_PATH" \
    -archivePath "$ARCHIVE" \
    archive \
    -allowProvisioningUpdates \
    CODE_SIGN_STYLE=Manual \
    CODE_SIGN_IDENTITY="$SIGNING_IDENTITY" \
    DEVELOPMENT_TEAM="$TEAM_ID" \
    PROVISIONING_PROFILE_SPECIFIER="$MACOS_PROFILE_UUID" \
    MARKETING_VERSION="$VERSION_NAME" \
    CURRENT_PROJECT_VERSION="$BUILD_NUMBER"

echo "==> Package macOS App Store artifact"
APP_PATH="$ARCHIVE/Products/Applications/ShadowsOfWar.app"
[[ -d "$APP_PATH" ]] || die "archive does not contain a macOS app bundle"
if [[ -z "$INSTALLER_SIGNING_IDENTITY" ]]; then
    INSTALLER_SIGNING_IDENTITY="$(security find-identity -v -p basic \
        | awk '/3rd Party Mac Developer Installer:/{hash=$2} END{print hash}')"
fi
[[ -n "$INSTALLER_SIGNING_IDENTITY" ]] \
    || die "could not resolve the Mac App Store installer signing identity"
security find-identity -v -p basic \
    | grep -Fq "$INSTALLER_SIGNING_IDENTITY" \
    || die "the Mac App Store installer signing identity $INSTALLER_SIGNING_IDENTITY is not installed in the keychain"
mkdir -p "$EXPORT_DIR"
PACKAGE="$EXPORT_DIR/ShadowsOfWar.pkg"
productbuild \
    --sign "$INSTALLER_SIGNING_IDENTITY" \
    --component "$APP_PATH" /Applications "$PACKAGE"

PACKAGE_STAGE="$(mktemp -d "${TMPDIR:-/tmp}/ShadowsOfWar-mac-pkg.XXXXXX")"
pkgutil --expand-full "$PACKAGE" "$PACKAGE_STAGE/expanded"
APP_PATH="$(find "$PACKAGE_STAGE/expanded" -type d -name '*.app' -print -quit)"
[[ -n "$APP_PATH" ]] || die "macOS package does not contain an app bundle"

codesign --verify --deep --strict "$APP_PATH"
APP_INFO="$APP_PATH/Contents/Info.plist"
APP_EXECUTABLE="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$APP_INFO")"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP_INFO")" == "games.shadowsofwar.app" ]] \
    || die "packaged macOS app has the wrong bundle identifier"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP_INFO")" == "$VERSION_NAME" ]] \
    || die "packaged macOS app has the wrong marketing version"
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$APP_INFO")" == "$BUILD_NUMBER" ]] \
    || die "packaged macOS app has the wrong build number"
[[ "$(lipo -archs "$APP_PATH/Contents/MacOS/$APP_EXECUTABLE")" == *arm64* ]] \
    || die "packaged macOS app is missing arm64"
[[ -f "$APP_PATH/Contents/embedded.provisionprofile" ]] \
    || die "packaged macOS app is missing its distribution provisioning profile"

SIGNING_DETAILS="$(codesign -dvvv "$APP_PATH" 2>&1 || true)"
grep -Eq '^Authority=(Apple Distribution|3rd Party Mac Developer Application):' <<<"$SIGNING_DETAILS" \
    || die "packaged macOS app is not signed for App Store distribution"
if grep -Eq '^Authority=(Apple Development|Developer ID Application):' <<<"$SIGNING_DETAILS"; then
    die "packaged macOS app has a development or Developer ID signature"
fi
grep -q '^TeamIdentifier=HS8F4NGXWN$' <<<"$SIGNING_DETAILS" \
    || die "packaged macOS app has the wrong signing team"
pkgutil --check-signature "$PACKAGE" \
    | grep -Eq '3rd Party Mac Developer Installer|Apple Distribution' \
    || die "macOS installer package is not signed for App Store distribution"

echo "PASS: signed macOS arm64 package is ready for App Store Connect"

if [[ "${1:-}" == "--upload" ]]; then
    echo "==> Upload macOS package to App Store Connect"
    xcrun altool \
        --upload-package "$PACKAGE" \
        --api-key "$ASC_API_KEY" \
        --api-issuer "$ASC_API_ISSUER" \
        --p8-file-path "$ASC_P8_PATH"
    echo "PASS: macOS upload accepted by App Store Connect; processing status still requires verification"
else
    echo "Package ready: $PACKAGE"
    echo "PASS: macOS archive/export completed; nothing uploaded"
fi
