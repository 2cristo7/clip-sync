package com.clipsync.accessibility

import android.accessibilityservice.AccessibilityService
import android.view.accessibility.AccessibilityEvent

/**
 * Placeholder accessibility service. Registered in the manifest so the user can
 * enable it in Settings → Accessibility, but currently does nothing.
 *
 * Auto-send via clipboard detection is disabled until a reliable mechanism
 * is found that works within Android 10+ background clipboard restrictions.
 * Outbound clipboard sync is handled manually via the overlay FAB.
 */
class ClipAccessibilityService : AccessibilityService() {
    override fun onAccessibilityEvent(event: AccessibilityEvent) = Unit
    override fun onInterrupt() = Unit
}
