package com.clipsync.app

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import com.clipsync.share.MacShareActivity
import com.clipsync.util.L
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
import com.clipsync.ui.theme.ThemeSwitchAnimator

class MainActivity : ComponentActivity() {
    companion object { private const val M = "UI" }

    private val deepLinkUri = mutableStateOf<Uri?>(null)

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        intent.data?.takeIf { it.scheme == "clipsync" }?.let {
            L.event(M, "deepLink onNewIntent uri=$it")
            deepLinkUri.value = it
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        val darkScrim = android.graphics.Color.TRANSPARENT
        val themePrefs = getSharedPreferences("clipsync_ui", MODE_PRIVATE)
        val initialDark = themePrefs.getBoolean("dark_mode", true)
        val initialStyle = if (initialDark) SystemBarStyle.dark(darkScrim)
            else SystemBarStyle.light(darkScrim, darkScrim)
        enableEdgeToEdge(
            statusBarStyle = initialStyle,
            navigationBarStyle = initialStyle,
        )
        super.onCreate(savedInstanceState)

        intent?.data?.takeIf { it.scheme == "clipsync" }?.let {
            L.event(M, "deepLink onCreate uri=$it")
            deepLinkUri.value = it
        }

        val prefs = Prefs(applicationContext)
        if (prefs.hasPairing() && prefs.syncEnabled) {
            ClipForegroundService.start(applicationContext)
        }

        registerMacShareShortcut()

        setContent {
            var isDark by rememberSaveable { mutableStateOf(initialDark) }

            ClipSyncTheme(isDark = isDark) {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = NeuColors.Background
                ) {
                    SettingsScreen(
                        isDark = isDark,
                        deepLinkUri = deepLinkUri.value,
                        onToggleTheme = { cx, cy ->
                            ThemeSwitchAnimator.animateThemeSwitch(
                                activity = this@MainActivity,
                                cx = cx,
                                cy = cy,
                                onMidpoint = {
                                    isDark = !isDark
                                    L.action(M, "toggleTheme isDark=$isDark")
                                    themePrefs.edit().putBoolean("dark_mode", isDark).apply()
                                    val style = if (isDark) SystemBarStyle.dark(darkScrim)
                                        else SystemBarStyle.light(darkScrim, darkScrim)
                                    enableEdgeToEdge(
                                        statusBarStyle = style,
                                        navigationBarStyle = style,
                                    )
                                },
                            )
                        }
                    )
                }
            }
        }
    }

    private fun registerMacShareShortcut() {
        val shortcut = ShortcutInfoCompat.Builder(this, "mac_share_target")
            .setShortLabel("Mac")
            .setIcon(IconCompat.createWithResource(this, R.drawable.ic_mac_share))
            .setIntent(
                Intent(Intent.ACTION_SEND, null, this, MacShareActivity::class.java)
            )
            .setCategories(setOf("com.clipsync.category.SHARE_TARGET"))
            .setLongLived(true)
            .build()
        ShortcutManagerCompat.pushDynamicShortcut(this, shortcut)
    }
}
