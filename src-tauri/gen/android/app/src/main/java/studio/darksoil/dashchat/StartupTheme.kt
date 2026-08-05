package studio.darksoil.dashchat

import android.app.Activity
import android.graphics.drawable.ColorDrawable
import org.json.JSONObject
import java.io.File

/**
 * The window background is what shows through the transparent webview until the
 * UI has rendered. `values-night` already resolves it for the system theme, so
 * only an in-app override of that theme needs applying here.
 */
fun Activity.applyColorSchemeOverride() {
  val background = when (readColorScheme()) {
    "dark" -> R.color.app_background_dark
    "light" -> R.color.app_background_light
    else -> return
  }
  window.setBackgroundDrawable(ColorDrawable(getColor(background)))
}

private fun Activity.readColorScheme(): String? {
  val settings = File(dataDir, "${BuildConfig.DATABASE_VERSION}/settings.json")
  if (!settings.isFile) return null
  // Settings are rewritten in place, so a torn read is possible.
  return runCatching { JSONObject(settings.readText()).optString("color_scheme") }
    .getOrNull()
}
