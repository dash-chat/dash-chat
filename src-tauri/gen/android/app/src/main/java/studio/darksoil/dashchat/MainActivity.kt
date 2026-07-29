package studio.darksoil.dashchat

import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import app.tauri.backgroundservice.HeadlessBridge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    HeadlessBridge.nativeLibName = "tauri_app_lib"
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }
}
