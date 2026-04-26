package com.clipsync.overlay

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.PixelFormat
import android.graphics.RectF
import android.graphics.Shader
import android.graphics.drawable.GradientDrawable
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Log
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.view.animation.OvershootInterpolator
import android.widget.FrameLayout
import android.widget.ImageView
import com.clipsync.app.R

/**
 * Manages a floating overlay FAB that appears when the user copies something
 * to the clipboard. The FAB is rendered via [WindowManager] with
 * [WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY] and requires the
 * `SYSTEM_ALERT_WINDOW` permission.
 *
 * Lifecycle:
 *  1. [showFab] — called by the clipboard-changed listener in the foreground
 *     service. Adds the view in ~16-32ms.
 *  2. User taps → launches [SendClipActivity] trampoline.
 *  3. Auto-dismiss after [AUTO_DISMISS_MS] if untouched.
 */
class ClipOverlayManager(private val context: Context) {

    private var fabView: View? = null
    private val wm = context.getSystemService(Context.WINDOW_SERVICE) as WindowManager
    private val handler = Handler(Looper.getMainLooper())

    private val resultReceiver = object : BroadcastReceiver() {
        override fun onReceive(ctx: Context?, intent: Intent?) {
            val success = intent?.getBooleanExtra(SendClipActivity.EXTRA_SUCCESS, false) ?: false
            if (success) {
                showSuccessFeedback()
            } else {
                showErrorFeedback()
            }
        }
    }

    private var receiverRegistered = false

    fun showFab() {
        // Must be called on the main thread.
        if (fabView != null) return

        if (!Settings.canDrawOverlays(context)) {
            Log.d(TAG, "SYSTEM_ALERT_WINDOW not granted — skipping overlay")
            return
        }

        val size = dpToPx(56)
        val params = WindowManager.LayoutParams(
            size, size,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE
                or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL
                or WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS,
            PixelFormat.TRANSLUCENT
        ).apply {
            gravity = Gravity.TOP or Gravity.END
            x = dpToPx(16)
            y = dpToPx(80)
        }

        val view = buildFabView(size)
        view.setOnClickListener {
            // Give visual feedback of tap
            view.alpha = 1.0f
            view.scaleX = 0.9f
            view.scaleY = 0.9f
            view.animate().scaleX(1f).scaleY(1f).setDuration(100).start()
            
            launchSendClipActivity()
        }

        try {
            wm.addView(view, params)
            fabView = view
        } catch (e: Exception) {
            Log.e(TAG, "Failed to add overlay: ${e.message}")
            return
        }

        // Register for send results
        if (!receiverRegistered) {
            val filter = IntentFilter(SendClipActivity.ACTION_SEND_RESULT)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                context.registerReceiver(resultReceiver, filter, Context.RECEIVER_NOT_EXPORTED)
            } else {
                context.registerReceiver(resultReceiver, filter)
            }
            receiverRegistered = true
        }

        // Entrance animation: fade to semi-transparent + overshoot scale
        view.alpha = 0f
        view.scaleX = 0.4f
        view.scaleY = 0.4f
        view.animate()
            .alpha(0.6f) // Idle state is semi-transparent so it's less intrusive
            .scaleX(1f)
            .scaleY(1f)
            .setDuration(200)
            .setInterpolator(OvershootInterpolator(2.0f))
            .start()
    }

    fun dismiss() {
        val view = fabView ?: return
        view.animate()
            .alpha(0f)
            .scaleX(0.4f)
            .scaleY(0.4f)
            .setDuration(120)
            .withEndAction {
                try {
                    wm.removeViewImmediate(view)
                } catch (_: Exception) { }
            }
            .start()
        fabView = null
    }

    fun destroy() {
        dismiss()
        if (receiverRegistered) {
            try { context.unregisterReceiver(resultReceiver) } catch (_: Exception) { }
            receiverRegistered = false
        }
    }

    private fun showSuccessFeedback() {
        val view = fabView ?: return
        val icon = view.findViewWithTag<ImageView>("icon")
        icon?.setColorFilter(Color.parseColor("#2ECC76"))
        view.alpha = 1.0f

        handler.postDelayed({ 
            icon?.setColorFilter(Color.WHITE)
            view.animate().alpha(0.6f).setDuration(300).start()
        }, 1000)
    }

    private fun showErrorFeedback() {
        val view = fabView ?: return
        val icon = view.findViewWithTag<ImageView>("icon")
        icon?.setColorFilter(Color.parseColor("#F44336"))
        view.alpha = 1.0f

        handler.postDelayed({ 
            icon?.setColorFilter(Color.WHITE)
            view.animate().alpha(0.6f).setDuration(300).start()
        }, 1000)
    }

    private fun launchSendClipActivity() {
        context.startActivity(SendClipActivity.intent(context))
    }

    /**
     * Build the FAB view with claymorphic styling:
     * - Soft rounded shape with gradient
     * - Subtle inner highlight (top edge)
     * - Diffuse outer shadow
     */
    private fun buildFabView(sizePx: Int): View {
        val container = FrameLayout(context)

        // Background circle with claymorphic gradient
        val bg = object : View(context) {
            private val paint = Paint(Paint.ANTI_ALIAS_FLAG)
            private val highlightPaint = Paint(Paint.ANTI_ALIAS_FLAG)
            private val shadowPaint = Paint(Paint.ANTI_ALIAS_FLAG)
            private val rect = RectF()

            override fun onDraw(canvas: Canvas) {
                val w = width.toFloat()
                val h = height.toFloat()
                val inset = dpToPx(4).toFloat()
                rect.set(inset, inset, w - inset, h - inset)
                val radius = (w - 2 * inset) / 2f

                // Outer shadow (clay feel)
                shadowPaint.setShadowLayer(dpToPx(8).toFloat(), 0f, dpToPx(3).toFloat(), Color.parseColor("#401AAD5A"))
                shadowPaint.color = Color.TRANSPARENT
                canvas.drawOval(rect, shadowPaint)

                // Main body gradient (mint green clay)
                paint.shader = LinearGradient(
                    0f, 0f, 0f, h,
                    Color.parseColor("#4BDD8A"),
                    Color.parseColor("#1AAD5A"),
                    Shader.TileMode.CLAMP
                )
                canvas.drawOval(rect, paint)

                // Inner highlight (top) — simulates 3D lighting
                highlightPaint.shader = LinearGradient(
                    0f, inset, 0f, h * 0.5f,
                    Color.parseColor("#40FFFFFF"),
                    Color.parseColor("#00FFFFFF"),
                    Shader.TileMode.CLAMP
                )
                canvas.drawOval(rect, highlightPaint)
            }

            init {
                setLayerType(LAYER_TYPE_SOFTWARE, null) // required for setShadowLayer
            }
        }
        bg.layoutParams = FrameLayout.LayoutParams(sizePx, sizePx)
        container.addView(bg)

        // Send icon (arrow up)
        val icon = ImageView(context)
        icon.tag = "icon"
        icon.setImageResource(R.drawable.ic_notification) // will be replaced with send icon in Phase 4
        icon.setColorFilter(Color.WHITE)
        val iconSize = dpToPx(24)
        val iconMargin = (sizePx - iconSize) / 2
        val iconParams = FrameLayout.LayoutParams(iconSize, iconSize).apply {
            gravity = Gravity.CENTER
        }
        icon.layoutParams = iconParams
        icon.scaleType = ImageView.ScaleType.FIT_CENTER
        container.addView(icon)

        container.isClickable = true
        container.isFocusable = true

        // Ripple-like press feedback
        container.foreground = GradientDrawable().apply {
            shape = GradientDrawable.OVAL
            setColor(Color.TRANSPARENT)
        }

        return container
    }

    private fun dpToPx(dp: Int): Int {
        return (dp * context.resources.displayMetrics.density + 0.5f).toInt()
    }

    companion object {
        private const val TAG = "ClipSync/Overlay"
    }
}
