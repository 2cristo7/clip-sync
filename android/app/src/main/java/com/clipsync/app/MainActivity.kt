package com.clipsync.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import com.clipsync.service.ClipForegroundService
import com.clipsync.storage.Prefs
import com.clipsync.ui.SettingsScreen
import com.clipsync.ui.theme.ClipSyncTheme
import com.clipsync.ui.theme.NeuColors

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // If we already paired and sync is enabled, spin up the foreground service on launch.
        val prefs = Prefs(applicationContext)
        if (prefs.hasPairing() && prefs.syncEnabled) {
            ClipForegroundService.start(applicationContext)
        }

        setContent {
            ClipSyncTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = NeuColors.Background
                ) {
                    SettingsScreen()
                }
            }
        }
    }
}
