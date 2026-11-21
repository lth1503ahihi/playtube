use jni::objects::{JByteArray, JByteBuffer, JClass, JLongArray, JString};
use jni::sys::{jboolean, jfloat, jint, jlong, jlongArray, jobjectArray, jstring, JNI_TRUE};
use jni::JNIEnv;
use std::f32::consts::PI;

mod dsp;
mod hrtf;
pub mod lyrics;
pub mod sponsorblock;

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

pub struct SpatialEngine {
    #[allow(dead_code)]
    rate: i32,
    glide: f32,
    bass_coef: f32,
    low_coef: f32,
    room_coef: f32,
    mid_taps: &'static [f32],
    side_taps: &'static [f32],
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
    pub fn new(rate: i32) -> Self {
        let (mid_taps, side_taps) = if rate == 44100 {
            (hrtf::MID_44100.as_slice(), hrtf::SIDE_44100.as_slice())
        } else {
            (hrtf::MID_48000.as_slice(), hrtf::SIDE_48000.as_slice())
        };

        let glide = 1.0 - (-1.0 / (0.05 * (rate as f64))).exp() as f32;
        let bass_coef = one_pole(rate, 200.0);
        let low_coef = one_pole(rate, 110.0);
        let room_coef = one_pole(rate, 4000.0);

        let mut taps_l = [0usize; 4];
        let mut taps_r = [0usize; 4];
        for i in 0..4 {
            taps_l[i] = ((TAPS_L_MS[i] / 1000.0) * (rate as f32)).round() as usize;
            taps_r[i] = ((TAPS_R_MS[i] / 1000.0) * (rate as f32)).round() as usize;
        }

        let max_tap = taps_r[3];
        let history = vec![0.0f32; max_tap + 1];

        let taps_len = mid_taps.len();
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
            mid_taps,
            side_taps,
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

        let taps = self.mid_taps.len();
        self.mid_ring[self.ring_pos] = mid;
        self.mid_ring[self.ring_pos + taps] = mid;
        self.side_ring[self.ring_pos] = wide;
        self.side_ring[self.ring_pos + taps] = wide;

        let head = (2.0 * w).min(1.0);
        if head > 0.0 {
            let mut hm = 0.0f32;
            let mut hs = 0.0f32;
            let r_idx = self.ring_pos + taps;
            // Vectorized FIR convolution
            for k in 0..taps {
                let idx = r_idx - k;
                hm += self.mid_taps[k] * self.mid_ring[idx];
                hs += self.side_taps[k] * self.side_ring[idx];
            }
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
    let output = env.new_string("PlayTube Rust DSP Core v1.0.0 (ARM64 SIMD)")
        .expect("Couldn't create java string");
    output.into_raw()
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_createSpatialEngine(
    _env: JNIEnv,
    _class: JClass,
    sample_rate: jint,
) -> jlong {
    let engine = Box::new(SpatialEngine::new(sample_rate));
    Box::into_raw(engine) as jlong
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_destroySpatialEngine(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        unsafe {
            drop(Box::from_raw(handle as *mut SpatialEngine));
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_resetSpatialEngine(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        let engine = unsafe { &mut *(handle as *mut SpatialEngine) };
        engine.reset();
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
    if handle == 0 || byte_count <= 0 {
        return 0;
    }

    let in_addr = env.get_direct_buffer_address(&input_buf);
    let out_addr = env.get_direct_buffer_address(&output_buf);

    let (in_ptr, out_ptr) = match (in_addr, out_addr) {
        (Ok(inp), Ok(outp)) => (inp, outp),
        _ => return -1,
    };

    let engine = unsafe { &mut *(handle as *mut SpatialEngine) };
    let num_frames = (byte_count as usize) / 4; // 2 channels * 2 bytes

    unsafe {
        let in_i16 = in_ptr as *const i16;
        let out_i16 = out_ptr as *mut i16;

        for i in 0..num_frames {
            let l_raw = *in_i16.add(i * 2) as f32 / 32768.0;
            let r_raw = *in_i16.add(i * 2 + 1) as f32 / 32768.0;

            let (out_l, out_r) = engine.process_frame(l_raw, r_raw, goal_amount, goal_bass);

            let l_i16 = (out_l * 32767.0).clamp(-32768.0, 32767.0) as i16;
            let r_i16 = (out_r * 32767.0).clamp(-32768.0, 32767.0) as i16;

            *out_i16.add(i * 2) = l_i16;
            *out_i16.add(i * 2 + 1) = r_i16;
        }
    }

    byte_count
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

    let timeline = Box::new(lyrics::LrcTimeline::parse(&lrc_rust));
    Box::into_raw(timeline) as jlong
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_destroyLrcTimeline(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        unsafe {
            drop(Box::from_raw(handle as *mut lyrics::LrcTimeline));
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
    if handle == 0 || pos_ms < 0 {
        return -1;
    }
    let timeline = unsafe { &*(handle as *const lyrics::LrcTimeline) };
    timeline.find_active_index(pos_ms as u64)
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_getLrcTimestamps<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> jlongArray {
    if handle == 0 {
        return std::ptr::null_mut();
    }
    let timeline = unsafe { &*(handle as *const lyrics::LrcTimeline) };
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
    if handle == 0 {
        return std::ptr::null_mut();
    }
    let timeline = unsafe { &*(handle as *const lyrics::LrcTimeline) };

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
    let timeline = Box::new(sponsorblock::SponsorTimeline::new());
    Box::into_raw(timeline) as jlong
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_destroySponsorTimeline(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        unsafe {
            drop(Box::from_raw(handle as *mut sponsorblock::SponsorTimeline));
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
    if handle == 0 {
        return;
    }

    let len = match env.get_array_length(&starts) {
        Ok(l) => l as usize,
        Err(_) => return,
    };

    let mut start_vec = vec![0i64; len];
    let mut end_vec = vec![0i64; len];
    let mut cat_vec = vec![0i8; len];
    let mut act_vec = vec![0i8; len];

    let _ = env.get_long_array_region(&starts, 0, &mut start_vec);
    let _ = env.get_long_array_region(&ends, 0, &mut end_vec);
    let _ = env.get_byte_array_region(&categories, 0, &mut cat_vec);
    let _ = env.get_byte_array_region(&actions, 0, &mut act_vec);

    let mut segments = Vec::with_capacity(len);
    for i in 0..len {
        segments.push(sponsorblock::SponsorSegment {
            start_ms: start_vec[i] as u64,
            end_ms: end_vec[i] as u64,
            category: cat_vec[i] as u8,
            action: act_vec[i] as u8,
        });
    }

    let timeline = unsafe { &mut *(handle as *mut sponsorblock::SponsorTimeline) };
    timeline.set_segments(segments);
}

#[no_mangle]
pub extern "system" fn Java_com_newtube_mobile_ui_meetune_PlayTuneNative_checkSponsorPosition(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    pos_ms: jlong,
) -> jlong {
    if handle == 0 || pos_ms < 0 {
        return 0;
    }
    let timeline = unsafe { &*(handle as *const sponsorblock::SponsorTimeline) };
    match timeline.check_position(pos_ms as u64) {
        Some((target_ms, _cat, _act)) => target_ms as jlong,
        None => 0,
    }
}
