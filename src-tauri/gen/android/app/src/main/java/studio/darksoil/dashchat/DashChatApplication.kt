package studio.darksoil.dashchat

import android.app.Application
import app.tauri.backgroundservice.HeadlessBridge

class DashChatApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        HeadlessBridge.nativeLibName = "tauri_app_lib"
    }
}
