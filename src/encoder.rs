//! Top-level ALAC encoder.
//!
//! Orchestrates stereo decorrelation, prediction, and entropy coding to produce
//! compressed ALAC frames. Falls back to verbatim encoding when compression
//! doesn't help (same as vendor's encoder).

use crate::bitwriter::BitWriter;
use crate::golomb;
use crate::predictor::{Predictor, DEFAULT_DENSHIFT, DEFAULT_ORDER};
use crate::simd;

/// ALAC element types (from ALACAudioTypes.h).
const TYPE_SCE: u32 = 0; // Single Channel Element
const TYPE_CPE: u32 = 1; // Channel Pair Element
const TYPE_END: u32 = 7; // End tag

/// Adaptive Golomb pb_factor.
const PB_FACTOR: u32 = 4;

/// Errors that can occur during ALAC encoding.
#[derive(thiserror::Error, Debug)]
pub enum AlacError {
    /// Provided output buffer is too small.
    #[error("Output buffer too small: required {required}, provided {provided}")]
    BufferTooSmall { required: usize, provided: usize },
    /// Unsupported PCM configuration.
    #[error("Unsupported PCM configuration: {channels} channels, {bit_depth} bits")]
    UnsupportedConfig { channels: u32, bit_depth: u32 },
}

/// Encoder configuration.
#[derive(Clone, Debug)]
pub struct AlacConfig {
    /// Samples per frame (typically 352 for protocol).
    pub frame_size: u32,
    /// Number of channels (1 = mono, 2 = stereo).
    pub channels: u32,
    /// Bit depth (16 or 24).
    pub bit_depth: u32,
    /// Sample rate in Hz.
    pub sample_rate: u32,
}

impl Default for AlacConfig {
    fn default() -> Self {
        Self {
            frame_size: 352,
            channels: 2,
            bit_depth: 16,
            sample_rate: 44100,
        }
    }
}

/// ALAC encoder with persistent state across frames.
pub struct AlacEncoder {
    config: AlacConfig,
    pred_u: Predictor,
    pred_v: Predictor,
    // Work buffers — allocated once, reused per frame.
    mix_u: Vec<i32>,
    mix_v: Vec<i32>,
    residuals_u: Vec<i32>,
    residuals_v: Vec<i32>,
    left: Vec<i32>,
    right: Vec<i32>,
    last_mix_res: i32,
}

impl AlacEncoder {
    /// Create a new encoder with the given configuration.
    pub fn new(config: AlacConfig) -> Self {
        let n = config.frame_size as usize;
        Self {
            pred_u: Predictor::new(DEFAULT_ORDER, DEFAULT_DENSHIFT),
            pred_v: Predictor::new(DEFAULT_ORDER, DEFAULT_DENSHIFT),
            mix_u: vec![0i32; n],
            mix_v: vec![0i32; n],
            residuals_u: vec![0i32; n],
            residuals_v: vec![0i32; n],
            left: vec![0i32; n],
            right: vec![0i32; n],
            last_mix_res: 0,
            config,
        }
    }

    /// Encode a frame of interleaved S16LE PCM into an ALAC frame.
    ///
    /// Returns the number of bytes written to `out`, or an `AlacError`.
    /// `pcm` must contain `frame_size * channels * (bit_depth/8)` bytes.
    /// `out` must be large enough (worst case: slightly larger than PCM).
    pub fn encode(&mut self, pcm: &[u8], out: &mut [u8]) -> Result<usize, AlacError> {
        let num = self.config.frame_size as usize;
        let channels = self.config.channels as usize;
        let bit_depth = self.config.bit_depth;

        if channels == 2 && bit_depth == 16 {
            Ok(self.encode_stereo_16(pcm, out, num))
        } else if channels == 2 && bit_depth == 24 {
            Ok(self.encode_stereo_24(pcm, out, num))
        } else if channels == 1 && bit_depth == 16 {
            Ok(self.encode_mono_16(pcm, out, num))
        } else {
            // Unsupported config — return error or emit verbatim.
            // Returning error since verbatim fallback for random bit depths is undefined.
            Err(AlacError::UnsupportedConfig { channels: channels as u32, bit_depth })
        }
    }

    /// Encode stereo 16-bit: the primary optimized path.
    fn encode_stereo_16(&mut self, pcm: &[u8], out: &mut [u8], num: usize) -> usize {
        let order = DEFAULT_ORDER;
        let denshift = DEFAULT_DENSHIFT;
        let chan_bits = 17u32; // 16 + 1 for matrix encoding
        let mix_bits: i32 = 2;

        // SIMD-accelerated deinterleave.
        simd::deinterleave_s16le(pcm, &mut self.left, &mut self.right, num);

        // Search for best mixRes (0, 1, or 2).
        let mut best_bits = u32::MAX;
        let mut best_mix_res: i32 = self.last_mix_res;

        for mr in 0..=2i32 {
            self.apply_mix(num, mix_bits, mr);

            // Clone predictors so the search doesn't modify the real state.
            let mut pu = self.pred_u.clone();
            let mut pv = self.pred_v.clone();
            pu.encode(&self.mix_u, &mut self.residuals_u, num);
            pv.encode(&self.mix_v, &mut self.residuals_v, num);

            let bits_u = golomb::estimate_bits(&self.residuals_u, num);
            let bits_v = golomb::estimate_bits(&self.residuals_v, num);
            let total = bits_u + bits_v;

            if total < best_bits {
                best_bits = total;
                best_mix_res = mr;
            }
        }

        // Re-encode with best mixRes, this time modifying predictor state.
        self.apply_mix(num, mix_bits, best_mix_res);
        self.pred_u.encode(&self.mix_u, &mut self.residuals_u, num);
        self.pred_v.encode(&self.mix_v, &mut self.residuals_v, num);
        self.last_mix_res = best_mix_res;

        // Write compressed frame.
        let compressed_size = self.write_compressed_stereo(
            out,
            num,
            order,
            denshift,
            chan_bits,
            mix_bits,
            best_mix_res,
        );

        // Compare with verbatim size.
        let verbatim_size = self.verbatim_size(num, 2, 16);
        if compressed_size >= verbatim_size {
            return self.encode_verbatim(pcm, out, num, 2, 16);
        }

        compressed_size
    }

    /// Encode stereo 24-bit: 24-bit samples packed as S24LE (3 bytes per sample).
    ///
    /// ALAC 24-bit uses `extraBytes=1` — the extra byte per sample is written
    /// separately from the compressed residuals (the "extra" LSB is peeled off
    /// and stored verbatim, while the upper 16 bits go through prediction +
    /// Golomb-Rice). This matches vendor's reference encoder behavior.
    fn encode_stereo_24(&mut self, pcm: &[u8], out: &mut [u8], num: usize) -> usize {
        let order = DEFAULT_ORDER;
        let denshift = DEFAULT_DENSHIFT;
        let chan_bits = 25u32; // 24 + 1 for matrix encoding
        let mix_bits: i32 = 2;
        let extra_bytes: u32 = 1; // 24-bit mode: 1 extra byte per sample
        let shift: u32 = 8; // Shift off bottom 8 bits for extra storage

        // Deinterleave 24-bit packed PCM.
        simd::deinterleave_s24le(pcm, &mut self.left, &mut self.right, num);

        // Search for best mixRes.
        let mut best_bits = u32::MAX;
        let mut best_mix_res: i32 = self.last_mix_res;

        for mr in 0..=2i32 {
            self.apply_mix(num, mix_bits, mr);

            // Shift down for prediction (predict on upper bits only).
            for i in 0..num {
                self.mix_u[i] >>= shift;
                self.mix_v[i] >>= shift;
            }

            let mut pu = self.pred_u.clone();
            let mut pv = self.pred_v.clone();
            pu.encode(&self.mix_u, &mut self.residuals_u, num);
            pv.encode(&self.mix_v, &mut self.residuals_v, num);

            let bits_u = golomb::estimate_bits(&self.residuals_u, num);
            let bits_v = golomb::estimate_bits(&self.residuals_v, num);
            let total = bits_u + bits_v;

            if total < best_bits {
                best_bits = total;
                best_mix_res = mr;
            }
        }

        // Re-encode with best mixRes.
        self.apply_mix(num, mix_bits, best_mix_res);

        // Save the full-resolution mixed values for extra byte extraction.
        // Extra bytes = bottom `shift` bits of the mixed signal.
        let extra_u: Vec<u8> = self.mix_u.iter().map(|&s| (s & 0xFF) as u8).collect();
        let extra_v: Vec<u8> = self.mix_v.iter().map(|&s| (s & 0xFF) as u8).collect();

        // Shift down for prediction.
        for i in 0..num {
            self.mix_u[i] >>= shift;
            self.mix_v[i] >>= shift;
        }

        self.pred_u.encode(&self.mix_u, &mut self.residuals_u, num);
        self.pred_v.encode(&self.mix_v, &mut self.residuals_v, num);
        self.last_mix_res = best_mix_res;

        // Write compressed frame with extra bytes.
        let compressed_size = self.write_compressed_stereo_24(
            out,
            num,
            order,
            denshift,
            chan_bits,
            mix_bits,
            best_mix_res,
            extra_bytes,
            &extra_u,
            &extra_v,
        );

        let verbatim_size = self.verbatim_size(num, 2, 24);
        if compressed_size >= verbatim_size {
            return self.encode_verbatim(pcm, out, num, 2, 24);
        }

        compressed_size
    }

    /// Encode mono 16-bit.
    fn encode_mono_16(&mut self, pcm: &[u8], out: &mut [u8], num: usize) -> usize {
        // Deinterleave mono (just byte-swap S16LE → i32).
        for i in 0..num {
            self.mix_u[i] = i16::from_le_bytes([pcm[i * 2], pcm[i * 2 + 1]]) as i32;
        }

        self.pred_u.encode(&self.mix_u, &mut self.residuals_u, num);

        let order = DEFAULT_ORDER;
        let denshift = DEFAULT_DENSHIFT;
        let chan_bits = 16u32;

        let compressed_size = self.write_compressed_mono(out, num, order, denshift, chan_bits);

        let verbatim_size = self.verbatim_size(num, 1, 16);
        if compressed_size >= verbatim_size {
            return self.encode_verbatim(pcm, out, num, 1, 16);
        }

        compressed_size
    }

    /// Apply middle-side stereo decorrelation.
    ///
    /// u = (L + (m - r) * R) / m
    /// v = L - R
    ///
    /// where m = 1 << mix_bits, r = mix_res.
    fn apply_mix(&mut self, num: usize, mix_bits: i32, mix_res: i32) {
        let m = 1i32 << mix_bits;
        for i in 0..num {
            let l = self.left[i];
            let r = self.right[i];
            self.mix_u[i] = (l + (m - mix_res) * r) / m;
            self.mix_v[i] = l - r;
        }
    }

    /// Write a compressed stereo ALAC frame.
    #[allow(clippy::too_many_arguments)]
    fn write_compressed_stereo(
        &self,
        out: &mut [u8],
        num: usize,
        order: usize,
        denshift: u32,
        _chan_bits: u32,
        mix_bits: i32,
        mix_res: i32,
    ) -> usize {
        let mut bw = BitWriter::new(out);
        let partial = num != self.config.frame_size as usize;

        // Element header: CPE.
        bw.write(TYPE_CPE, 3);
        bw.write(0, 4); // elementInstanceTag
        bw.write(0, 12); // unused

        // Flags byte: 0000psse
        let p = if partial { 1u32 } else { 0 };
        bw.write(p, 1); // partial frame
        bw.write(0, 2); // extraBytes = 0 (16-bit)
        bw.write(0, 1); // escape = 0 (compressed)

        if partial {
            bw.write(num as u32, 32);
        }

        // Mix parameters.
        bw.write(mix_bits as u32, 8);
        bw.write(mix_res as u32 & 0xFF, 8);

        // Channel U prediction params.
        bw.write(0, 4); // modeU = 0
        bw.write(denshift, 4); // denShiftU
        bw.write(PB_FACTOR, 3); // pbFactorU
        bw.write(order as u32, 5); // numU
        for i in 0..order {
            bw.write(self.pred_u.coefs[i] as u16 as u32, 16);
        }

        // Channel V prediction params.
        bw.write(0, 4); // modeV = 0
        bw.write(denshift, 4); // denShiftV
        bw.write(PB_FACTOR, 3); // pbFactorV
        bw.write(order as u32, 5); // numV
        for i in 0..order {
            bw.write(self.pred_v.coefs[i] as u16 as u32, 16);
        }

        // Entropy-encoded residuals.
        golomb::encode_residuals(&self.residuals_u, num, &mut bw);
        golomb::encode_residuals(&self.residuals_v, num, &mut bw);

        // End tag.
        bw.write(TYPE_END, 3);
        bw.finish()
    }

    /// Write a compressed stereo 24-bit ALAC frame with extra bytes.
    #[allow(clippy::too_many_arguments)]
    fn write_compressed_stereo_24(
        &self,
        out: &mut [u8],
        num: usize,
        order: usize,
        denshift: u32,
        _chan_bits: u32,
        mix_bits: i32,
        mix_res: i32,
        extra_bytes: u32,
        extra_u: &[u8],
        extra_v: &[u8],
    ) -> usize {
        let mut bw = BitWriter::new(out);
        let partial = num != self.config.frame_size as usize;

        // Element header: CPE.
        bw.write(TYPE_CPE, 3);
        bw.write(0, 4); // elementInstanceTag
        bw.write(0, 12); // unused

        // Flags byte: 0000psse
        let p = if partial { 1u32 } else { 0 };
        bw.write(p, 1); // partial frame
        bw.write(extra_bytes, 2); // extraBytes = 1 for 24-bit
        bw.write(0, 1); // escape = 0 (compressed)

        if partial {
            bw.write(num as u32, 32);
        }

        // Mix parameters.
        bw.write(mix_bits as u32, 8);
        bw.write(mix_res as u32 & 0xFF, 8);

        // Channel U prediction params.
        bw.write(0, 4);
        bw.write(denshift, 4);
        bw.write(PB_FACTOR, 3);
        bw.write(order as u32, 5);
        for i in 0..order {
            bw.write(self.pred_u.coefs[i] as u16 as u32, 16);
        }

        // Channel V prediction params.
        bw.write(0, 4);
        bw.write(denshift, 4);
        bw.write(PB_FACTOR, 3);
        bw.write(order as u32, 5);
        for i in 0..order {
            bw.write(self.pred_v.coefs[i] as u16 as u32, 16);
        }

        // Extra bytes (LSBs): interleaved U/V, 8 bits each.
        for i in 0..num {
            bw.write(extra_u[i] as u32, 8);
            bw.write(extra_v[i] as u32, 8);
        }

        // Entropy-encoded residuals (upper bits).
        golomb::encode_residuals(&self.residuals_u, num, &mut bw);
        golomb::encode_residuals(&self.residuals_v, num, &mut bw);

        // End tag.
        bw.write(TYPE_END, 3);
        bw.finish()
    }

    /// Write a compressed mono ALAC frame.
    fn write_compressed_mono(
        &self,
        out: &mut [u8],
        num: usize,
        order: usize,
        denshift: u32,
        _chan_bits: u32,
    ) -> usize {
        let mut bw = BitWriter::new(out);
        let partial = num != self.config.frame_size as usize;

        bw.write(TYPE_SCE, 3);
        bw.write(0, 4);
        bw.write(0, 12);

        let p = if partial { 1u32 } else { 0 };
        bw.write(p, 1);
        bw.write(0, 2);
        bw.write(0, 1);

        if partial {
            bw.write(num as u32, 32);
        }

        // Mono has no mix params — just prediction params.
        bw.write(0, 4);
        bw.write(denshift, 4);
        bw.write(PB_FACTOR, 3);
        bw.write(order as u32, 5);
        for i in 0..order {
            bw.write(self.pred_u.coefs[i] as u16 as u32, 16);
        }

        golomb::encode_residuals(&self.residuals_u, num, &mut bw);
        bw.write(TYPE_END, 3);
        bw.finish()
    }

    /// Write a verbatim (uncompressed) ALAC frame.
    fn encode_verbatim(
        &self,
        pcm: &[u8],
        out: &mut [u8],
        num: usize,
        channels: usize,
        bit_depth: usize,
    ) -> usize {
        let mut bw = BitWriter::new(out);

        // Element header.
        if channels == 2 {
            bw.write(TYPE_CPE, 3);
        } else {
            bw.write(TYPE_SCE, 3);
        }
        bw.write(0, 4); // elementInstanceTag
        bw.write(0, 12); // unused

        bw.write(1, 1); // hasSize = 1
        bw.write(0, 2); // extraBytes = 0
        bw.write(1, 1); // escape = 1 (verbatim)

        bw.write(num as u32, 32); // numSamples

        // Write raw samples.
        let bytes_per_sample = bit_depth / 8;
        for i in 0..(num * channels) {
            let off = i * bytes_per_sample;
            match bit_depth {
                16 => {
                    let sample = u16::from_le_bytes([pcm[off], pcm[off + 1]]);
                    bw.write(sample as u32, 16);
                }
                24 => {
                    let sample =
                        pcm[off] as u32 | (pcm[off + 1] as u32) << 8 | (pcm[off + 2] as u32) << 16;
                    bw.write(sample, 24);
                }
                _ => {
                    // Generic fallback.
                    let mut val: u32 = 0;
                    for b in 0..bytes_per_sample {
                        val |= (pcm[off + b] as u32) << (b * 8);
                    }
                    bw.write(val, bit_depth as u32);
                }
            }
        }

        bw.write(TYPE_END, 3);
        bw.finish()
    }

    /// Calculate the verbatim frame size in bytes for comparison.
    fn verbatim_size(&self, num: usize, channels: usize, bit_depth: usize) -> usize {
        // header: 3 + 4 + 12 + 1 + 2 + 1 + 32 = 55 bits
        // samples: num * channels * bit_depth bits
        // end: 3 bits
        let total_bits = 55 + num * channels * bit_depth + 3;
        total_bits.div_ceil(8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pcm_silence(num_samples: usize) -> Vec<u8> {
        vec![0u8; num_samples * 4] // stereo 16-bit = 4 bytes/sample
    }

    fn make_pcm_sine(num_samples: usize) -> Vec<u8> {
        let mut pcm = vec![0u8; num_samples * 4];
        for i in 0..num_samples {
            let t = i as f64 / 44100.0;
            let sample = (f64::sin(2.0 * std::f64::consts::PI * 440.0 * t) * 16000.0) as i16;
            let bytes = sample.to_le_bytes();
            pcm[i * 4] = bytes[0];
            pcm[i * 4 + 1] = bytes[1];
            pcm[i * 4 + 2] = bytes[0]; // same on both channels
            pcm[i * 4 + 3] = bytes[1];
        }
        pcm
    }

    #[test]
    fn test_encode_silence() {
        let config = AlacConfig::default();
        let mut enc = AlacEncoder::new(config);
        let pcm = make_pcm_silence(352);
        let mut out = vec![0u8; 8192];
        let n = enc.encode(&pcm, &mut out).unwrap();

        // Silence should compress very well — much smaller than verbatim (1416 bytes).
        assert!(n > 0, "encoded 0 bytes");
        assert!(n < 200, "silence encoded to {} bytes, expected < 200", n);
    }

    #[test]
    fn test_encode_sine() {
        let config = AlacConfig::default();
        let mut enc = AlacEncoder::new(config);
        let pcm = make_pcm_sine(352);
        let mut out = vec![0u8; 8192];
        let n = enc.encode(&pcm, &mut out).unwrap();

        // Sine should compress meaningfully vs verbatim (~1416 bytes).
        assert!(n > 0, "encoded 0 bytes");
        let verbatim_approx = 352 * 2 * 2; // ~1408 bytes raw
        assert!(
            n < verbatim_approx,
            "sine encoded to {} bytes, expected < {} (verbatim)",
            n,
            verbatim_approx
        );
    }

    #[test]
    fn test_encode_multiple_frames() {
        // Verify encoder state persists across frames for better compression.
        let config = AlacConfig::default();
        let mut enc = AlacEncoder::new(config);
        let pcm = make_pcm_sine(352);
        let mut out = vec![0u8; 8192];

        let mut sizes = Vec::new();
        for _ in 0..10 {
            let n = enc.encode(&pcm, &mut out).unwrap();
            sizes.push(n);
        }

        // Later frames should compress at least as well as the first
        // (predictor coefficients converge).
        let first = sizes[0];
        let last = sizes[sizes.len() - 1];
        assert!(
            last <= first + 10,
            "compression got worse: first={} last={}",
            first,
            last
        );
    }

    // ── 24-bit tests ────────────────────────────────────────────────────────

    fn make_pcm_silence_24(num_samples: usize) -> Vec<u8> {
        vec![0u8; num_samples * 6] // stereo 24-bit = 6 bytes/frame
    }

    fn make_pcm_sine_24(num_samples: usize) -> Vec<u8> {
        let mut pcm = vec![0u8; num_samples * 6];
        for i in 0..num_samples {
            let t = i as f64 / 48000.0;
            // 24-bit range: ±8388607
            let sample = (f64::sin(2.0 * std::f64::consts::PI * 440.0 * t) * 4_000_000.0) as i32;
            let bytes = sample.to_le_bytes();
            // Left channel: 3 bytes LE
            pcm[i * 6] = bytes[0];
            pcm[i * 6 + 1] = bytes[1];
            pcm[i * 6 + 2] = bytes[2];
            // Right channel: same
            pcm[i * 6 + 3] = bytes[0];
            pcm[i * 6 + 4] = bytes[1];
            pcm[i * 6 + 5] = bytes[2];
        }
        pcm
    }

    #[test]
    fn test_encode_silence_24bit() {
        let config = AlacConfig {
            frame_size: 352,
            channels: 2,
            bit_depth: 24,
            sample_rate: 48000,
        };
        let mut enc = AlacEncoder::new(config);
        let pcm = make_pcm_silence_24(352);
        let mut out = vec![0u8; 16384];
        let n = enc.encode(&pcm, &mut out).unwrap();

        assert!(n > 0, "encoded 0 bytes");
        // Silence should compress well — verbatim 24-bit stereo is ~2112 bytes.
        // 24-bit mode has fixed extra-byte overhead (352 * 2 * 1 = 704 bytes).
        assert!(
            n < 1000,
            "24-bit silence encoded to {} bytes, expected < 1000",
            n
        );
    }

    #[test]
    fn test_encode_sine_24bit() {
        let config = AlacConfig {
            frame_size: 352,
            channels: 2,
            bit_depth: 24,
            sample_rate: 48000,
        };
        let mut enc = AlacEncoder::new(config);
        let pcm = make_pcm_sine_24(352);
        let mut out = vec![0u8; 16384];
        let n = enc.encode(&pcm, &mut out).unwrap();

        assert!(n > 0, "encoded 0 bytes");
        let verbatim_approx = 352 * 2 * 3; // ~2112 bytes raw
        assert!(
            n < verbatim_approx,
            "24-bit sine encoded to {} bytes, expected < {} (verbatim)",
            n,
            verbatim_approx
        );
    }

    #[test]
    fn test_encode_24bit_multiple_frames() {
        let config = AlacConfig {
            frame_size: 352,
            channels: 2,
            bit_depth: 24,
            sample_rate: 48000,
        };
        let mut enc = AlacEncoder::new(config);
        let pcm = make_pcm_sine_24(352);
        let mut out = vec![0u8; 16384];

        let mut sizes = Vec::new();
        for _ in 0..10 {
            let n = enc.encode(&pcm, &mut out).unwrap();
            sizes.push(n);
        }

        let first = sizes[0];
        let last = sizes[sizes.len() - 1];
        assert!(
            last <= first + 20,
            "24-bit compression got worse: first={} last={}",
            first,
            last
        );
    }
}
