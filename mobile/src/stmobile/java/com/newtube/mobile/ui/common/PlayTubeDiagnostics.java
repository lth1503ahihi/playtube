package com.newtube.mobile.ui.common;

import android.app.ActivityManager;
import android.content.ComponentCallbacks2;
import android.content.Context;
import android.os.Debug;
import android.util.Log;

import androidx.annotation.NonNull;
import androidx.annotation.Nullable;

import com.bumptech.glide.Glide;
import com.newtube.mobile.ui.meetune.PlayTuneNative;

import java.util.Locale;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * PlayTube Unified Memory Budget, Pressure Handler & Diagnostics Telemetry.
 * Coordinates Java/Glide caches and Rust Native Core telemetry under a single budget.
 */
public final class PlayTubeDiagnostics {
    private static final String TAG = "PlayTubeDiagnostics";
    private static final AtomicInteger sTrimCount = new AtomicInteger(0);
    private static volatile int sLastTrimLevel = 0;

    private PlayTubeDiagnostics() {
    }

    /**
     * Handles Android memory pressure signals across Glide/bitmap caches and logs telemetry.
     */
    public static void handleTrimMemory(@NonNull Context context, int level) {
        sTrimCount.incrementAndGet();
        sLastTrimLevel = level;
        Context app = context.getApplicationContext();

        try {
            Glide.get(app).trimMemory(level);
            if (level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_CRITICAL
                    || level >= ComponentCallbacks2.TRIM_MEMORY_COMPLETE) {
                Glide.get(app).clearMemory();
            }
        } catch (Throwable t) {
            Log.w(TAG, "Glide trimMemory failed: " + t.getMessage());
        }

        if (level >= ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW) {
            Log.i(TAG, snapshot(app, "TRIM_LEVEL_" + level, "N/A", "N/A", "N/A"));
        }
    }

    /**
     * Returns an adaptive memory budget in MB for caches based on device memory class.
     */
    public static int adaptiveCacheBudgetMb(@NonNull Context context) {
        ActivityManager am = (ActivityManager) context.getSystemService(Context.ACTIVITY_SERVICE);
        if (am == null) {
            return 64;
        }
        int memClass = am.getMemoryClass();
        return am.isLowRamDevice() ? Math.max(24, memClass / 6) : Math.max(48, memClass / 4);
    }

    /**
     * Formats a unified diagnostics snapshot covering Playback, Java/Native/PSS Memory, and Rust Core.
     */
    @NonNull
    public static String snapshot(
            @NonNull Context context,
            @Nullable String playbackState,
            @Nullable String streamType,
            @Nullable String decoder,
            @Nullable String networkState) {
        Runtime rt = Runtime.getRuntime();
        long javaUsedMb = (rt.totalMemory() - rt.freeMemory()) / (1024L * 1024L);
        long javaMaxMb = rt.maxMemory() / (1024L * 1024L);
        long nativeHeapMb = Debug.getNativeHeapAllocatedSize() / (1024L * 1024L);

        Debug.MemoryInfo memInfo = new Debug.MemoryInfo();
        Debug.getMemoryInfo(memInfo);
        long pssMb = memInfo.getTotalPss() / 1024L;

        long spatialHandles = 0;
        long lyricsHandles = 0;
        long sponsorHandles = 0;
        long pcmFrames = 0;
        long jniErrors = 0;
        boolean nativeOk = PlayTuneNative.isLoaded();
        if (nativeOk) {
            try {
                long[] stats = PlayTuneNative.getNativeStats();
                if (stats != null && stats.length >= 5) {
                    spatialHandles = stats[0];
                    lyricsHandles = stats[1];
                    sponsorHandles = stats[2];
                    pcmFrames = stats[3];
                    jniErrors = stats[4];
                }
            } catch (Throwable ignored) {
                nativeOk = false;
            }
        }

        return String.format(
                Locale.US,
                "[PlayTube Diagnostics] Playback State: %s | Stream Type: %s | Decoder: %s | Network: %s | "
                        + "Memory: Java Heap: %d/%d MB, Native Heap: %d MB, PSS: %d MB, Budget: %d MB, Trims: %d (last=%d) | "
                        + "Native: JNI=%s, SpatialHandles=%d, LyricsHandles=%d, SponsorHandles=%d, PcmFrames=%d, JniErrors=%d",
                playbackState != null ? playbackState : "IDLE",
                streamType != null ? streamType : "UNKNOWN",
                decoder != null ? decoder : "MediaCodec",
                networkState != null ? networkState : "UNKNOWN",
                javaUsedMb, javaMaxMb, nativeHeapMb, pssMb,
                adaptiveCacheBudgetMb(context), sTrimCount.get(), sLastTrimLevel,
                nativeOk ? "OK" : "UNAVAILABLE",
                spatialHandles, lyricsHandles, sponsorHandles, pcmFrames, jniErrors
        );
    }
}
