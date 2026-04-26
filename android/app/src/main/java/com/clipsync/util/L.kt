package com.clipsync.util

import android.util.Log

object L {
    private const val TAG = "ClipSync"

    fun action(module: String, msg: String) = Log.d(TAG, "👆 [$module] $msg")
    fun event(module: String, msg: String)  = Log.i(TAG, "⚡ [$module] $msg")
    fun perm(module: String, msg: String)   = Log.i(TAG, "🔐 [$module] $msg")
    fun verbose(module: String, msg: String)= Log.v(TAG, "📋 [$module] $msg")
    fun warn(module: String, msg: String)   = Log.w(TAG, "[$module] $msg")
    fun warn(module: String, msg: String, t: Throwable) = Log.w(TAG, "[$module] $msg", t)
    fun error(module: String, msg: String)  = Log.e(TAG, "[$module] $msg")
    fun error(module: String, msg: String, t: Throwable) = Log.e(TAG, "[$module] $msg", t)
}
