package com.shadowsofwar;

import android.app.Activity;
import android.content.Context;
import android.content.SharedPreferences;
import android.util.Log;

import com.google.android.gms.games.GamesSignInClient;

import org.json.JSONObject;

import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;

/** Small shared bridge for Play Games identity and the existing server rendezvous. */
final class PlayGamesAuth {
    private static final String TAG = "SOW_PGS";
    private static final String PREFS = "sow_playgames";
    private static final String MODE = "identity_mode";
    private static final String PLAYGAMES = "playgames";
    private static final String ANONYMOUS = "anonymous_opt_out";

    interface Callback {
        void onComplete(boolean success);
    }

    private PlayGamesAuth() {}

    static boolean isAnonymousOptOut(Context context) {
        return ANONYMOUS.equals(prefs(context).getString(MODE, PLAYGAMES));
    }

    static void setAnonymousOptOut(Context context, boolean enabled) {
        prefs(context).edit().putString(MODE, enabled ? ANONYMOUS : PLAYGAMES).commit();
    }

    static void requestServerAccess(
            Activity activity,
            GamesSignInClient signInClient,
            String rendezvousId,
            Callback callback) {
        String clientId = BuildConfig.PLAY_GAMES_WEB_CLIENT_ID.trim();
        if (clientId.isEmpty() || rendezvousId == null || rendezvousId.isEmpty()) {
            Log.e(TAG, "server access configuration is incomplete");
            callback.onComplete(false);
            return;
        }
        signInClient.requestServerSideAccess(clientId, false).addOnCompleteListener(task -> {
            Log.i(TAG, "server access success=" + task.isSuccessful());
            String serverAuthCode = task.isSuccessful() ? task.getResult() : null;
            if (serverAuthCode == null || serverAuthCode.isEmpty()) {
                callback.onComplete(false);
                return;
            }
            exchangeCode(activity, serverAuthCode, rendezvousId, callback);
        });
    }

    private static void exchangeCode(
            Activity activity,
            String serverAuthCode,
            String rendezvousId,
            Callback callback) {
        Log.i(TAG, "exchanging server auth code");
        new Thread(() -> {
            boolean success = false;
            HttpURLConnection connection = null;
            try {
                String base = BuildConfig.PLAY_GAMES_AUTH_URL.replaceAll("/+$", "");
                URL url = new URL(base + "/auth/playgames/exchange");
                connection = (HttpURLConnection) url.openConnection();
                connection.setRequestMethod("POST");
                connection.setConnectTimeout(5000);
                connection.setReadTimeout(5000);
                connection.setDoOutput(true);
                connection.setRequestProperty("Content-Type", "application/json");
                JSONObject body = new JSONObject()
                        .put("server_auth_code", serverAuthCode)
                        .put("package_name", activity.getPackageName())
                        .put("rendezvous_id", rendezvousId);
                byte[] bytes = body.toString().getBytes(StandardCharsets.UTF_8);
                try (OutputStream output = connection.getOutputStream()) {
                    output.write(bytes);
                }
                int status = connection.getResponseCode();
                success = status >= 200 && status < 300;
                Log.i(TAG, "Play Games rendezvous success=" + success + " HTTP " + status);
            } catch (Exception error) {
                Log.w(TAG, "Play Games handoff unavailable", error);
            } finally {
                if (connection != null) {
                    connection.disconnect();
                }
            }
            boolean result = success;
            activity.runOnUiThread(() -> callback.onComplete(result));
        }, "sow-playgames-auth").start();
    }

    private static SharedPreferences prefs(Context context) {
        return context.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }
}
