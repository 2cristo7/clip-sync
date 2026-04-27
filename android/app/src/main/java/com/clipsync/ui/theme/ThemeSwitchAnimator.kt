package com.clipsync.ui.theme

import android.animation.Animator
import android.animation.AnimatorListenerAdapter
import android.animation.ValueAnimator
import android.app.Activity
import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Path
import android.view.View
import android.view.ViewGroup
import android.view.animation.DecelerateInterpolator
import kotlin.math.hypot
import kotlin.math.max

object ThemeSwitchAnimator {

    private var isAnimating = false

    fun animateThemeSwitch(
        activity: Activity,
        cx: Float,
        cy: Float,
        onMidpoint: () -> Unit,
        durationMs: Long = 1200L,
    ) {
        if (isAnimating) {
            onMidpoint()
            return
        }

        val decorView = activity.window.decorView as ViewGroup
        val rootView = decorView.findViewById<View>(android.R.id.content)

        val bitmap = try {
            val b = Bitmap.createBitmap(rootView.width, rootView.height, Bitmap.Config.ARGB_8888)
            rootView.draw(Canvas(b))
            b
        } catch (_: Exception) {
            onMidpoint()
            return
        }

        onMidpoint()

        val overlay = CircularRevealOverlayView(activity, bitmap, cx, cy)
        overlay.layoutParams = ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT,
        )
        decorView.addView(overlay)

        val maxRadius = hypot(
            max(cx, decorView.width - cx).toDouble(),
            max(cy, decorView.height - cy).toDouble(),
        ).toFloat()

        isAnimating = true

        ValueAnimator.ofFloat(0f, maxRadius).apply {
            duration = durationMs
            interpolator = DecelerateInterpolator(2f)
            addUpdateListener { overlay.revealRadius = it.animatedValue as Float }
            addListener(object : AnimatorListenerAdapter() {
                override fun onAnimationEnd(animation: Animator) {
                    decorView.removeView(overlay)
                    bitmap.recycle()
                    isAnimating = false
                }
            })
            decorView.post { start() }
        }
    }

    private class CircularRevealOverlayView(
        context: Context,
        private val bitmap: Bitmap,
        private val cx: Float,
        private val cy: Float,
    ) : View(context) {

        var revealRadius: Float = 0f
            set(value) {
                field = value
                invalidate()
            }

        private val clipPath = Path()

        override fun onDraw(canvas: Canvas) {
            clipPath.reset()
            clipPath.addRect(0f, 0f, width.toFloat(), height.toFloat(), Path.Direction.CW)
            clipPath.addCircle(cx, cy, revealRadius, Path.Direction.CCW)
            clipPath.fillType = Path.FillType.EVEN_ODD
            canvas.clipPath(clipPath)
            canvas.drawBitmap(bitmap, 0f, 0f, null)
        }
    }
}
