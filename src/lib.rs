//! ALAC (vendor Lossless Audio Codec) encoder with SIMD acceleration.
//!
//! Implements the compressed encoding path for stereo 16-bit and 24-bit frames,
//! based on the vendor open-source reference (Apache 2.0):
//!
//! - Middle-side stereo decorrelation
//! - Adaptive FIR linear prediction (order 4–8, sign-sign LMS adaptation)
//! - Adaptive Golomb-Rice entropy coding
//! - Automatic verbatim fallback when compression doesn't help
//! - SIMD: aarch64 NEON for FIR inner loop and sample deinterleaving,
//!   x86_64 SSE2 fallback

mod bitwriter;
mod encoder;
mod golomb;
mod predictor;
mod simd;

pub use encoder::{AlacConfig, AlacEncoder};
