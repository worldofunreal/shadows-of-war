package com.shadowsofwar

import android.app.Activity
import android.content.Context
import android.util.Log
import com.google.android.gms.games.GamesSignInClient
import org.json.JSONObject
import java.io.OutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.nio.charset.StandardCharsets

/** Shared Play Games identity bridge for the server rendezvous. */
internal object PlayGamesAuth {
    private const val TAG = "SOW_PGS"
    private const val PREFS = "sow_playgames"
    private const val MODE = "identity_mode"
    private const val PLAYGAMES = "playgames"
    private const val ANONYMOUS = "anonymous_opt_out"

    fun interface Callback {
        fun onComplete(success: Boolean)
    }

    fun isAnonymousOptOut(context: Context): Boolean =
        prefs(context).getString(MODE, PLAYGAMES) == ANONYMOUS

    fun setAnonymousOptOut(context: Context, enabled: Boolean) {
        prefs(context).edit().putString(MODE, if (enabled) ANONYMOUS else PLAYGAMES).commit()
    }

    fun requestServerAccess(
        activity: Activity,
        signInClient: GamesSignInClient,
        rendezvousId: String?,
        callback: Callback,
    ) {
        val clientId = BuildConfig.PLAY_GAMES_WEB_CLIENT_ID.trim()
        if (clientId.isEmpty() || rendezvousId.isNullOrBlank()) {
            Log.e(TAG, "server access configuration is incomplete")
            callback.onComplete(false)
            return
        }
        signInClient.requestServerSideAccess(clientId, false).addOnCompleteListener { task ->
            Log.i(TAG, "server access success=${task.isSuccessful}")
            val serverAuthCode = if (task.isSuccessful) task.getResult() else null
            if (serverAuthCode.isNullOrBlank()) {
                callback.onComplete(false)
                return@addOnCompleteListener
            }
            exchangeCode(activity, serverAuthCode, rendezvousId, callback)
        }
    }

    private fun exchangeCode(
        activity: Activity,
        serverAuthCode: String,
        rendezvousId: String,
        callback: Callback,
    ) {
        Log.i(TAG, "exchanging server auth code")
        Thread({
            var success = false
            var connection: HttpURLConnection? = null
            try {
                val base = BuildConfig.PLAY_GAMES_AUTH_URL.trimEnd('/')
                connection = (URL("$base/auth/playgames/exchange").openConnection() as HttpURLConnection).apply {
                    requestMethod = "POST"
                    connectTimeout = 5000
                    readTimeout = 5000
                    doOutput = true
                    setRequestProperty("Content-Type", "application/json")
                }
                val body = JSONObject()
                    .put("server_auth_code", serverAuthCode)
                    .put("package_name", activity.packageName)
                    .put("rendezvous_id", rendezvousId)
                val bytes = body.toString().toByteArray(StandardCharsets.UTF_8)
                connection.outputStream.use { output: OutputStream -> output.write(bytes) }
                val status = connection.responseCode
                success = status in 200..299
                Log.i(TAG, "Play Games rendezvous success=$success HTTP $status")
            } catch (error: Exception) {
                Log.w(TAG, "Play Games handoff unavailable", error)
            } finally {
                connection?.disconnect()
            }
            val result = success
            activity.runOnUiThread { callback.onComplete(result) }
        }, "sow-playgames-auth").start()
    }

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
}
