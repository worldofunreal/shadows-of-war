package com.shadowsofwar;

import android.graphics.Color;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;
import android.view.View;
import android.view.Window;

import com.google.androidbrowserhelper.trusted.LauncherActivity;
import com.google.android.gms.games.GamesSignInClient;
import com.google.android.gms.games.PlayGames;
import com.google.android.gms.games.PlayGamesSdk;

import java.util.UUID;

/** Opens the TWA; the web loader starts optional Play Games authentication. */
public final class TwaLauncherActivity extends LauncherActivity {
    private static final String TAG = "SOW_PGS";
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
