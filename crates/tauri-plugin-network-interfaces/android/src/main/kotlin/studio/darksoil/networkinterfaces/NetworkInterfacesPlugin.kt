package studio.darksoil.networkinterfaces

import android.app.Activity
import android.content.Context
import android.net.wifi.WifiManager
import android.webkit.WebView
import app.tauri.Logger
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Plugin

/**
 * Holds a [WifiManager.MulticastLock] so mDNS replies actually reach the app.
 *
 * Without it the wifi driver discards multicast that is not addressed to this
 * device, which is every mDNS announcement on 224.0.0.251 — so a hub that starts
 * while we are already browsing is never heard, and discovery has to wait for
 * the next outgoing query instead.
 *
 * The lock is taken from both [load] and [onResume] because either can be the
 * first to run: `PluginManager.load` only calls [load] when a WebView already
 * exists, and [onResume] only reaches plugins registered before the activity
 * resumed. Taking it twice is harmless — the lock is not reference counted and
 * [acquireMulticastLock] is a no-op while it is already held — whereas relying
 * on either one alone leaves a startup ordering in which it is never taken.
 */
@TauriPlugin
class NetworkInterfacesPlugin(private val activity: Activity) : Plugin(activity) {
    private var multicastLock: WifiManager.MulticastLock? = null

    override fun load(webView: WebView) {
        super.load(webView)
        acquireMulticastLock()
    }

    override fun onResume() {
        super.onResume()
        acquireMulticastLock()
    }

    override fun onPause() {
        releaseMulticastLock()
        super.onPause()
    }

    override fun onDestroy() {
        releaseMulticastLock()
        super.onDestroy()
    }

    private fun acquireMulticastLock() {
        if (multicastLock?.isHeld == true) return
        try {
            val wifi = activity.applicationContext
                .getSystemService(Context.WIFI_SERVICE) as WifiManager
            multicastLock = wifi.createMulticastLock(LOCK_TAG).apply {
                setReferenceCounted(false)
                acquire()
            }
            // Logged because the only other way to tell whether this ran is
            // `adb shell dumpsys wifi`, and a silently inert lock looks exactly
            // like a working one from the app's side.
            Logger.info(TAG, "acquired multicast lock")
        } catch (e: Exception) {
            // A device without wifi, or an OEM that refuses the lock, still works
            // over unicast and cloud sync — only local hub discovery degrades.
            Logger.error(TAG, "failed to acquire multicast lock", e)
        }
    }

    private fun releaseMulticastLock() {
        multicastLock?.takeIf { it.isHeld }?.let {
            it.release()
            Logger.info(TAG, "released multicast lock")
        }
        multicastLock = null
    }

    private companion object {
        const val TAG = "NetworkInterfaces"
        const val LOCK_TAG = "dashchat-mdns"
    }
}
