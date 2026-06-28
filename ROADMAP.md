# ALAC Encoder Engineering Roadmap

## Completed (Prior History)
- [x] Implemented core Apple Lossless Audio Codec (ALAC) compression algorithms.
- [x] Added `cargo-fuzz` targets and achieved 100% boundary condition coverage.
- [x] Integrated SIMD-accelerated prediction (`benches` via `criterion`).
- [x] Established strict Rust lints (`clippy` pedantic) and CI pipelines.

## Short-term Goals
- [ ] Stabilize the v1.0.0 encoding API (specifically `AlacEncoder` struct boundaries).
- [ ] Add `std::io::Write` trait wrappers for streaming ALAC frames directly to disk/network.
- [ ] Provide multi-channel (5.1/7.1) matrix encoding fallback support.

## Mid-term Goals
- [ ] Explore NEON/AVX512 instruction offloading for the decorrelation phase.
- [ ] Add a `#![no_std]` feature flag for embedded target support.
- [ ] Implement seamless zero-copy buffering across the encoding pipeline.

## Long-term Vision
- [ ] Direct integration with major Rust audio ecosystems (`rodio`, `symphonia`).
- [ ] Explore custom hardware acceleration (e.g. specialized DSPs).
