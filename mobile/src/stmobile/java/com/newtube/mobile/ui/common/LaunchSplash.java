package com.newtube.mobile.ui.common;

import android.app.Activity;
import android.graphics.Color;
import android.graphics.Typeface;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.animation.AccelerateDecelerateInterpolator;
import android.view.animation.DecelerateInterpolator;
import android.view.animation.OvershootInterpolator;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;

import androidx.core.content.ContextCompat;

import com.liskovsoft.smartyoutubetv2.tv.R;

/**
 * PLAYTUBE(launch): modern animated splash logo shown smoothly once on app start.
 * Features an animated scaling heart-play logo with a pulsing red aura, heartbeat rhythm, and title fade.
 */
public final class LaunchSplash {
    private static final long HOLD_MS = 1050;
    private static final long FADE_MS = 320;
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

        // Center container
        LinearLayout centerContainer = new LinearLayout(activity);
        centerContainer.setOrientation(LinearLayout.VERTICAL);
        centerContainer.setGravity(Gravity.CENTER);

        // Pulsing glow aura behind icon
        View glow = new View(activity);
        int glowSize = (int) (140 * density);
        FrameLayout.LayoutParams glowLp = new FrameLayout.LayoutParams(glowSize, glowSize, Gravity.CENTER);
        android.graphics.drawable.GradientDrawable glowDrawable = new android.graphics.drawable.GradientDrawable();
        glowDrawable.setShape(android.graphics.drawable.GradientDrawable.OVAL);
        glowDrawable.setColor(Color.parseColor("#44FF0033"));
        glow.setBackground(glowDrawable);
        glow.setScaleX(0.4f);
        glow.setScaleY(0.4f);
        glow.setAlpha(0f);

        // Logo Icon wrapper
        FrameLayout iconWrapper = new FrameLayout(activity);
        int iconSize = (int) (88 * density);
        LinearLayout.LayoutParams wrapperLp = new LinearLayout.LayoutParams(glowSize, glowSize);
        wrapperLp.gravity = Gravity.CENTER_HORIZONTAL;
        iconWrapper.setLayoutParams(wrapperLp);

        ImageView icon = new ImageView(activity);
        icon.setScaleType(ImageView.ScaleType.FIT_CENTER);
        icon.setImageResource(R.drawable.ic_launcher_default);
        FrameLayout.LayoutParams iconLp = new FrameLayout.LayoutParams(iconSize, iconSize, Gravity.CENTER);

        iconWrapper.addView(glow, glowLp);
        iconWrapper.addView(icon, iconLp);

        // App Title "PlayTube"
        TextView title = new TextView(activity);
        title.setText("PlayTube");
        title.setTextColor(Color.WHITE);
        title.setTextSize(26);
        title.setTypeface(Typeface.create("sans-serif-medium", Typeface.BOLD));
        title.setLetterSpacing(0.06f);
        LinearLayout.LayoutParams titleLp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        titleLp.topMargin = (int) (6 * density);
        titleLp.gravity = Gravity.CENTER_HORIZONTAL;
        title.setLayoutParams(titleLp);
        title.setAlpha(0f);
        title.setTranslationY(16 * density);

        centerContainer.addView(iconWrapper);
        centerContainer.addView(title);

        FrameLayout.LayoutParams containerLp = new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.CENTER);
        splash.addView(centerContainer, containerLp);

        ((ViewGroup) activity.getWindow().getDecorView()).addView(splash,
                new ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));

        // Animated entrance: pop-in with overshoot
        icon.setScaleX(0.5f);
        icon.setScaleY(0.5f);
        icon.setAlpha(0f);

        icon.animate()
                .scaleX(1.0f)
                .scaleY(1.0f)
                .alpha(1.0f)
                .setDuration(460)
                .setInterpolator(new OvershootInterpolator(1.4f))
                .withEndAction(() -> {
                    // Heartbeat pulse effect
                    icon.animate()
                            .scaleX(1.08f)
                            .scaleY(1.08f)
                            .setDuration(240)
                            .setInterpolator(new AccelerateDecelerateInterpolator())
                            .withEndAction(() -> {
                                icon.animate()
                                        .scaleX(1.0f)
                                        .scaleY(1.0f)
                                        .setDuration(240)
                                        .setInterpolator(new AccelerateDecelerateInterpolator())
                                        .start();
                            })
                            .start();
                })
                .start();

        glow.animate()
                .scaleX(1.7f)
                .scaleY(1.7f)
                .alpha(0.7f)
                .setDuration(520)
                .setInterpolator(new DecelerateInterpolator())
                .withEndAction(() -> {
                    glow.animate()
                            .scaleX(1.4f)
                            .scaleY(1.4f)
                            .alpha(0.4f)
                            .setDuration(400)
                            .start();
                })
                .start();

        title.animate()
                .alpha(1.0f)
                .translationY(0)
                .setStartDelay(180)
                .setDuration(420)
                .setInterpolator(new DecelerateInterpolator())
                .start();

        // Fade out smoothly into Home
        splash.animate()
                .alpha(0f)
                .setStartDelay(HOLD_MS)
                .setDuration(FADE_MS)
                .setInterpolator(new DecelerateInterpolator())
                .withEndAction(() -> {
                    if (splash.getParent() instanceof ViewGroup) {
                        ((ViewGroup) splash.getParent()).removeView(splash);
                    }
                });
    }
}
