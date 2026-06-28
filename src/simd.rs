//! SIMD-accelerated primitives for the ALAC encoder.
//!
//! Provides hardware-accelerated versions of the hot inner loops:
//! - FIR convolution (prediction inner loop)
//! - Sample deinterleaving (S16LE stereo → separate channels)
//! - Sign extraction for LMS coefficient adaptation
//!
//! Falls back to scalar implementations when SIMD is unavailable.

/// Compute the dot product of `coefs` (i16) and `history` (i32), returning
/// the sum as i64. This is the FIR prediction inner loop — called once per
/// sample, so it must be fast.
///
/// `coefs` and `history` must have the same length (typically 4 or 8).
#[inline]
pub fn fir_dot_product(coefs: &[i16], history: &[i32]) -> i64 {
    #[cfg(target_arch = "aarch64")]
    {
        if history.len() >= 4 {
            return unsafe { fir_dot_neon(coefs, history) };
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sse2") && history.len() >= 4 {
            return unsafe { fir_dot_sse2(coefs, history) };
        }
    }

    fir_dot_scalar(coefs, history)
}

/// Scalar fallback for FIR dot product.
#[inline]
fn fir_dot_scalar(coefs: &[i16], history: &[i32]) -> i64 {
    let mut sum: i64 = 0;
    for (c, h) in coefs.iter().zip(history.iter()) {
        sum += *c as i64 * *h as i64;
    }
    sum
}

/// Deinterleave stereo S16LE samples into separate i32 channel buffers.
#[inline]
pub fn deinterleave_s16le(pcm: &[u8], left: &mut [i32], right: &mut [i32], num_samples: usize) {
    #[cfg(target_arch = "aarch64")]
    {
        if num_samples >= 8 {
            unsafe { deinterleave_s16le_neon(pcm, left, right, num_samples) };
            return;
        }
    }

    deinterleave_s16le_scalar(pcm, left, right, num_samples);
}

/// Scalar deinterleave.
#[inline]
fn deinterleave_s16le_scalar(pcm: &[u8], left: &mut [i32], right: &mut [i32], num_samples: usize) {
    for i in 0..num_samples {
        let off = i * 4;
        left[i] = i16::from_le_bytes([pcm[off], pcm[off + 1]]) as i32;
        right[i] = i16::from_le_bytes([pcm[off + 2], pcm[off + 3]]) as i32;
    }
}

/// Deinterleave stereo S24LE (packed 3 bytes/sample) into separate i32 channel buffers.
/// Each stereo frame is 6 bytes: [L0 L1 L2 R0 R1 R2].
#[inline]
pub fn deinterleave_s24le(pcm: &[u8], left: &mut [i32], right: &mut [i32], num_samples: usize) {
    for i in 0..num_samples {
        let off = i * 6; // 3 bytes per sample × 2 channels
        left[i] = s24le_to_i32(&pcm[off..off + 3]);
        right[i] = s24le_to_i32(&pcm[off + 3..off + 6]);
    }
}

/// Convert a 3-byte little-endian signed 24-bit value to i32 with sign extension.
#[inline]
fn s24le_to_i32(b: &[u8]) -> i32 {
    let raw = b[0] as u32 | (b[1] as u32) << 8 | (b[2] as u32) << 16;
    // Sign-extend from 24 bits to 32 bits.
    if raw & 0x800000 != 0 {
        (raw | 0xFF000000) as i32
    } else {
        raw as i32
    }
}

// ── aarch64 NEON implementations ──────────────────────────────────────────────

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

/// NEON FIR dot product: processes 4 coefs at a time using widening multiply-add.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn fir_dot_neon(coefs: &[i16], history: &[i32]) -> i64 {
    let n = coefs.len();
    let chunks = n / 4;
    let mut acc = vdupq_n_s64(0);

    for i in 0..chunks {
        let base = i * 4;
        // Load 4 x i16 coefs → widen to 4 x i32
        let c16 = vld1_s16(coefs.as_ptr().add(base));
        let c32 = vmovl_s16(c16);

        // Load 4 x i32 history
        let h = vld1q_s32(history.as_ptr().add(base));

        // Multiply: 4 x i32 * i32 → we need i64 results
        // Use widening: multiply low pair → i64, high pair → i64
        let prod_lo = vmull_s32(vget_low_s32(vmulq_s32(c32, h)), vdup_n_s32(1));
        let prod_hi = vmull_s32(vget_high_s32(vmulq_s32(c32, h)), vdup_n_s32(1));

        // Accumulate
        acc = vaddq_s64(acc, prod_lo);
        acc = vaddq_s64(acc, prod_hi);
    }

    // Horizontal sum of the 2 x i64 lanes
    let mut sum = vgetq_lane_s64(acc, 0) + vgetq_lane_s64(acc, 1);

    // Handle remaining elements (if n is not a multiple of 4)
    for i in (chunks * 4)..n {
        sum += coefs[i] as i64 * history[i] as i64;
    }

    sum
}

/// NEON deinterleave: loads 8 stereo pairs at a time, deinterleaves with VLD2.
#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
unsafe fn deinterleave_s16le_neon(pcm: &[u8], left: &mut [i32], right: &mut [i32], num_samples: usize) {
    let chunks = num_samples / 8;
    let pcm_ptr = pcm.as_ptr() as *const i16;

    for i in 0..chunks {
        let base = i * 8;
        // VLD2: deinterleave 8 stereo pairs into separate L/R vectors
        let stereo = vld2q_s16(pcm_ptr.add(base * 2));

        // Widen i16 → i32 (low and high halves)
        let l_lo = vmovl_s16(vget_low_s16(stereo.0));
        let l_hi = vmovl_s16(vget_high_s16(stereo.0));
        let r_lo = vmovl_s16(vget_low_s16(stereo.1));
        let r_hi = vmovl_s16(vget_high_s16(stereo.1));

        // Store
        vst1q_s32(left.as_mut_ptr().add(base), l_lo);
        vst1q_s32(left.as_mut_ptr().add(base + 4), l_hi);
        vst1q_s32(right.as_mut_ptr().add(base), r_lo);
        vst1q_s32(right.as_mut_ptr().add(base + 4), r_hi);
    }

    // Remaining samples
    for i in (chunks * 8)..num_samples {
        let off = i * 4;
        left[i] = i16::from_le_bytes([pcm[off], pcm[off + 1]]) as i32;
        right[i] = i16::from_le_bytes([pcm[off + 2], pcm[off + 3]]) as i32;
    }
}

// ── x86_64 SSE2 implementations ──────────────────────────────────────────────

#[cfg(target_arch = "x86_64")]
#[allow(unused_imports)]
use std::arch::x86_64::*;

/// SSE2 FIR dot product: processes elements using i64 accumulation.
/// SSE2 lacks a signed 32×32→64 multiply (_mm_mul_epi32 is SSE4.1),
/// so we fall back to scalar i64 accumulation which auto-vectorizes
/// well with -C target-cpu=native on modern x86_64.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn fir_dot_sse2(coefs: &[i16], history: &[i32]) -> i64 {
    // Scalar i64 accumulation — the compiler auto-vectorizes this with
    // SSE2/AVX2 when built with target-cpu=native. Manual SSE2 signed
    // 32×32→64 emulation is complex and error-prone (no _mm_mul_epi32
    // until SSE4.1), while the auto-vectorized scalar path matches or
    // beats hand-written SSE2 on modern CPUs.
    let n = coefs.len();
    let mut sum: i64 = 0;

    // Process in groups of 4 for better ILP.
    let chunks = n / 4;
    for i in 0..chunks {
        let base = i * 4;
        sum += coefs[base] as i64 * history[base] as i64;
        sum += coefs[base + 1] as i64 * history[base + 1] as i64;
        sum += coefs[base + 2] as i64 * history[base + 2] as i64;
        sum += coefs[base + 3] as i64 * history[base + 3] as i64;
    }

    for i in (chunks * 4)..n {
        sum += coefs[i] as i64 * history[i] as i64;
    }

    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fir_dot_product() {
        let coefs: Vec<i16> = vec![10, -20, 30, -40, 50, -60, 70, -80];
        let history: Vec<i32> = vec![100, 200, 300, 400, 500, 600, 700, 800];
        let expected: i64 = coefs.iter().zip(history.iter())
            .map(|(c, h)| *c as i64 * *h as i64)
            .sum();
        assert_eq!(fir_dot_product(&coefs, &history), expected);
    }

    #[test]
    fn test_deinterleave() {
        let pcm: Vec<u8> = vec![
            0x01, 0x00, 0x02, 0x00, // L=1, R=2
            0xFE, 0xFF, 0xFD, 0xFF, // L=-2, R=-3
            0x00, 0x80, 0xFF, 0x7F, // L=-32768, R=32767
            0x00, 0x00, 0x00, 0x00, // L=0, R=0
        ];
        let mut left = vec![0i32; 4];
        let mut right = vec![0i32; 4];
        deinterleave_s16le(&pcm, &mut left, &mut right, 4);
        assert_eq!(left, vec![1, -2, -32768, 0]);
        assert_eq!(right, vec![2, -3, 32767, 0]);
    }
}
