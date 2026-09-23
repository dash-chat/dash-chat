package studio.darksoil.dashchat

import android.os.Bundle
import android.view.View
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    if (!webViewReportsSystemBarInsets()) {
      insetContentFromSystemBars()
    }
  }

  // WebView only exposes the system bars through env(safe-area-inset-*) from
  // M136; older ones report 0 there, so the page would draw under the bars.
  private fun webViewReportsSystemBarInsets(): Boolean {
    val major = WebView.getCurrentWebViewPackage()?.versionName
      ?.substringBefore('.')?.toIntOrNull() ?: return false
    return major >= 136
  }

  /**
   * Keeps the webview between the system bars instead of edge-to-edge. The
   * webview gets the bars zeroed out, so a WebView that does report them
   * can't pad the page a second time.
   */
  private fun insetContentFromSystemBars() {
    val bars = WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
    ViewCompat.setOnApplyWindowInsetsListener(findViewById<View>(android.R.id.content)) { view, insets ->
      val inset = insets.getInsets(bars)
      view.setPadding(inset.left, inset.top, inset.right, inset.bottom)
      WindowInsetsCompat.Builder(insets)
        .setInsets(bars, Insets.NONE)
        .setDisplayCutout(null)
        .build()
    }
  }
}
