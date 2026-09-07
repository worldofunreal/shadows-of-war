package com.shadowsofwar;

import android.app.Activity;
import android.net.Uri;
import android.os.Bundle;

import com.google.android.gms.games.PlayGames;
import com.google.android.gms.games.PlayGamesSdk;
import com.google.android.gms.games.GamesSignInClient;
import com.google.android.gms.tasks.Task;

/** Native entry points for Play Games achievements, leaderboards, and events. */
public final class PlayGamesServicesActivity extends Activity {
    private static final int REQUEST_PLAY_GAMES_UI = 4101;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        PlayGamesSdk.initialize(getApplicationContext());
        Uri uri = getIntent().getData();
        String action = uri == null || uri.getPathSegments().isEmpty()
                ? ""
                : uri.getPathSegments().get(0);
        if ("signout".equals(action)) {
            PlayGamesAuth.setAnonymousOptOut(this, true);
            finish();
        } else if ("signin".equals(action)) {
            signIn(uri);
        } else if ("auto_signin".equals(action)) {
            autoSignIn(uri);
        } else if ("achievements".equals(action)) {
            launchUi(PlayGames.getAchievementsClient(this).getAchievementsIntent());
        } else if ("leaderboards".equals(action)) {
            launchUi(PlayGames.getLeaderboardsClient(this).getAllLeaderboardsIntent());
        } else {
            finish();
        }
    }

    private void signIn(Uri uri) {
        String rendezvousId = uri == null ? null : uri.getQueryParameter("rendezvous_id");
        if (rendezvousId == null || rendezvousId.isEmpty()) {
            finish();
            return;
        }
        PlayGames.getGamesSignInClient(this).signIn().addOnCompleteListener(this, task -> {
            if (!task.isSuccessful()) {
                finish();
                return;
            }
            PlayGamesAuth.requestServerAccess(
                    this,
                    PlayGames.getGamesSignInClient(this),
                    rendezvousId,
                    success -> {
                        if (success) {
                            PlayGamesAuth.setAnonymousOptOut(this, false);
                        }
                        finish();
                    });
        });
    }

    private void autoSignIn(Uri uri) {
        String rendezvousId = uri == null ? null : uri.getQueryParameter("rendezvous_id");
        if (rendezvousId == null || rendezvousId.isEmpty() || PlayGamesAuth.isAnonymousOptOut(this)) {
            finish();
            return;
        }
        GamesSignInClient client = PlayGames.getGamesSignInClient(this);
        client.isAuthenticated().addOnCompleteListener(this, task -> {
            if (task.isSuccessful() && task.getResult() != null && task.getResult().isAuthenticated()) {
                requestServerAccess(client, rendezvousId, true);
                return;
            }
            client.signIn().addOnCompleteListener(this, signInTask -> {
                if (!signInTask.isSuccessful()) {
                    PlayGamesAuth.setAnonymousOptOut(this, true);
                    finish();
                    return;
                }
                requestServerAccess(client, rendezvousId, true);
            });
        });
    }

    private void requestServerAccess(
            GamesSignInClient client,
            String rendezvousId,
            boolean automatic) {
        PlayGamesAuth.requestServerAccess(this, client, rendezvousId, success -> {
            if (automatic && !success) {
                PlayGamesAuth.setAnonymousOptOut(this, true);
            } else if (success) {
                PlayGamesAuth.setAnonymousOptOut(this, false);
            }
            finish();
        });
    }

    private void launchUi(Task<android.content.Intent> task) {
        task.addOnSuccessListener(this, intent -> startActivityForResult(intent, REQUEST_PLAY_GAMES_UI))
                .addOnFailureListener(this, error -> finish());
    }

}
