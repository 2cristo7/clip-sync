# ============================================================
# ClipSync ProGuard / R8 rules
# ============================================================

# --- OkHttp / Okio ---
-dontwarn okhttp3.**
-dontwarn okio.**
-dontwarn org.conscrypt.**
-dontwarn org.bouncycastle.**
-dontwarn org.openjsse.**
-keep class okhttp3.internal.platform.** { *; }
-keepnames class okhttp3.internal.publicsuffix.PublicSuffixDatabase

# Keep OkHttp platform adapters (Conscrypt, OpenJSSE, BouncyCastle)
-keep class okhttp3.internal.platform.ConscryptPlatform { *; }
-keep class okhttp3.internal.platform.OpenJSSEPlatform { *; }
-keep class okhttp3.internal.platform.BouncyCastlePlatform { *; }

# --- Kotlin metadata ---
-keep class kotlin.Metadata { *; }

# --- Kotlin serialization (if used) ---
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.AnnotationsKt
-keepclassmembers class kotlinx.serialization.json.** { *; }
-keep,includedescriptorclasses class com.clipsync.**$$serializer { *; }
-keepclassmembers class com.clipsync.** {
    *** Companion;
}
-keepclasseswithmembers class com.clipsync.** {
    kotlinx.serialization.KSerializer serializer(...);
}

# --- Jetpack Compose (safety rules, mostly handled by AGP) ---
-dontwarn androidx.compose.**
-keep class androidx.compose.runtime.** { *; }
-keep class androidx.compose.ui.tooling.** { *; }

# --- Keep ClipSync service and component declarations ---
-keep class com.clipsync.** extends android.app.Service { *; }
-keep class com.clipsync.** extends android.app.Activity { *; }
-keep class com.clipsync.** extends android.content.BroadcastReceiver { *; }

# --- General Android ---
-keepattributes Signature
-keepattributes Exceptions
-keepattributes SourceFile,LineNumberTable
