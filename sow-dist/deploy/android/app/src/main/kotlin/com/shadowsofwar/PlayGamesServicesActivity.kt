package com.shadowsofwar

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import com.google.android.gms.games.PlayGames
import com.google.android.gms.games.PlayGamesSdk
import com.google.android.gms.tasks.Task

/** Native entry points for Play Games achievements, leaderboards, and explicit sign-in. */
class PlayGamesServicesActivity : Activity() {
    companion object {
        private const val REQUEST_PLAY_GAMES_UI = 4101
    }

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        PlayGamesSdk.initialize(applicationContext)
        val uri = intent?.data
        when (uri?.pathSegments?.firstOrNull().orEmpty()) {
            "signout" -> {
                PlayGamesAuth.setAnonymousOptOut(this, true)
                finish()
            }
            "signin" -> if (uri != null) signIn(uri) else finish()
            "achievements" -> launchUi(PlayGames.getAchievementsClient(this).achievementsIntent)
            "leaderboards" -> launchUi(PlayGames.getLeaderboardsClient(this).allLeaderboardsIntent)
            else -> finish()
        }
    }

    private fun signIn(uri: Uri) {
        val rendezvousId = uri.getQueryParameter("rendezvous_id")
        if (rendezvousId.isNullOrBlank()) {
            finish()
            return
        }
        val client = PlayGames.getGamesSignInClient(this)
        client.signIn().addOnCompleteListener(this) { task ->
            if (!task.isSuccessful) {
                finish()
                return@addOnCompleteListener
            }
            PlayGamesAuth.requestServerAccess(this, client, rendezvousId) { success ->
                if (success) PlayGamesAuth.setAnonymousOptOut(this, false)
                finish()
            }
        }
    }

    private fun launchUi(task: Task<Intent>) {
        task.addOnSuccessListener(this) { launchedIntent ->
            startActivityForResult(launchedIntent, REQUEST_PLAY_GAMES_UI)
        }.addOnFailureListener(this) {
            finish()
        }
    }
}
