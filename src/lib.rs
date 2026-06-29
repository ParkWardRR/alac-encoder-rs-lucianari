#![cfg_attr(not(feature = "std"), no_std)]

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

extern crate alloc;

pub mod bitwriter;
pub mod encoder;
pub mod ffi;
pub mod golomb;
mod predictor;
mod simd;

#[cfg(feature = "grpc")]
pub mod grpc;

#[cfg(feature = "crypto")]
pub mod crypto;

#[cfg(feature = "mastering")]
pub mod mastering;

#[cfg(feature = "spatial")]
pub mod spatial;

#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(feature = "integrations")]
pub mod integrations;

#[cfg(feature = "std")]
pub mod stream;

pub use encoder::{AlacConfig, AlacEncoder, AlacError};

#[cfg(feature = "ebpf")]
usdt::dtrace_provider!("src/provider.d");
