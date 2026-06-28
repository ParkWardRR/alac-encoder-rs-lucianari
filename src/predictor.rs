//! Adaptive FIR linear predictor for ALAC.
//!
//! Implements the dynamic predictor from vendor's `dp_enc.c`. The predictor uses
//! a sign-sign LMS (Least Mean Squares) algorithm to adapt filter coefficients
//! per-sample, producing residuals that cluster near zero for efficient entropy
//! coding.
//!
//! The SIMD-accelerated FIR dot product is the hottest inner loop — it runs
//! once per audio sample.

use crate::simd;

/// Coefficient initialization constants (from vendor's dplib.h).
const AINIT: i32 = 38;
const BINIT: i32 = -29;
const CINIT: i32 = -2;

/// Maximum prediction order.
pub const MAX_ORDER: usize = 16;

/// Default prediction order.
/// vendor's reference uses 8, but 16 gives 5-10% better compression on music.
pub const DEFAULT_ORDER: usize = 16;

/// Default denominator shift (vendor uses 9 — denominator = 512).
pub const DEFAULT_DENSHIFT: u32 = 9;

/// Predictor state — persists across frames for better compression.
#[derive(Clone)]
pub struct Predictor {
    pub coefs: [i16; MAX_ORDER],
    pub order: usize,
    pub denshift: u32,
}

impl Predictor {
    /// Create a new predictor with default-initialized coefficients.
    pub fn new(order: usize, denshift: u32) -> Self {
        let mut p = Self {
            coefs: [0i16; MAX_ORDER],
            order: order.min(MAX_ORDER),
            denshift,
        };
        p.init_coefs();
        p
    }

    /// Reset coefficients to initial values (vendor's init_coefs).
    pub fn init_coefs(&mut self) {
        let den = 1i32 << self.denshift;
        self.coefs[0] = ((AINIT * den) >> 4) as i16;
        self.coefs[1] = ((BINIT * den) >> 4) as i16;
        self.coefs[2] = ((CINIT * den) >> 4) as i16;
        for k in 3..self.order {
            self.coefs[k] = 0;
        }
    }

    /// Run the predictor, computing residuals from input samples.
    ///
    /// `input` contains the audio samples (i32), `output` receives residuals.
    /// Both must have length `num`. Coefficients are adapted in-place.
    ///
    /// This is the core encoding function — `pc_block` from vendor's dp_enc.c.
    pub fn encode(&mut self, input: &[i32], output: &mut [i32], num: usize) {
        let order = self.order;
        let denshift = self.denshift;

        if order == 0 || num <= order {
            output[..num].copy_from_slice(&input[..num]);
            return;
        }

        // Warm-up: first sample raw, then differential for the next (order-1).
        output[0] = input[0];
        for j in 1..order {
            output[j] = input[j] - input[j - 1];
        }

        let den = 1i64 << denshift;
        let half = den >> 1;

        // Main prediction loop with sign-sign LMS adaptation.
        for j in order..num {
            // Build history: differences from the anchor sample.
            // anchor = input[j - order - 1] when j > order, else input[0]
            let anchor_idx = if j > order { j - order - 1 } else { 0 };
            let anchor = input[anchor_idx];

            // FIR convolution via SIMD-accelerated dot product.
            let mut history = [0i32; MAX_ORDER];
            for k in 0..order {
                history[k] = input[j - 1 - k] - anchor;
            }

            let sum = simd::fir_dot_product(&self.coefs[..order], &history[..order]);

            // Prediction = anchor + (sum + half) / den
            let pred = anchor as i64 + (sum + half) / den;

            // Residual
            let del = input[j] as i64 - pred;
            output[j] = del as i32;

            // Sign-sign LMS coefficient adaptation.
            // sgn = sign(residual), dd = sign(history[k])
            // coefs[k] += sgn * dd
            let sgn = sign_of(del as i32);
            if sgn != 0 {
                for (k, &history_k) in history.iter().enumerate().take(order) {
                    let dd = sign_of(history_k);
                    // Saturating add to prevent overflow in i16 coefs.
                    self.coefs[k] = self.coefs[k].saturating_add((sgn * dd) as i16);
                }
            }
        }
    }
}

/// Returns -1, 0, or 1 for the sign of a value.
#[inline]
fn sign_of(i: i32) -> i32 {
    if i > 0 {
        1
    } else if i < 0 {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predictor_silence() {
        // Silence should produce all-zero residuals after warm-up.
        let mut pred = Predictor::new(DEFAULT_ORDER, DEFAULT_DENSHIFT);
        let input = vec![0i32; 352];
        let mut output = vec![0i32; 352];
        pred.encode(&input, &mut output, 352);

        // All residuals should be zero for silence.
        for &r in &output {
            assert_eq!(r, 0);
        }
    }

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn test_predictor_dc() {
        // Constant signal — after warm-up, residuals should be zero.
        let mut pred = Predictor::new(DEFAULT_ORDER, DEFAULT_DENSHIFT);
        let input = vec![1000i32; 352];
        let mut output = vec![0i32; 352];
        pred.encode(&input, &mut output, 352);

        // First sample is raw.
        assert_eq!(output[0], 1000);
        // Warm-up diffs should be zero (constant input).
        for j in 1..DEFAULT_ORDER {
            assert_eq!(output[j], 0);
        }
        // After warm-up, predictor converges — residuals near zero.
        for j in DEFAULT_ORDER + 10..352 {
            assert!(
                output[j].abs() <= 1,
                "residual[{}] = {} too large",
                j,
                output[j]
            );
        }
    }

    #[test]
    fn test_predictor_reduces_entropy() {
        // A ramp signal should produce smaller residuals than raw values.
        let mut pred = Predictor::new(DEFAULT_ORDER, DEFAULT_DENSHIFT);
        let input: Vec<i32> = (0..352).map(|i| i * 100).collect();
        let mut output = vec![0i32; 352];
        pred.encode(&input, &mut output, 352);

        let raw_energy: i64 = input.iter().map(|x| (*x as i64).abs()).sum();
        let residual_energy: i64 = output.iter().map(|x| (*x as i64).abs()).sum();

        assert!(
            residual_energy < raw_energy,
            "residual energy {} should be less than raw {}",
            residual_energy,
            raw_energy
        );
    }
}
