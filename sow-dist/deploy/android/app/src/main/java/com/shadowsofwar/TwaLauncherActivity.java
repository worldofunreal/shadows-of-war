package com.shadowsofwar;

import android.graphics.Color;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;
import android.view.View;
import android.view.Window;

import androidx.browser.customtabs.CustomTabsCallback;
import androidx.browser.customtabs.CustomTabsSession;

import com.google.androidbrowserhelper.trusted.LauncherActivity;
import com.google.androidbrowserhelper.trusted.TwaLauncher;
import com.google.android.gms.games.GamesSignInClient;
import com.google.android.gms.games.PlayGames;
import com.google.android.gms.games.PlayGamesSdk;

import org.json.JSONObject;
import org.json.JSONArray;

import java.lang.reflect.Field;
import java.util.UUID;

/** Opens the TWA; the web loader starts optional Play Games authentication. */
public final class TwaLauncherActivity extends LauncherActivity {
    private static final String TAG = "SOW_PGS";
    private static final Uri SOW_ORIGIN = Uri.parse("https://shadowsofwar.io");
    private static volatile CustomTabsSession messageSession;
    private GamesSignInClient signInClient;
    private boolean requestInFlight;
    private String rendezvousId;

    @Override
    protected void onCreate(Bundle state) {
        hideSystemBars();
        super.onCreate(state);
        hideSystemBars();
        Log.i(TAG, "TWA launched; starting Play Games in parallel");
        PlayGamesSdk.initialize(getApplicationContext());
        if (!PlayGamesAuth.isAnonymousOptOut(this)) {
            signInClient = PlayGames.getGamesSignInClient(this);
            checkAuthentication();
        } else {
            Log.i(TAG, "automatic Play Games login suppressed by anonymous mode");
        }
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            hideSystemBars();
        }
    }

    @Override
    protected CustomTabsCallback getCustomTabsCallback() {
        return new CustomTabsCallback() {
            @Override
            public void onNavigationEvent(int navigationEvent, Bundle extras) {
                if (navigationEvent != CustomTabsCallback.NAVIGATION_FINISHED) {
                    return;
                }
                CustomTabsSession session = findMessageSession();
                if (session == null) {
                    Log.w(TAG, "TWA session unavailable; purchase bridge disabled");
                    return;
                }
                messageSession = session;
                boolean requested = session.requestPostMessageChannel(
                        SOW_ORIGIN, SOW_ORIGIN, new Bundle());
                Log.i(TAG, "purchase bridge channel requested=" + requested);
            }

            @Override
            public void onMessageChannelReady(Bundle extras) {
                CustomTabsSession session = findMessageSession();
                if (session == null) {
                    return;
                }
                messageSession = session;
                JSONArray products = new JSONArray();
                for (String product : PurchaseActivity.storeProductIds()) products.put(product);
                JSONObject ready = new JSONObject();
                try {
                    ready.put("type", "sow_bridge_ready");
                    ready.put("bridge_version", 1);
                    ready.put("capabilities", new JSONArray().put("purchase"));
                    ready.put("products", products);
                } catch (Exception error) {
                    Log.w(TAG, "could not build purchase bridge capabilities", error);
                    return;
                }
                int result = session.postMessage(ready.toString(), null);
                Log.i(TAG, "purchase bridge ready result=" + result);
            }

            @Override
            public void onPostMessage(String message, Bundle extras) {
                handleBridgeMessage(message);
            }
        };
    }

    private void handleBridgeMessage(String message) {
        try {
            JSONObject request = new JSONObject(message);
            if (!"purchase".equals(request.optString("type"))) {
                return;
            }
            String productId = request.optString("product_id", "");
            String appUserId = request.optString("app_user_id", "");
            String requestId = request.optString("request_id", "");
            if (!PurchaseActivity.isStoreProduct(productId)
                    || !PurchaseActivity.isPurchaseUserId(appUserId)
                    || !requestId.matches("[A-Za-z0-9_-]{1,80}")) {
                Log.w(TAG, "rejected invalid purchase bridge request");
                postPurchaseResult(requestId, "error", productId);
                return;
            }
            Intent intent = new Intent(this, PurchaseActivity.class).setData(
                    Uri.parse("sow://purchase").buildUpon()
                            .appendQueryParameter("product_id", productId)
                            .appendQueryParameter("app_user_id", appUserId)
                            .appendQueryParameter("request_id", requestId)
                            .build());
            startActivity(intent);
        } catch (Exception error) {
            Log.w(TAG, "invalid purchase bridge message", error);
        }
    }

    static void postPurchaseResult(String requestId, String status, String productId) {
        CustomTabsSession session = messageSession;
        if (session == null) {
            Log.w(TAG, "purchase result dropped; TWA message session unavailable");
            return;
        }
        try {
            JSONObject result = new JSONObject()
                    .put("type", "purchase_result")
                    .put("request_id", requestId == null ? "" : requestId)
                    .put("status", status);
            if (productId != null) {
                result.put("product_id", productId);
            }
            int code = session.postMessage(result.toString(), null);
            Log.i(TAG, "purchase result posted status=" + status + " code=" + code);
        } catch (Exception error) {
            Log.w(TAG, "could not post purchase result", error);
        }
    }

    private CustomTabsSession findMessageSession() {
        try {
            Field launcherField = LauncherActivity.class.getDeclaredField("mTwaLauncher");
            launcherField.setAccessible(true);
            Object launcher = launcherField.get(this);
            if (launcher == null) {
                return null;
            }
            Field sessionField = TwaLauncher.class.getDeclaredField("mSession");
            sessionField.setAccessible(true);
            return (CustomTabsSession) sessionField.get(launcher);
        } catch (ReflectiveOperationException | ClassCastException error) {
            Log.w(TAG, "could not access TWA session for bridge", error);
            return null;
        }
    }

    private void hideSystemBars() {
        Window window = getWindow();
        window.setStatusBarColor(Color.TRANSPARENT);
        window.setNavigationBarColor(Color.TRANSPARENT);
        window.getDecorView().setSystemUiVisibility(
                View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                        | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                        | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_FULLSCREEN
                        | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY);
    }

    @Override
    protected Uri getLaunchingUrl() {
        rendezvousId = UUID.randomUUID().toString().replace("-", "");
        Uri.Builder builder = Uri.parse("https://shadowsofwar.io/play/")
                .buildUpon()
                .appendQueryParameter("sow_platform", "android")
                .appendQueryParameter("sow_playgames_rendezvous", rendezvousId);
        if (PlayGamesAuth.isAnonymousOptOut(this)) {
            builder.appendQueryParameter("sow_playgames_mode", "anonymous");
        }
        return builder.build();
    }

    private void checkAuthentication() {
        if (requestInFlight || signInClient == null) {
            return;
        }
        requestInFlight = true;
        signInClient.isAuthenticated().addOnCompleteListener(task -> {
            Log.i(TAG, "isAuthenticated success=" + task.isSuccessful());
            if (task.isSuccessful() && task.getResult() != null && task.getResult().isAuthenticated()) {
                Log.i(TAG, "automatic Play Games session authenticated");
                PlayGamesAuth.requestServerAccess(
                        this,
                        signInClient,
                        rendezvousId,
                        success -> requestInFlight = false);
            } else {
                requestInFlight = false;
                Log.i(TAG, "automatic Play Games authentication unavailable; continuing anonymously");
            }
        });
    }

}
