package com.newtube.mobile.ui.meetune;

import android.util.Log;
import java.nio.ByteBuffer;

/**
 * Native bridge to the PlayTube Rust Processing Core (libplaytube_core.so).
 * Accelerates:
 * 1. 3D Spatial Audio, MIT KEMAR HRTF FIR convolution, and EQ filtering using ARM SIMD vectorization.
 * 2. High-performance, zero-allocation LRC & Synced Lyrics parsing and O(log N) timeline lookup.
 * 3. SponsorBlock Interval Tree segment evaluation for instant zero-latency skipping.
 */
public final class PlayTuneNative {
    private static final String TAG = "PlayTuneNative";
    private static boolean sLoaded = false;

    static {
        try {
            System.loadLibrary("playtube_core");
            sLoaded = true;
            Log.i(TAG, "PlayTube Rust Core loaded successfully: " + getVersion());
        } catch (Throwable t) {
            Log.w(TAG, "Rust native library not available: " + t.getMessage());
            sLoaded = false;
        }
    }

    private PlayTuneNative() {
    }

    public static boolean isLoaded() {
        return sLoaded;
    }

    // Audio DSP Engine
    public static native boolean isAvailable();
    public static native String getVersion();
    public static native long createSpatialEngine(int sampleRate);
    public static native void destroySpatialEngine(long handle);
    public static native void resetSpatialEngine(long handle);
    public static native int processPcm16(long handle, ByteBuffer inBuf, ByteBuffer outBuf, int byteCount, float goalAmount, float goalBass);

    // Lyrics & Subtitle Engine
    public static native long parseLrcTimeline(String lrcContent);
    public static native void destroyLrcTimeline(long handle);
    public static native int findActiveLrcIndex(long handle, long posMs);
    public static native long[] getLrcTimestamps(long handle);
    public static native String[] getLrcLines(long handle);

    // SponsorBlock Segment Engine
    public static native long createSponsorTimeline();
    public static native void destroySponsorTimeline(long handle);
    public static native void setSponsorSegments(long handle, long[] starts, long[] ends, byte[] categories, byte[] actions);
    public static native long checkSponsorPosition(long handle, long posMs);
}
