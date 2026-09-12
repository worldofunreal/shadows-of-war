package com.shadowsofwar

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import android.view.Window
import androidx.browser.customtabs.CustomTabsCallback
import androidx.browser.customtabs.CustomTabsSession
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.google.android.gms.games.PlayGames
import com.google.android.gms.games.PlayGamesSdk
import com.google.androidbrowserhelper.trusted.LauncherActivity
import com.google.androidbrowserhelper.trusted.TwaLauncher
import org.json.JSONArray
import org.json.JSONObject
import java.lang.reflect.Field
import java.util.UUID

/** Opens the TWA. Play Games auth starts only after the web loader is ready. */
class TwaLauncherActivity : LauncherActivity() {
    companion object {
        private const val TAG = "SOW_PGS"
        private val SOW_ORIGIN: Uri = Uri.parse("https://shadowsofwar.io")

        @Volatile
        private var messageSession: CustomTabsSession? = null

        @JvmStatic
        fun postPurchaseResult(requestId: String?, status: String, productId: String?) {
            val session = messageSession
            if (session == null) {
                Log.w(TAG, "purchase result dropped; TWA message session unavailable")
                return
            }
            try {
                val result = JSONObject()
                    .put("type", "purchase_result")
                    .put("request_id", requestId.orEmpty())
                    .put("status", status)
                if (productId != null) result.put("product_id", productId)
                val code = session.postMessage(result.toString(), null)
                Log.i(TAG, "purchase result posted status=$status code=$code")
            } catch (error: Exception) {
                Log.w(TAG, "could not post purchase result", error)
            }
        }

        @JvmStatic
        fun postPlayGamesAuthResult(status: String, rendezvousId: String?) {
            val session = messageSession
            if (session == null) {
                Log.w(TAG, "Play Games auth result dropped; TWA message session unavailable")
                return
            }
            try {
                val result = JSONObject()
                    .put("type", "playgames_auth_result")
                    .put("status", status)
                    .put("rendezvous_id", rendezvousId.orEmpty())
                val code = session.postMessage(result.toString(), null)
                Log.i(TAG, "Play Games auth result posted status=$status code=$code")
            } catch (error: Exception) {
                Log.w(TAG, "could not post Play Games auth result", error)
            }
        }
    }

    private var rendezvousId: String? = null
    private var silentAuthInFlight = false

    override fun onCreate(state: Bundle?) {
        hideSystemBars()
        super.onCreate(state)
        hideSystemBars()
        Log.i(
            TAG,
            "artifact source_sha=${BuildConfig.SOW_SOURCE_SHA} " +
                "version_code=${BuildConfig.VERSION_CODE} package=${packageName}"
        )
        Log.i(TAG, "TWA launcher ready; Play Games waits for loader-ready")
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) hideSystemBars()
    }

    override fun getCustomTabsCallback(): CustomTabsCallback = object : CustomTabsCallback() {
        override fun onNavigationEvent(navigationEvent: Int, extras: Bundle?) {
            if (navigationEvent != CustomTabsCallback.NAVIGATION_FINISHED) return
            val session = findMessageSession()
            if (session == null) {
                Log.w(TAG, "TWA session unavailable; native bridge disabled")
                return
            }
            messageSession = session
            val requested = session.requestPostMessageChannel(SOW_ORIGIN, SOW_ORIGIN, Bundle())
            Log.i(TAG, "native bridge channel requested=$requested")
        }

        override fun onMessageChannelReady(extras: Bundle?) {
            val session = findMessageSession() ?: return
            messageSession = session
            val products = JSONArray()
            PurchaseActivity.storeProductIds().forEach(products::put)
            val ready = JSONObject()
                .put("type", "sow_bridge_ready")
                .put("bridge_version", 2)
                .put(
                    "capabilities",
                    JSONArray()
                        .put("purchase")
                        .put("restore")
                        .put("playgames_silent_auth")
                )
                .put("products", products)
            val result = session.postMessage(ready.toString(), null)
            Log.i(TAG, "native bridge ready result=$result")
        }

        override fun onPostMessage(message: String, extras: Bundle?) {
            handleBridgeMessage(message)
        }
    }

    private fun handleBridgeMessage(message: String) {
        try {
            val request = JSONObject(message)
            when (request.optString("type")) {
                "playgames_silent_auth" -> startSilentPlayGamesAuth(request)
                "purchase" -> handlePurchaseRequest(request)
                "restore" -> handleRestoreRequest(request)
            }
        } catch (error: Exception) {
            Log.w(TAG, "invalid native bridge message", error)
        }
    }

    private fun startSilentPlayGamesAuth(request: JSONObject) {
        if (silentAuthInFlight) return
        val localRendezvous = rendezvousId
        val requestedRendezvous = request.optString("rendezvous_id", "")
        Log.i(TAG, "loader-ready Play Games silent auth requested")
        if (localRendezvous.isNullOrBlank() || requestedRendezvous != localRendezvous) {
            postPlayGamesAuthResult("error", localRendezvous)
            return
        }
        if (PlayGamesAuth.isAnonymousOptOut(this)) {
            postPlayGamesAuthResult("unavailable", localRendezvous)
            return
        }

        silentAuthInFlight = true
        PlayGamesSdk.initialize(applicationContext)
        val client = PlayGames.getGamesSignInClient(this)
        client.isAuthenticated().addOnCompleteListener { task ->
            val authenticated = task.isSuccessful && task.getResult()?.isAuthenticated == true
            Log.i(TAG, "loader-ready isAuthenticated success=${task.isSuccessful} authenticated=$authenticated")
            if (!authenticated) {
                silentAuthInFlight = false
                postPlayGamesAuthResult("unavailable", localRendezvous)
                return@addOnCompleteListener
            }
            PlayGamesAuth.requestServerAccess(this, client, localRendezvous) { success ->
                silentAuthInFlight = false
                if (success) PlayGamesAuth.setAnonymousOptOut(this, false)
                postPlayGamesAuthResult(if (success) "ready" else "error", localRendezvous)
            }
        }
    }

    private fun handlePurchaseRequest(request: JSONObject) {
        val productId = request.optString("product_id", "")
        val appUserId = request.optString("app_user_id", "")
        val requestId = request.optString("request_id", "")
        if (!PurchaseActivity.isStoreProduct(productId) ||
            !PurchaseActivity.isPurchaseUserId(appUserId) ||
            !validRequestId(requestId)
        ) {
            Log.w(TAG, "rejected invalid purchase bridge request")
            postPurchaseResult(requestId, "error", productId)
            return
        }
        val data = Uri.parse("sow://purchase").buildUpon()
            .appendQueryParameter("product_id", productId)
            .appendQueryParameter("app_user_id", appUserId)
            .appendQueryParameter("request_id", requestId)
            .build()
        startActivity(Intent(this, PurchaseActivity::class.java).setData(data))
    }

    private fun handleRestoreRequest(request: JSONObject) {
        val appUserId = request.optString("app_user_id", "")
        val requestId = request.optString("request_id", "")
        if (!PurchaseActivity.isPurchaseUserId(appUserId) || !validRequestId(requestId)) {
            Log.w(TAG, "rejected invalid restore bridge request")
            postPurchaseResult(requestId, "error", null)
            return
        }
        val data = Uri.parse("sow://restore").buildUpon()
            .appendQueryParameter("app_user_id", appUserId)
            .appendQueryParameter("request_id", requestId)
            .build()
        startActivity(Intent(this, PurchaseActivity::class.java).setData(data))
    }

    private fun validRequestId(value: String): Boolean = value.matches(Regex("[A-Za-z0-9_-]{1,80}"))

    private fun findMessageSession(): CustomTabsSession? {
        return try {
            val launcherField: Field = LauncherActivity::class.java.getDeclaredField("mTwaLauncher")
            launcherField.isAccessible = true
            val launcher = launcherField.get(this) ?: return null
            val sessionField: Field = TwaLauncher::class.java.getDeclaredField("mSession")
            sessionField.isAccessible = true
            sessionField.get(launcher) as? CustomTabsSession
        } catch (error: ReflectiveOperationException) {
            Log.w(TAG, "could not access TWA session for bridge", error)
            null
        } catch (error: ClassCastException) {
            Log.w(TAG, "TWA session has an unexpected type", error)
            null
        }
    }

    private fun hideSystemBars() {
        val currentWindow: Window = window
        WindowCompat.setDecorFitsSystemWindows(currentWindow, false)
        WindowInsetsControllerCompat(currentWindow, currentWindow.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
    }

    override fun getLaunchingUrl(): Uri {
        rendezvousId = UUID.randomUUID().toString().replace("-", "")
        val builder = Uri.parse("https://shadowsofwar.io/play/").buildUpon()
            .appendQueryParameter("sow_platform", "android")
            .appendQueryParameter("sow_playgames_rendezvous", rendezvousId)
            .appendQueryParameter("sow_source_sha", BuildConfig.SOW_SOURCE_SHA)
        if (PlayGamesAuth.isAnonymousOptOut(this)) {
            builder.appendQueryParameter("sow_playgames_mode", "anonymous")
        }
        return builder.build()
    }
}
