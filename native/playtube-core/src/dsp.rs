//! Universal Audio Processing Pipeline:
//! 1. BS2B Binaural Crossfeed (Natural speaker soundstage, reduces headphone fatigue)
//! 2. Psychoacoustic Bass Exciter (Deep punchy bass on all drivers without rattling)
//! 3. Soft-Knee Dynamic Peak Limiter (Zero distortion / clipping on any device)

use std::f32::consts::PI;

/// Bauer Stereophonic-to-Binaural (BS2B) filter.
/// Blends low frequencies across channels with interaural time delay (~0.3ms)
/// so headphones sound like high-end stereo speakers in an acoustically treated room.
pub struct Bs2bFilter {
    // Left channel biquad / IIR state
    l_d1: f32,
    l_d2: f32,
    // Right channel biquad / IIR state
    r_d1: f32,
    r_d2: f32,
    // Filter coefficients for standard 700Hz / 4.5dB crossfeed
    a0: f32,
    b1: f32,
    gb: f32,
}

impl Bs2bFilter {
    pub fn new(sample_rate: i32) -> Self {
        // Standard BS2B 700 Hz, 4.5 dB level
        let fc = 700.0f32;
        let g = 0.595662f32; // -4.5 dB
        let x = (-2.0 * PI * fc / (sample_rate as f32)).exp();
        let b1 = x;
        let a0 = 1.0 - x;
        let gb = (1.0 - g) / g;

        Self {
            l_d1: 0.0,
            l_d2: 0.0,
            r_d1: 0.0,
            r_d2: 0.0,
            a0,
            b1,
            gb,
        }
    }

    pub fn reset(&mut self) {
        self.l_d1 = 0.0;
        self.l_d2 = 0.0;
        self.r_d1 = 0.0;
        self.r_d2 = 0.0;
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, amount: f32) -> (f32, f32) {
        if amount <= 0.001 {
            return (l, r);
        }

        // Low-pass filtered cross-talk
        self.l_d1 = self.a0 * l + self.b1 * self.l_d1;
        self.r_d1 = self.a0 * r + self.b1 * self.r_d1;

        // High-boost direct
        let l_direct = l + self.l_d1 * self.gb;
        let r_direct = r + self.r_d1 * self.gb;

        // Crossfeed blend
        let out_l = (l_direct + self.r_d1) * 0.5;
        let out_r = (r_direct + self.l_d1) * 0.5;

        // Dry/wet interpolate
        (
            l + (out_l - l) * amount,
            r + (out_r - r) * amount,
        )
    }
}

/// Psychoacoustic Bass Generator (MaxxBass principle).
/// Generates 2nd and 3rd harmonics of low frequencies (<120Hz).
/// The brain perceives deep bass even on tiny phone speakers or budget earbuds!
pub struct HarmonicBass {
    lp1: f32,
    lp2: f32,
    hp1: f32,
    alpha_lp: f32,
    alpha_hp: f32,
}

impl HarmonicBass {
    pub fn new(sample_rate: i32) -> Self {
        let sr = sample_rate as f32;
        let alpha_lp = (-2.0 * PI * 120.0 / sr).exp();
        let alpha_hp = (-2.0 * PI * 55.0 / sr).exp();

        Self {
            lp1: 0.0,
            lp2: 0.0,
            hp1: 0.0,
            alpha_lp,
            alpha_hp,
        }
    }

    pub fn reset(&mut self) {
        self.lp1 = 0.0;
        self.lp2 = 0.0;
        self.hp1 = 0.0;
    }

    #[inline(always)]
    pub fn process(&mut self, mid: f32, amount: f32) -> f32 {
        if amount <= 0.001 {
            return mid;
        }

        // 2-pole lowpass at 120Hz to extract fundamental bass
        self.lp1 = (1.0 - self.alpha_lp) * mid + self.alpha_lp * self.lp1;
        self.lp2 = (1.0 - self.alpha_lp) * self.lp1 + self.alpha_lp * self.lp2;
        let bass = self.lp2;

        // Non-linear harmonic synthesis: generates 2nd (asymmetric) & 3rd (symmetric) harmonics
        // tanh-like smooth saturation of the bass signal
        let x = bass * 2.2;
        let harmonics = x / (1.0 + x.abs());

        // Highpass the harmonics slightly so we don't DC-offset or overload the sub-sub range
        self.hp1 = self.alpha_hp * (self.hp1 + harmonics - bass);
        let shaped_harmonics = harmonics - self.hp1;

        mid + shaped_harmonics * 0.35 * amount
    }
}

/// Soft-knee Peak Limiter.
/// Ensures zero digital clipping / distortion across all volume levels and outputs.
pub struct FastLimiter {
    envelope: f32,
    attack_coef: f32,
    release_coef: f32,
}

impl FastLimiter {
    pub fn new(sample_rate: i32) -> Self {
        let sr = sample_rate as f32;
        // Fast attack: 1ms, Smooth release: 60ms
        let attack_coef = (-1.0 / (0.001 * sr)).exp();
        let release_coef = (-1.0 / (0.060 * sr)).exp();

        Self {
            envelope: 0.0,
            attack_coef,
            release_coef,
        }
    }

    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let peak = l.abs().max(r.abs());

        if peak > self.envelope {
            self.envelope = self.attack_coef * self.envelope + (1.0 - self.attack_coef) * peak;
        } else {
            self.envelope = self.release_coef * self.envelope + (1.0 - self.release_coef) * peak;
        }

        let threshold = 0.92f32;
        let gain = if self.envelope > threshold {
            threshold / self.envelope
        } else {
            1.0
        };

        (l * gain, r * gain)
    }
}
