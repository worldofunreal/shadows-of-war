#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROJECT="$ROOT/sow-dist/deploy/android"
[[ -f "$ROOT/sow-dist/.env" ]] && { set -a; source "$ROOT/sow-dist/.env"; set +a; }

VARIANT="${SOW_ANDROID_TEST_VARIANT:-debug}"
PACKAGE="com.shadowsofwar.debug"
TASK=":app:assembleDebug"
APK="$PROJECT/app/build/outputs/apk/debug/app-debug.apk"
if [[ "$VARIANT" == release ]]; then
    PACKAGE="com.shadowsofwar"
    TASK=":app:assembleRelease"
    APK="$PROJECT/app/build/outputs/apk/release/app-release.apk"
elif [[ "$VARIANT" != debug ]]; then
    echo "SOW_ANDROID_TEST_VARIANT must be release or debug" >&2
    exit 1
fi

ACTIVITY="com.shadowsofwar.TwaLauncherActivity"
OUT="$ROOT/dist/android/local-test"
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="$OUT/logcat-$STAMP.txt"
START="$OUT/start-$STAMP.txt"
TRACE="$OUT/trace-$STAMP.ndjson"
EVIDENCE="$OUT/evidence-$STAMP.txt"
PACKAGE_DUMP="$OUT/package-$STAMP.txt"
VERSION_NAME="${SOW_ANDROID_TEST_VERSION_NAME:-$(tr -d '[:space:]' <"$ROOT/.version")}"
VERSION_CODE="${SOW_ANDROID_TEST_VERSION_CODE:-$(tr -d '[:space:]' <"$ROOT/.android-version-code")}"
SOURCE_SHA="${SOW_ANDROID_TEST_SOURCE_SHA:-$(git -C "$ROOT" rev-parse HEAD)}"
SKIP_BUILD="${SOW_ANDROID_SKIP_BUILD:-0}"
WEB_CACHE_BUST="${SOW_ANDROID_TEST_CACHE_BUST:-local-$STAMP}"
PLAY_TRACK="${SOW_ANDROID_TEST_TRACK:-local}"
TEST_ROTATION="${SOW_ANDROID_TEST_ROTATION:-}"
ROTATION_LOCKED=0
LOGGER_PID=0

die() { echo "ERROR: $*" >&2; exit 1; }
event() { printf '{"event":"%s","epoch_ms":%s}\n' "$1" "$(date +%s%3N)" >>"$TRACE"; }
restore_rotation() {
    if ((ROTATION_LOCKED)); then
        adb shell cmd window set-user-rotation free >/dev/null 2>&1 || true
        ROTATION_LOCKED=0
    fi
}
stop_logger() {
    if ((LOGGER_PID)); then
        kill "$LOGGER_PID" 2>/dev/null || true
        wait "$LOGGER_PID" 2>/dev/null || true
        LOGGER_PID=0
    fi
}
trap 'stop_logger; restore_rotation' EXIT

case "$TEST_ROTATION" in
    ""|portrait|landscape) ;;
    *) die "SOW_ANDROID_TEST_ROTATION must be portrait or landscape" ;;
esac
command -v adb >/dev/null || die "adb is required"
command -v rg >/dev/null || die "rg is required"
mkdir -p "$OUT"
: >"$TRACE"

missing_play_games=()
for key in SOW_PLAY_GAMES_APP_ID SOW_PLAY_GAMES_WEB_CLIENT_ID; do
    [[ -n "${!key:-}" ]] || missing_play_games+=("$key")
done
(( ${#missing_play_games[@]} == 0 )) || die "missing Play Games config: ${missing_play_games[*]}"
PLAY_GAMES_APP_ID_ARG="-PsowPlayGamesAppId=$SOW_PLAY_GAMES_APP_ID"
PLAY_GAMES_CLIENT_ID_ARG="-PsowPlayGamesWebClientId=$SOW_PLAY_GAMES_WEB_CLIENT_ID"
PLAY_GAMES_AUTH_URL_ARG="-PsowPlayGamesAuthUrl=${SOW_PLAY_GAMES_AUTH_URL:-https://shadowsofwar.io/api}"
SOURCE_SHA_ARG="-PsowSourceSha=$SOURCE_SHA"

if [[ "$SKIP_BUILD" != 1 ]]; then
    ( cd "$PROJECT" && ./gradlew --warning-mode fail --no-daemon --no-configuration-cache "$TASK" \
        "-PsowVersionName=$VERSION_NAME" "-PsowVersionCode=$VERSION_CODE" \
        "-PsowWebCacheBust=$WEB_CACHE_BUST" "$SOURCE_SHA_ARG" \
        "$PLAY_GAMES_APP_ID_ARG" "$PLAY_GAMES_CLIENT_ID_ARG" "$PLAY_GAMES_AUTH_URL_ARG" )
fi
[[ -s "$APK" ]] || die "Android test APK missing: $APK"

MANIFEST="$PROJECT/app/build/intermediates/merged_manifests/$VARIANT/process${VARIANT^}Manifest/AndroidManifest.xml"
BUILD_CONFIG="$PROJECT/app/build/generated/source/buildConfig/$VARIANT/com/shadowsofwar/BuildConfig.java"
rg -q "SOW_SOURCE_SHA = \"$SOURCE_SHA\"" "$BUILD_CONFIG" ||
    die "assembled artifact source SHA is not bound to $SOURCE_SHA"
rg -q "VERSION_CODE = $VERSION_CODE;" "$BUILD_CONFIG" ||
    die "assembled artifact versionCode is not bound to $VERSION_CODE"
rg -q "package=\"$PACKAGE\"" "$MANIFEST" ||
    die "assembled artifact package is not bound to $PACKAGE"
rg -q "android:versionCode=\"$VERSION_CODE\"" "$MANIFEST" ||
    die "assembled manifest versionCode is not bound to $VERSION_CODE"
rg -q 'com\.google\.android\.gms\.games\.SUPPRESS_GAME_PROFILE_CREATION' "$MANIFEST" ||
    die "Play Games profile suppression missing"
! rg -q 'SPLASH_IMAGE_DRAWABLE|FILE_PROVIDER_AUTHORITY|sow_splash' "$MANIFEST" ||
    die "native TWA splash is still configured"
rg -q 'android:name="[^"]*\.TwaLauncherActivity"' "$MANIFEST" ||
    die "TWA launcher activity missing from manifest"
rg -q 'android:name="[^"]*\.PurchaseActivity"' "$MANIFEST" ||
    die "native purchase activity missing from manifest"
rg -q 'android:host="restore"' "$MANIFEST" ||
    die "native restore route missing from manifest"
LAUNCHER_SOURCE="$PROJECT/app/src/main/kotlin/com/shadowsofwar/TwaLauncherActivity.kt"
ON_CREATE_SOURCE="$(sed -n '/override fun onCreate/,/^    }$/p' "$LAUNCHER_SOURCE")"
! rg -q '\.signIn\(|PlayGamesSdk|checkAuthentication' <<<"$ON_CREATE_SOURCE" ||
    die "launcher authenticates before the loader"
! rg -q 'checkAuthentication' "$LAUNCHER_SOURCE" ||
    die "legacy launcher authentication route remains"
! rg -q 'auto_signin' "$PROJECT/app/src/main/kotlin/com/shadowsofwar" ||
    die "duplicate automatic Play Games route remains"
rg -q 'PlayGamesSdk\.initialize|loader-ready Play Games silent auth requested' "$LAUNCHER_SOURCE" ||
    die "post-loader Play Games auth path is missing"
if rg --files "$PROJECT/app/src/main/java" 2>/dev/null | rg -q '\.java$'; then
    die "Android application still contains Java source"
fi
rg -q 'playgames_silent_auth|"restore"' \
    "$PROJECT/app/src/main/kotlin/com/shadowsofwar/TwaLauncherActivity.kt" ||
    die "native bridge is missing silent auth or restore"
rg -q 'if \(isAndroidTwa\(\)\).*SOW_requestAndroidPurchase|SOW_requestAndroidPurchase' \
    "$ROOT/sow-web/shell/main_menu.screens.js" ||
    die "Android purchase path is not native"
rg -q 'SOW_requestAndroidRestore|restore_purchases' \
    "$ROOT/sow-web/shell/main_menu.screens.js" ||
    die "Android restore caller is missing"
rg -q 'data-command=.sign_in.|SOW_signInAndroidPlayGames' \
    "$ROOT/sow-web/shell/main_menu.screens.js" "$ROOT/sow-web/shell/sdk/store_portals.js" ||
    die "explicit Play Games sign-in button path is missing"
LOADER_HOOK_LINE="$(rg -n "sow:loader-ready" "$ROOT/sow-web/shell/index.html.template" | head -1 | cut -d: -f1 || true)"
AUTO_AUTH_LINE="$(rg -n "SOW_startAndroidPlayGamesAutoAuth" "$ROOT/sow-web/shell/index.html.template" | head -1 | cut -d: -f1 || true)"
[[ -n "$LOADER_HOOK_LINE" && -n "$AUTO_AUTH_LINE" && $AUTO_AUTH_LINE -gt $LOADER_HOOK_LINE ]] ||
    die "native silent auth is not gated by loader-ready"
PREP_SOURCE="$(sed -n '/window\.SOW_prepareAndroidAuthState/,/^  };$/p' \
    "$ROOT/sow-web/shell/sdk/store_portals.js")"
! rg -q 'fetch|playgames/consume|playgames_silent_auth|\.signIn\(|PlayGamesSdk' <<<"$PREP_SOURCE" ||
    die "pre-WASM Android preparation still starts authentication"
PURCHASE_ANDROID_BLOCK="$(sed -n '/function beginStorePurchase/,/var creds = selfCreds()/p' \
    "$ROOT/sow-web/shell/main_menu.screens.js")"
rg -q 'SOW_requestAndroidPurchase' <<<"$PURCHASE_ANDROID_BLOCK" ||
    die "Android purchase branch does not call the native bridge"
! rg -q 'loadStripeJs|/store/checkout|REVENUECAT|Stripe|stripe' <<<"$PURCHASE_ANDROID_BLOCK" ||
    die "Android purchase branch contains an external checkout"

if ! adb get-state 2>/dev/null | grep -qx device; then
    adb devices -l >&2 || true
    die "no Android device is connected"
fi

POWER="$(adb shell dumpsys power 2>/dev/null | tr -d '\r' || true)"
if grep -Eq 'mWakefulness=(Asleep|Dozing)|Display Power: state=OFF' <<<"$POWER"; then
    adb shell input keyevent KEYCODE_WAKEUP
    sleep 1
    POWER="$(adb shell dumpsys power 2>/dev/null | tr -d '\r' || true)"
fi
grep -Eq 'mWakefulness=(Asleep|Dozing)|Display Power: state=OFF' <<<"$POWER" &&
    die "Android screen could not be awakened"
WINDOW="$(adb shell dumpsys window 2>/dev/null | tr -d '\r' || true)"
grep -q 'mDreamingLockscreen=true' <<<"$WINDOW" && die "Android device is locked"

SERIAL="$(adb get-serialno | tr -d '\r')"
MODEL="$(adb shell getprop ro.product.model | tr -d '\r')"
ANDROID_VERSION="$(adb shell getprop ro.build.version.release | tr -d '\r')"
DIRTY=0
[[ -n "$(git -C "$ROOT" status --porcelain --untracked-files=all)" ]] && DIRTY=1

adb install -r "$APK" >"$OUT/install-$STAMP.txt"
adb shell dumpsys package "$PACKAGE" >"$PACKAGE_DUMP"
INSTALLED_CODE="$(rg -o -m1 'versionCode=[0-9]+' "$PACKAGE_DUMP" | cut -d= -f2 || true)"
[[ "$INSTALLED_CODE" == "$VERSION_CODE" ]] ||
    die "installed versionCode=$INSTALLED_CODE, expected $VERSION_CODE; see $PACKAGE_DUMP"
adb shell am force-stop "$PACKAGE"
adb shell am force-stop com.android.chrome
adb logcat -c
if [[ "$TEST_ROTATION" == portrait ]]; then
    adb shell cmd window set-user-rotation lock 0 >/dev/null
    ROTATION_LOCKED=1
elif [[ "$TEST_ROTATION" == landscape ]]; then
    adb shell cmd window set-user-rotation lock 1 >/dev/null
    ROTATION_LOCKED=1
fi

stdbuf -oL adb logcat -v threadtime -b main -b system -b crash \
    SOW_PGS:I AndroidRuntime:E chromium:I '*:S' >"$LOG" &
LOGGER_PID=$!
event TEST_START
adb shell am start -n "$PACKAGE/$ACTIVITY" >"$START" || die "activity failed to start; see $START"
grep -q '^Starting:' "$START" || die "activity did not start; see $START"
event LAUNCH_REQUESTED

TWA=0
LAUNCHER=0
LOADER=0
AUTH=0
PGS_EARLY=0
deadline=$((SECONDS + 60))
while ((SECONDS < deadline)); do
    WINDOW="$(adb shell dumpsys activity activities 2>/dev/null | tr -d '\r' || true)"
    if rg -q 'GamesResolutionActivity|SignInActivity' <<<"$WINDOW"; then
        ((TWA)) || { event PLAY_GAMES_UI_BEFORE_TWA; PGS_EARLY=1; break; }
        event PLAY_GAMES_UI_WITHOUT_EXPLICIT_TAP
        break
    fi
    if ((!LAUNCHER)) && rg -q 'SOW_PGS.*TWA launcher ready; Play Games waits for loader-ready' "$LOG"; then
        event LAUNCHER_READY
        LAUNCHER=1
    fi
    if ((!TWA)) && rg -q 'mResumedActivity:.*CustomTabActivity' <<<"$WINDOW" &&
        rg -q 'TwaLauncherActivity|com\.google\.androidbrowserhelper\.trusted\.LauncherActivity' <<<"$WINDOW"; then
        event TWA_VISIBLE
        TWA=1
    fi
    if ((!LOADER)) && rg -q 'SOW_PGS.*loader-ready Play Games silent auth requested' "$LOG"; then
        event LOADER_READY
        LOADER=1
    fi
    if ((!AUTH)) && rg -q 'SOW_PGS.*(rendezvous success=true HTTP 2|loader-ready isAuthenticated|Play Games auth result posted status=(ready|unavailable|error))' "$LOG"; then
        event AUTH_OUTCOME
        AUTH=1
    fi
    ((TWA && LOADER && AUTH)) && break
    sleep 1
done

stop_logger
restore_rotation
trap - EXIT
APP_LOG="$OUT/app-$STAMP.txt"
rg 'SOW_PGS' "$LOG" >"$APP_LOG" || true
APK_SHA256="$(sha256sum "$APK" | awk '{print $1}')"
{
    printf 'evidence_version=2\n'
    printf 'SOURCE source_sha=%s worktree_dirty=%s version_name=%s expected_version_code=%s package=%s\n' \
        "$SOURCE_SHA" "$DIRTY" "$VERSION_NAME" "$VERSION_CODE" "$PACKAGE"
    printf 'PLAY_API track=%s\n' "$PLAY_TRACK"
    printf 'DEVICE artifact=%s artifact_sha256=%s installed_package=%s installed_version_code=%s\n' \
        "$APK" "$APK_SHA256" "$PACKAGE" "$INSTALLED_CODE"
    printf 'DEVICE device_serial=%s device_model=%s device_android=%s timestamp=%s\n' \
        "$SERIAL" "$MODEL" "$ANDROID_VERSION" "$(date --iso-8601=seconds)"
    printf 'DEVICE trace=%s logcat=%s\n' "$TRACE" "$LOG"
} >"$EVIDENCE"

if rg -q 'AndroidRuntime: Process: '"$PACKAGE"', PID:' "$LOG"; then
    die "Android startup crash detected; evidence=$EVIDENCE"
fi
((PGS_EARLY == 0)) || die "Play Games UI appeared before TWA; evidence=$EVIDENCE"
((LAUNCHER)) || die "launcher-ready marker missing; evidence=$EVIDENCE"
((TWA)) || die "TWA was not observed as foreground; evidence=$EVIDENCE"
((LOADER)) || die "loader-ready auth request missing; web/native artifacts are not aligned; evidence=$EVIDENCE"
((AUTH)) || die "post-loader Play Games auth outcome missing; evidence=$EVIDENCE"
! rg -q 'PLAY_GAMES_UI_WITHOUT_EXPLICIT_TAP' "$TRACE" ||
    die "interactive Play Games UI appeared without a tap; evidence=$EVIDENCE"
LAUNCHER_LINE="$(rg -n '"event":"LAUNCHER_READY"' "$TRACE" | head -1 | cut -d: -f1 || true)"
TWA_LINE="$(rg -n '"event":"TWA_VISIBLE"' "$TRACE" | head -1 | cut -d: -f1 || true)"
LOADER_LINE="$(rg -n '"event":"LOADER_READY"' "$TRACE" | head -1 | cut -d: -f1 || true)"
AUTH_LINE="$(rg -n '"event":"AUTH_OUTCOME"' "$TRACE" | head -1 | cut -d: -f1 || true)"
[[ -n "$LAUNCHER_LINE" && -n "$TWA_LINE" && -n "$LOADER_LINE" && -n "$AUTH_LINE" ]] ||
    die "runtime event trace is incomplete; evidence=$EVIDENCE"
(( LAUNCHER_LINE < TWA_LINE && TWA_LINE < LOADER_LINE && LOADER_LINE <= AUTH_LINE )) ||
    die "runtime event order is invalid; evidence=$EVIDENCE"
rg -q "artifact source_sha=$SOURCE_SHA version_code=$VERSION_CODE package=$PACKAGE" "$APP_LOG" ||
    die "runtime artifact identity is not bound to source/version; evidence=$EVIDENCE"
! rg -q ' [WE] SOW_PGS :' "$LOG" || die "Play Games bridge emitted warning/error; evidence=$EVIDENCE"

SCREENSHOT="$OUT/screenshot-$STAMP.png"
adb exec-out screencap -p >"$SCREENSHOT"
[[ -s "$SCREENSHOT" ]] || die "Android screenshot is empty"
echo "PASS: TWA visible before post-loader Play Games auth; no interactive Play Games UI without a tap."
echo "Evidence: $EVIDENCE"
echo "Trace: $TRACE"
echo "Screenshot: $SCREENSHOT"
