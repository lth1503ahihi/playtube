use jni::objects::{JByteArray, JByteBuffer, JClass, JLongArray, JString};
use jni::sys::{jboolean, jfloat, jint, jlong, jlongArray, jobjectArray, jstring, JNI_TRUE};
use jni::JNIEnv;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

mod dsp;
mod hrtf;
pub mod lyrics;
pub mod sponsorblock;

static NEXT_HANDLE: AtomicI64 = AtomicI64::new(1);

fn next_handle_id() -> jlong {
    NEXT_HANDLE.fetch_add(1, Ordering::Relaxed)
}

fn spatial_registry() -> &'static Mutex<HashMap<jlong, Arc<Mutex<SpatialEngine>>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<jlong, Arc<Mutex<SpatialEngine>>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lyrics_registry() -> &'static Mutex<HashMap<jlong, Arc<lyrics::LrcTimeline>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<jlong, Arc<lyrics::LrcTimeline>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn sponsor_registry() -> &'static Mutex<HashMap<jlong, Arc<RwLock<sponsorblock::SponsorTimeline>>>> {
    static REGISTRY: OnceLock<Mutex<HashMap<jlong, Arc<RwLock<sponsorblock::SponsorTimeline>>>>> =
        OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

const TAPS_L_MS: [f32; 4] = [8.3, 14.9, 22.1, 33.7];
const TAPS_R_MS: [f32; 4] = [10.1, 17.3, 26.9, 38.9];
const TAP_GAINS: [f32; 4] = [0.30, 0.22, 0.16, 0.11];
const BASS_GAIN: f32 = 0.78;

#[inline(always)]
fn one_pole(rate: i32, hz: f64) -> f32 {
    (-2.0 * (PI as f64) * hz / (rate as f64)).exp() as f32
}

#[inline(always)]
fn soft(x: f32) -> f32 {
    let a = x.abs();
    if a <= 0.9 {
        x
    } else {
        let over = a - 0.9;
        let y = 0.9 + over / (1.0 + over * 10.0);
        if x < 0.0 { -y } else { y }
    }
}

#[inline(always)]
fn dot_product_f32(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    #[cfg(target_arch = "aarch64")]
    {
        // Safety: aarch64 always has NEON; lengths are asserted equal.
        unsafe { dot_product_neon(a, b) }
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let mut sum = 0.0f32;
        for i in 0..a.len() {
            sum += a[i] * b[i];
        }
        sum
    }
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn dot_product_neon(a: &[f32], b: &[f32]) -> f32 {
    use std::arch::aarch64::*;
    let len = a.len();
    let mut i = 0usize;
    let mut acc = vdupq_n_f32(0.0);
    let pa = a.as_ptr();
    let pb = b.as_ptr();
    while i + 4 <= len {
        let va = vld1q_f32(pa.add(i));
        let vb = vld1q_f32(pb.add(i));
        acc = vfmaq_f32(acc, va, vb);
        i += 4;
    }
    let mut sum = vaddvq_f32(acc);
    while i < len {
        sum += *pa.add(i) * *pb.add(i);
        i += 1;
    }
    sum
}

/// Linearly resamples a 48 kHz impulse response to `target_rate` once at initialization.
fn resample_ir(src: &[f32], src_rate: f64, target_rate: f64) -> Vec<f32> {
    if src.is_empty() {
        return Vec::new();
    }
    let ratio = target_rate / src_rate;
    let out_len = ((src.len() as f64) * ratio).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(out_len);
    let scale = (1.0 / ratio) as f32;
    for i in 0..out_len {
        let pos = (i as f64) / ratio;
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let s0 = src[idx.min(src.len() - 1)];
        let s1 = src[(idx + 1).min(src.len() - 1)];
        out.push((s0 + (s1 - s0) * frac) * scale);
    }
    out
}

pub struct SpatialEngine {
    #[allow(dead_code)]
    rate: i32,
    glide: f32,
    bass_coef: f32,
    low_coef: f32,
    room_coef: f32,
    mid_taps_rev: Vec<f32>,
    side_taps_rev: Vec<f32>,
    mid_ring: Vec<f32>,
    side_ring: Vec<f32>,
    ring_pos: usize,
    history: Vec<f32>,
    history_pos: usize,
    taps_l: [usize; 4],
    taps_r: [usize; 4],
    side_low: f32,
    bass_low: f32,
    room_low: f32,
    wet: f32,
    bass_wet: f32,
    harmonic_bass: dsp::HarmonicBass,
    bs2b: dsp::Bs2bFilter,
    limiter: dsp::FastLimiter,
}

impl SpatialEngine {
    pub fn new(sample_rate: i32) -> Self {
        let rate = if sample_rate <= 0 {
            48000
        } else {
            sample_rate.clamp(8000, 192000)
        };

        let (mut mid_taps_rev, mut side_taps_rev) = if rate == 44100 {
            (hrtf::MID_44100.to_vec(), hrtf::SIDE_44100.to_vec())
        } else if rate == 48000 {
            (hrtf::MID_48000.to_vec(), hrtf::SIDE_48000.to_vec())
        } else {
            (
                resample_ir(&hrtf::MID_48000, 48000.0, rate as f64),
                resample_ir(&hrtf::SIDE_48000, 48000.0, rate as f64),
            )
        };
        // Reverse once so FIR convolution reads forward-contiguous slices
        mid_taps_rev.reverse();
        side_taps_rev.reverse();

        let glide = 1.0 - (-1.0 / (0.05 * (rate as f64))).exp() as f32;
        let bass_coef = one_pole(rate, 200.0);
        let low_coef = one_pole(rate, 110.0);
        let room_coef = one_pole(rate, 4000.0);

        let mut taps_l = [0usize; 4];
        let mut taps_r = [0usize; 4];
        for i in 0..4 {
            taps_l[i] = ((TAPS_L_MS[i] / 1000.0) * (rate as f32)).round().max(1.0) as usize;
            taps_r[i] = ((TAPS_R_MS[i] / 1000.0) * (rate as f32)).round().max(1.0) as usize;
        }

        let max_tap = taps_r[3];
        let history = vec![0.0f32; max_tap + 1];

        let taps_len = mid_taps_rev.len();
        let mid_ring = vec![0.0f32; 2 * taps_len];
        let side_ring = vec![0.0f32; 2 * taps_len];

        let harmonic_bass = dsp::HarmonicBass::new(rate);
        let bs2b = dsp::Bs2bFilter::new(rate);
        let limiter = dsp::FastLimiter::new(rate);

        Self {
            rate,
            glide,
            bass_coef,
            low_coef,
            room_coef,
            mid_taps_rev,
            side_taps_rev,
            mid_ring,
            side_ring,
            ring_pos: 0,
            history,
            history_pos: 0,
            taps_l,
            taps_r,
            side_low: 0.0,
            bass_low: 0.0,
            room_low: 0.0,
            wet: 0.0,
            bass_wet: 0.0,
            harmonic_bass,
            bs2b,
            limiter,
        }
    }

    pub fn reset(&mut self) {
        self.wet = 0.0;
        self.bass_wet = 0.0;
        self.side_low = 0.0;
        self.bass_low = 0.0;
        self.room_low = 0.0;
        self.mid_ring.fill(0.0);
        self.side_ring.fill(0.0);
        self.history.fill(0.0);
        self.ring_pos = 0;
        self.history_pos = 0;
        self.harmonic_bass.reset();
        self.bs2b.reset();
        self.limiter.reset();
    }

    pub fn process_frame(&mut self, l: f32, r: f32, goal_amount: f32, goal_bass: f32) -> (f32, f32) {
        self.wet += (goal_amount - self.wet) * self.glide;
        self.bass_wet += (goal_bass - self.bass_wet) * self.glide;
        let w = self.wet;
        let b = self.bass_wet;

        // Wider above bass
        let mid_dry = (l + r) * 0.5;
        let side_dry = (l - r) * 0.5;
        self.side_low = side_dry + (self.side_low - side_dry) * self.bass_coef;
        let mut wide = self.side_low + (side_dry - self.side_low) * (1.0 + 0.9 * w);
        let mut mid = mid_dry;

        let taps = self.mid_taps_rev.len();
        self.mid_ring[self.ring_pos] = mid;
        self.mid_ring[self.ring_pos + taps] = mid;
        self.side_ring[self.ring_pos] = wide;
        self.side_ring[self.ring_pos + taps] = wide;

        let head = (2.0 * w).min(1.0);
        if head > 0.0 && taps > 0 {
            let start = self.ring_pos + 1;
            let end = start + taps;
            // Contiguous forward-slice FIR convolution (explicit NEON on aarch64)
            let hm = dot_product_f32(&self.mid_taps_rev, &self.mid_ring[start..end]);
            let hs = dot_product_f32(&self.side_taps_rev, &self.side_ring[start..end]);
            mid += (hm - mid) * head;
            wide += (hs - wide) * head;
        }

        self.ring_pos = if self.ring_pos + 1 == taps { 0 } else { self.ring_pos + 1 };

        // Cinema bass + Psychoacoustic harmonic excitation
        self.bass_low = mid + (self.bass_low - mid) * self.low_coef;
        mid += self.bass_low * BASS_GAIN * b;
        mid = self.harmonic_bass.process(mid, b);

        // Room reflections
        self.room_low = mid + (self.room_low - mid) * self.room_coef;
        self.history[self.history_pos] = self.room_low;

        let h_len = self.history.len();
        let mut room_l = 0.0f32;
        let mut room_r = 0.0f32;
        for t in 0..4 {
            let idx_l = (self.history_pos + h_len - (self.taps_l[t] % h_len)) % h_len;
            let idx_r = (self.history_pos + h_len - (self.taps_r[t] % h_len)) % h_len;
            room_l += self.history[idx_l] * TAP_GAINS[t];
            room_r += self.history[idx_r] * TAP_GAINS[t];
        }

        self.history_pos = if self.history_pos + 1 == h_len { 0 } else { self.history_pos + 1 };

        let level = 1.0 / (1.0 + 0.3 * w + 0.3 * b);
        let out_l_raw = soft((mid + wide + room_l * 0.6 * w) * level);
        let out_r_raw = soft((mid - wide + room_r * 0.6 * w) * level);

        // BS2B Binaural crossfeed stage (subtle speaker staging)
        let (cross_l, cross_r) = self.bs2b.process(out_l_raw, out_r_raw, (0.5 * w).min(0.8));

        // Fast peak limiter (zero distortion across all devices)
        self.limiter.process(cross_l, cross_r)
    }
}

// ----------------------------------------------------------------------------- JNI Exports

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_isAvailable(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    JNI_TRUE
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_getVersion<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    let ver = if cfg!(target_arch = "aarch64") {
        "PlayTube Rust Core v1.1.0 (ARM64 NEON)"
    } else if cfg!(target_arch = "arm") {
        "PlayTube Rust Core v1.1.0 (ARMv7)"
    } else {
        "PlayTube Rust Core v1.1.0"
    };
    match env.new_string(ver) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_createSpatialEngine(
    _env: JNIEnv,
    _class: JClass,
    sample_rate: jint,
) -> jlong {
    let id = next_handle_id();
    let engine = Arc::new(Mutex::new(SpatialEngine::new(sample_rate)));
    if let Ok(mut map) = spatial_registry().lock() {
        map.insert(id, engine);
        id
    } else {
        0
    }
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_destroySpatialEngine(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle > 0 {
        if let Ok(mut map) = spatial_registry().lock() {
            map.remove(&handle);
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_resetSpatialEngine(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle <= 0 {
        return;
    }
    let engine_arc = spatial_registry()
        .lock()
        .ok()
        .and_then(|map| map.get(&handle).cloned());
    if let Some(arc) = engine_arc {
        if let Ok(mut engine) = arc.lock() {
            engine.reset();
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_processPcm16(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    input_buf: JByteBuffer,
    output_buf: JByteBuffer,
    byte_count: jint,
    goal_amount: jfloat,
    goal_bass: jfloat,
) -> jint {
    if handle <= 0 || byte_count <= 0 {
        return 0;
    }

    let usable_bytes = (byte_count as usize) & !3; // stereo 16-bit = 4 bytes/frame
    if usable_bytes == 0 {
        return 0;
    }

    let in_cap = match env.get_direct_buffer_capacity(&input_buf) {
        Ok(c) => c,
        Err(_) => return -1,
    };
    let out_cap = match env.get_direct_buffer_capacity(&output_buf) {
        Ok(c) => c,
        Err(_) => return -1,
    };
    if in_cap < usable_bytes || out_cap < usable_bytes {
        return -1;
    }

    let (in_ptr, out_ptr) = match (
        env.get_direct_buffer_address(&input_buf),
        env.get_direct_buffer_address(&output_buf),
    ) {
        (Ok(inp), Ok(outp)) => (inp, outp),
        _ => return -1,
    };

    let engine_arc = spatial_registry()
        .lock()
        .ok()
        .and_then(|map| map.get(&handle).cloned());
    let arc = match engine_arc {
        Some(a) => a,
        None => return -1,
    };
    let mut engine = match arc.lock() {
        Ok(g) => g,
        Err(_) => return -1,
    };

    let num_frames = usable_bytes / 4;
    let amount = if goal_amount.is_finite() { goal_amount.clamp(0.0, 1.5) } else { 0.0 };
    let bass = if goal_bass.is_finite() { goal_bass.clamp(0.0, 1.5) } else { 0.0 };

    unsafe {
        let in_i16 = in_ptr as *const i16;
        let out_i16 = out_ptr as *mut i16;

        for i in 0..num_frames {
            let l_raw = *in_i16.add(i * 2) as f32 / 32768.0;
            let r_raw = *in_i16.add(i * 2 + 1) as f32 / 32768.0;

            let (out_l, out_r) = engine.process_frame(l_raw, r_raw, amount, bass);

            let l_i16 = (out_l * 32767.0).clamp(-32768.0, 32767.0) as i16;
            let r_i16 = (out_r * 32767.0).clamp(-32768.0, 32767.0) as i16;

            *out_i16.add(i * 2) = l_i16;
            *out_i16.add(i * 2 + 1) = r_i16;
        }
    }

    usable_bytes as jint
}


// ----------------------------------------------------------------------------- Lyrics JNI

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_parseLrcTimeline<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    lrc_string: JString<'local>,
) -> jlong {
    let lrc_rust: String = match env.get_string(&lrc_string) {
        Ok(s) => s.into(),
        Err(_) => return 0,
    };

    let id = next_handle_id();
    let timeline = Arc::new(lyrics::LrcTimeline::parse(&lrc_rust));
    if let Ok(mut map) = lyrics_registry().lock() {
        map.insert(id, timeline);
        id
    } else {
        0
    }
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_destroyLrcTimeline(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle > 0 {
        if let Ok(mut map) = lyrics_registry().lock() {
            map.remove(&handle);
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_findActiveLrcIndex(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    pos_ms: jlong,
) -> jint {
    if handle <= 0 || pos_ms < 0 {
        return -1;
    }
    let timeline = lyrics_registry()
        .lock()
        .ok()
        .and_then(|map| map.get(&handle).cloned());
    match timeline {
        Some(t) => t.find_active_index(pos_ms as u64),
        None => -1,
    }
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_getLrcTimestamps<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> jlongArray {
    if handle <= 0 {
        return std::ptr::null_mut();
    }
    let timeline = match lyrics_registry()
        .lock()
        .ok()
        .and_then(|map| map.get(&handle).cloned())
    {
        Some(t) => t,
        None => return std::ptr::null_mut(),
    };
    let times: Vec<i64> = timeline.lines.iter().map(|l| l.time_ms as i64).collect();

    match env.new_long_array(times.len() as i32) {
        Ok(arr) => {
            let _ = env.set_long_array_region(&arr, 0, &times);
            arr.into_raw()
        }
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_getLrcLines<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> jobjectArray {
    if handle <= 0 {
        return std::ptr::null_mut();
    }
    let timeline = match lyrics_registry()
        .lock()
        .ok()
        .and_then(|map| map.get(&handle).cloned())
    {
        Some(t) => t,
        None => return std::ptr::null_mut(),
    };

    let string_class = match env.find_class("java/lang/String") {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };

    let empty_str = match env.new_string("") {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };

    let arr = match env.new_object_array(timeline.lines.len() as i32, &string_class, &empty_str) {
        Ok(a) => a,
        Err(_) => return std::ptr::null_mut(),
    };

    for (i, line) in timeline.lines.iter().enumerate() {
        if let Ok(js) = env.new_string(&line.text) {
            let _ = env.set_object_array_element(&arr, i as i32, &js);
        }
    }

    arr.into_raw()
}

// ----------------------------------------------------------------------- SponsorBlock JNI

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_createSponsorTimeline(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    let id = next_handle_id();
    let timeline = Arc::new(RwLock::new(sponsorblock::SponsorTimeline::new()));
    if let Ok(mut map) = sponsor_registry().lock() {
        map.insert(id, timeline);
        id
    } else {
        0
    }
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_destroySponsorTimeline(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle > 0 {
        if let Ok(mut map) = sponsor_registry().lock() {
            map.remove(&handle);
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_setSponsorSegments(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    starts: JLongArray,
    ends: JLongArray,
    categories: JByteArray,
    actions: JByteArray,
) {
    if handle <= 0 {
        return;
    }

    let (len_s, len_e, len_c, len_a) = match (
        env.get_array_length(&starts),
        env.get_array_length(&ends),
        env.get_array_length(&categories),
        env.get_array_length(&actions),
    ) {
        (Ok(s), Ok(e), Ok(c), Ok(a)) if s >= 0 && e >= 0 && c >= 0 && a >= 0 => {
            (s as usize, e as usize, c as usize, a as usize)
        }
        _ => return,
    };

    let len = len_s.min(len_e).min(len_c).min(len_a);
    let mut segments = Vec::with_capacity(len);

    if len > 0 {
        let mut start_vec = vec![0i64; len];
        let mut end_vec = vec![0i64; len];
        let mut cat_vec = vec![0i8; len];
        let mut act_vec = vec![0i8; len];

        if env.get_long_array_region(&starts, 0, &mut start_vec).is_err()
            || env.get_long_array_region(&ends, 0, &mut end_vec).is_err()
            || env.get_byte_array_region(&categories, 0, &mut cat_vec).is_err()
            || env.get_byte_array_region(&actions, 0, &mut act_vec).is_err()
        {
            return;
        }

        for i in 0..len {
            if start_vec[i] < 0 || end_vec[i] <= start_vec[i] {
                continue;
            }
            segments.push(sponsorblock::SponsorSegment {
                start_ms: start_vec[i] as u64,
                end_ms: end_vec[i] as u64,
                category: cat_vec[i] as u8,
                action: act_vec[i] as u8,
            });
        }
    }

    let timeline_arc = sponsor_registry()
        .lock()
        .ok()
        .and_then(|map| map.get(&handle).cloned());
    if let Some(arc) = timeline_arc {
        if let Ok(mut timeline) = arc.write() {
            timeline.set_segments(segments);
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_checkSponsorPosition(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    pos_ms: jlong,
) -> jlong {
    if handle <= 0 || pos_ms < 0 {
        return 0;
    }
    let timeline_arc = sponsor_registry()
        .lock()
        .ok()
        .and_then(|map| map.get(&handle).cloned());
    let arc = match timeline_arc {
        Some(a) => a,
        None => return 0,
    };
    let timeline = match arc.read() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    match timeline.check_position(pos_ms as u64) {
        Some((target_ms, _cat, _act)) => target_ms as jlong,
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_engine_sample_rates_and_reset() {
        for rate in [0, -1, 32000, 44100, 48000, 96000] {
            let mut engine = SpatialEngine::new(rate);
            let (l, r) = engine.process_frame(0.5, -0.5, 0.8, 0.5);
            assert!(l.is_finite() && r.is_finite());
            engine.reset();
            let (l2, r2) = engine.process_frame(0.0, 0.0, 0.0, 0.0);
            assert_eq!(l2, 0.0);
            assert_eq!(r2, 0.0);
        }
    }

    #[test]
    fn test_dot_product_f32() {
        let a = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let b = [0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5];
        let got = dot_product_f32(&a, &b);
        let expected: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        assert!((got - expected).abs() < 1e-5);
    }
}
