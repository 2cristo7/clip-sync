package com.clipsync.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import com.clipsync.service.ClipForegroundService
import com.clipsync.storage.Prefs
import com.clipsync.ui.SettingsScreen
import com.clipsync.ui.theme.ClipSyncTheme
import com.clipsync.ui.theme.NeuColors

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        val darkScrim = android.graphics.Color.TRANSPARENT
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(darkScrim),
            navigationBarStyle = SystemBarStyle.dark(darkScrim),
        )
        super.onCreate(savedInstanceState)

        val prefs = Prefs(applicationContext)
        if (prefs.hasPairing() && prefs.syncEnabled) {
            ClipForegroundService.start(applicationContext)
        }

        val themePrefs = getSharedPreferences("clipsync_ui", MODE_PRIVATE)
        val initialDark = themePrefs.getBoolean("dark_mode", true)

        setContent {
            var isDark by rememberSaveable { mutableStateOf(initialDark) }

            ClipSyncTheme(isDark = isDark) {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = NeuColors.Background
                ) {
                    SettingsScreen(
                        isDark = isDark,
                        onToggleTheme = {
                            isDark = !isDark
                            themePrefs.edit().putBoolean("dark_mode", isDark).apply()
                            val style = if (isDark) SystemBarStyle.dark(darkScrim)
                                else SystemBarStyle.light(darkScrim, darkScrim)
                            enableEdgeToEdge(
                                statusBarStyle = style,
                                navigationBarStyle = style,
                            )
                        }
                    )
                }
            }
        }
    }
}
