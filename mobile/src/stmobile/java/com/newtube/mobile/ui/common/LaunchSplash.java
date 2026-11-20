package com.newtube.mobile.ui.common;

import android.app.Activity;
import android.view.Gravity;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.ImageView;

import androidx.core.content.ContextCompat;

import com.liskovsoft.smartyoutubetv2.tv.R;

/**
 * PLAYTUBE(launch): clean, minimal YouTube-style launch splash.
 * Shows the centered PlayTube icon over the app theme background and smoothly fades out
 * into the home feed once loaded, exactly matching YouTube's official launch experience.
 */
public final class LaunchSplash {
    private static final long HOLD_MS = 300;
    private static final long FADE_MS = 220;
    private static boolean sPlayed;

    private LaunchSplash() {
    }

    public static void play(Activity activity) {
        if (sPlayed || activity == null || activity.isFinishing() || activity.isDestroyed()) {
            return;
        }
        sPlayed = true;

        float density = activity.getResources().getDisplayMetrics().density;

        FrameLayout splash = new FrameLayout(activity);
        splash.setBackgroundColor(ContextCompat.getColor(activity, R.color.mobile_color_background));
        splash.setClickable(true);

        ImageView icon = new ImageView(activity);
        icon.setScaleType(ImageView.ScaleType.FIT_CENTER);
        icon.setImageResource(R.drawable.ic_launcher_default);

        int iconSize = (int) (72 * density);
        FrameLayout.LayoutParams iconLp = new FrameLayout.LayoutParams(iconSize, iconSize, Gravity.CENTER);
        splash.addView(icon, iconLp);

        ((ViewGroup) activity.getWindow().getDecorView()).addView(splash,
                new ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));

        // Smooth YouTube-style crossfade into Home
        splash.animate()
                .alpha(0f)
                .setStartDelay(HOLD_MS)
                .setDuration(FADE_MS)
                .setInterpolator(Motion.STANDARD)
                .withEndAction(() -> {
                    if (splash.getParent() instanceof ViewGroup) {
                        ((ViewGroup) splash.getParent()).removeView(splash);
                    }
                });
    }
}
