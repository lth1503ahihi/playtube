package com.newtube.mobile.ui.meetune;

import android.util.Log;
import java.nio.ByteBuffer;

/**
 * Native bridge to the PlayTube Rust DSP Core (libplaytube_core.so).
 * Accelerates 3D Spatial Audio, MIT KEMAR HRTF FIR convolution, and EQ filtering
 * using ARM NEON SIMD vectorization.
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
            Log.w(TAG, "Rust native library not available, falling back to Java DSP: " + t.getMessage());
            sLoaded = false;
        }
    }

    private PlayTuneNative() {
    }

    public static boolean isLoaded() {
        return sLoaded;
    }

    public static native boolean isAvailable();
    public static native String getVersion();
    public static native long createSpatialEngine(int sampleRate);
    public static native void destroySpatialEngine(long handle);
    public static native void resetSpatialEngine(long handle);
    public static native int processPcm16(long handle, ByteBuffer inBuf, ByteBuffer outBuf, int byteCount, float goalAmount, float goalBass);
}
