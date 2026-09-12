package com.shadowsofwar

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import android.widget.TextView
import com.revenuecat.purchases.CustomerInfo
import com.revenuecat.purchases.Offering
import com.revenuecat.purchases.Offerings
import com.revenuecat.purchases.Package
import com.revenuecat.purchases.PurchaseParams
import com.revenuecat.purchases.Purchases
import com.revenuecat.purchases.PurchasesConfiguration
import com.revenuecat.purchases.PurchasesError
import com.revenuecat.purchases.interfaces.LogInCallback
import com.revenuecat.purchases.interfaces.PurchaseCallback
import com.revenuecat.purchases.interfaces.ReceiveCustomerInfoCallback
import com.revenuecat.purchases.interfaces.ReceiveOfferingsCallback
import com.revenuecat.purchases.models.StoreTransaction

/** Native Google Play checkout and restore bridge for the TWA. */
class PurchaseActivity : Activity() {
    companion object {
        private const val TAG = "SOW Purchases"
        private val STORE_PRODUCTS = setOf("sow_gems_500", "sow_gems_1200", "sow_gems_2600")

        @JvmStatic
        fun storeProductIds(): Array<String> = STORE_PRODUCTS.toTypedArray()

        @JvmStatic
        fun isStoreProduct(value: String?): Boolean = value != null && STORE_PRODUCTS.contains(value)

        @JvmStatic
        fun isPurchaseUserId(value: String?): Boolean = value?.matches(Regex("p_[0-9a-f]{24}")) == true
    }

    private var requestId = ""

    override fun onCreate(state: Bundle?) {
        super.onCreate(state)
        handleIntent(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        val uri = intent?.data
        requestId = uri?.getQueryParameter("request_id").orEmpty()
        if (uri?.scheme != "sow") {
            fail("Invalid purchase request")
            return
        }
        val appUserId = uri.getQueryParameter("app_user_id")
        if (!isPurchaseUserId(appUserId)) {
            fail("Invalid player account")
            return
        }
        val validAppUserId = appUserId ?: return
        when (uri.host) {
            "restore" -> withConfiguredUser(validAppUserId) { restoreConfigured() }
            "purchase" -> {
                val productId = uri.getQueryParameter("product_id")
                if (!isStoreProduct(productId)) {
                    fail("Invalid store product")
                    return
                }
                val validProductId = productId ?: return
                withConfiguredUser(validAppUserId) { purchaseConfigured(validProductId) }
            }
            else -> fail("Invalid store request")
        }
    }

    private fun purchaseConfigured(productId: String) {
        setMessage("Opening Google Play…")
        Purchases.sharedInstance.getOfferings(object : ReceiveOfferingsCallback {
            override fun onReceived(offerings: Offerings) {
                val current: Offering = offerings.current ?: run {
                    fail("Store is temporarily unavailable")
                    return
                }
                val productPackage: Package? = current.availablePackages.firstOrNull {
                    productId == it.product.id
                }
                if (productPackage == null) {
                    fail("Product is not available")
                    return
                }
                Purchases.sharedInstance.purchase(
                    PurchaseParams.Builder(this@PurchaseActivity, productPackage).build(),
                    object : PurchaseCallback {
                        override fun onCompleted(storeTransaction: StoreTransaction, customerInfo: CustomerInfo) {
                            returnToGame("success", productId)
                        }

                        override fun onError(error: PurchasesError, userCancelled: Boolean) {
                            returnToGame(if (userCancelled) "cancelled" else "error", productId)
                        }
                    },
                )
            }

            override fun onError(error: PurchasesError) {
                fail("Store is temporarily unavailable")
            }
        })
    }

    private fun restoreConfigured() {
        setMessage("Restoring purchases…")
        Purchases.sharedInstance.restorePurchases(object : ReceiveCustomerInfoCallback {
            override fun onReceived(customerInfo: CustomerInfo) {
                returnToGame("restored", null)
            }

            override fun onError(error: PurchasesError) {
                returnToGame("error", null)
            }
        })
    }

    private fun withConfiguredUser(appUserId: String, action: () -> Unit) {
        val key = BuildConfig.REVENUECAT_ANDROID_PUBLIC_KEY.trim()
        if (key.isEmpty()) {
            fail("RevenueCat is not configured")
            return
        }
        try {
            if (!Purchases.isConfigured) {
                Purchases.configure(PurchasesConfiguration.Builder(this, key).appUserID(appUserId).build())
                action()
                return
            }
            val purchases = Purchases.sharedInstance
            if (purchases.appUserID == appUserId) {
                action()
                return
            }
            setMessage("Switching store account…")
            purchases.logIn(appUserId, object : LogInCallback {
                override fun onReceived(customerInfo: CustomerInfo, created: Boolean) {
                    action()
                }

                override fun onError(error: PurchasesError) {
                    fail("Store account could not be selected")
                }
            })
        } catch (error: RuntimeException) {
            Log.e(TAG, "RevenueCat configuration failed", error)
            fail("Store configuration failed")
        }
    }

    private fun setMessage(message: String) {
        setContentView(TextView(this).apply {
            text = message
            textSize = 18f
            setPadding(48, 48, 48, 48)
        })
    }

    private fun fail(message: String) {
        setMessage(message)
        returnToGame("error", null)
    }

    private fun returnToGame(status: String, productId: String?) {
        TwaLauncherActivity.postPurchaseResult(requestId, status, productId)
        finish()
    }
}
