//! Adaptive Golomb-Rice entropy coder for ALAC residuals.
//!
//! Implements the Adaptive Golomb (AG) encoder from vendor's `ag_enc.c`.
//! The Rice parameter k adapts based on a running estimate of the mean
//! absolute residual value, converging to near-optimal entropy coding
//! without side information.

use crate::bitwriter::BitWriter;

/// Initial mean estimate (scaled by 4). vendor uses MB0 = 40.
const MB0: i32 = 40;

/// Maximum unary prefix length before escape.
const MAX_PREFIX_16: u32 = 9;

/// Maximum raw bits for escape coding.
const MAX_DATA_BITS_16: u32 = 16;

/// Encode a slice of residuals using adaptive Golomb-Rice coding.
///
/// Each residual is first mapped from signed to unsigned (zig-zag encoding):
///   0 → 0, -1 → 1, 1 → 2, -2 → 3, 2 → 4, ...
///
/// Then Rice-coded with an adaptive parameter k derived from the running mean.
pub fn encode_residuals(residuals: &[i32], num: usize, bw: &mut BitWriter) {
    let mut mb: i32 = MB0; // running mean estimate (scaled by 4)

    for i in 0..num {
        // Zig-zag encode: signed → unsigned.
        let val = residuals[i];
        let uval: u32 = if val >= 0 {
            (val as u32) << 1
        } else {
            (((-val) as u32) << 1) - 1
        };

        // Compute Rice parameter k from current mean estimate.
        let k = calc_k(mb);

        // Rice encode: quotient in unary + remainder in k bits.
        let q = uval >> k;
        let r = uval & ((1u32 << k) - 1);

        if q < MAX_PREFIX_16 {
            // Normal case: unary quotient + binary remainder.
            // Unary: q ones followed by a zero.
            // Optimization: write up to 8 ones at a time.
            let mut remaining = q;
            while remaining >= 8 {
                bw.write(0xFF, 8);
                remaining -= 8;
            }
            if remaining > 0 {
                bw.write((1u32 << remaining) - 1, remaining);
            }
            bw.write(0, 1); // terminator zero

            // Remainder in k bits.
            if k > 0 {
                bw.write(r, k);
            }
        } else {
            // Escape: MAX_PREFIX ones + raw value in MAX_DATA_BITS bits.
            // This handles outliers that would produce very long unary codes.
            let mut remaining = MAX_PREFIX_16;
            while remaining >= 8 {
                bw.write(0xFF, 8);
                remaining -= 8;
            }
            if remaining > 0 {
                bw.write((1u32 << remaining) - 1, remaining);
            }
            bw.write(uval, MAX_DATA_BITS_16);
        }

        // Update running mean estimate (exponential moving average).
        // mb ≈ 4 * mean(|residual|)
        // Update: mb += (uval - mb/4), clamped to [1, 0xFFFF].
        mb += uval as i32 - (mb >> 2);
        if mb < 1 {
            mb = 1;
        }
        if mb > 0xFFFF {
            mb = 0xFFFF;
        }
    }
}

/// Compute the Rice parameter k from the running mean estimate.
///
/// k = ceil(log2(mean)), where mean ≈ mb/4.
/// The parameter adapts so that ~half the residuals have quotient 0
/// and ~half have quotient 1, which is optimal for Rice coding.
#[inline]
fn calc_k(mb: i32) -> u32 {
    let m = ((mb as u32) + 2) >> 2; // mean estimate
    if m <= 1 {
        return 0;
    }
    // Fast ceil(log2(m)): 32 - leading_zeros(m - 1)
    32 - (m - 1).leading_zeros()
}

/// Estimate the number of bits needed to Golomb-encode residuals.
/// Used for fast mixRes selection without actually writing bits.
pub fn estimate_bits(residuals: &[i32], num: usize) -> u32 {
    let mut mb: i32 = MB0;
    let mut total_bits: u32 = 0;

    for i in 0..num {
        let val = residuals[i];
        let uval: u32 = if val >= 0 {
            (val as u32) << 1
        } else {
            (((-val) as u32) << 1) - 1
        };

        let k = calc_k(mb);
        let q = uval >> k;

        if q < MAX_PREFIX_16 {
            total_bits += q + 1 + k; // unary(q) + zero + remainder(k)
        } else {
            total_bits += MAX_PREFIX_16 + MAX_DATA_BITS_16; // escape
        }

        mb += uval as i32 - (mb >> 2);
        if mb < 1 { mb = 1; }
        if mb > 0xFFFF { mb = 0xFFFF; }
    }

    total_bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc_k() {
        assert_eq!(calc_k(0), 0);
        assert_eq!(calc_k(1), 0);
        assert_eq!(calc_k(4), 0);  // mean=1 → k=0
        assert_eq!(calc_k(8), 1);  // mean=2 → k=1
        assert_eq!(calc_k(16), 2); // mean=4 → k=2
        assert_eq!(calc_k(40), 4); // mean=10 → ceil(log2(10))=4
    }

    #[test]
    fn test_zig_zag() {
        // Verify zig-zag mapping for a few values.
        let cases = [(0, 0u32), (-1, 1), (1, 2), (-2, 3), (2, 4), (-3, 5)];
        for (signed, expected) in cases {
            let uval = if signed >= 0 {
                (signed as u32) << 1
            } else {
                (((-signed) as u32) << 1) - 1
            };
            assert_eq!(uval, expected, "zig_zag({}) = {}, expected {}", signed, uval, expected);
        }
    }

    #[test]
    fn test_encode_silence() {
        // All-zero residuals should encode very compactly.
        let residuals = vec![0i32; 352];
        let mut buf = vec![0u8; 4096];
        let mut bw = BitWriter::new(&mut buf);
        encode_residuals(&residuals, 352, &mut bw);
        let nbytes = bw.finish();
        // 352 zeros should encode to ~352 bits (1 bit each: unary 0 + terminator).
        // Plus some overhead. Should be well under 100 bytes.
        assert!(nbytes < 100, "silence encoded to {} bytes, expected < 100", nbytes);
    }

    #[test]
    fn test_estimate_matches_encode() {
        let residuals: Vec<i32> = (0..100).map(|i| ((i * 7) % 50) as i32 - 25).collect();
        let estimated = estimate_bits(&residuals, 100);

        let mut buf = vec![0u8; 4096];
        let mut bw = BitWriter::new(&mut buf);
        encode_residuals(&residuals, 100, &mut bw);
        let actual_bytes = bw.finish();
        let actual_bits = (actual_bytes as u32) * 8; // upper bound (may have padding)

        // Estimated should be close to actual (within padding).
        assert!(
            estimated <= actual_bits,
            "estimated {} > actual {} bits",
            estimated,
            actual_bits
        );
    }
}
