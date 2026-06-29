# ALAC Encoder Engineering Roadmap

This document outlines the strategic vision and engineering milestones for the `alac-encoder-rs-lucianari` project. Our objective is to deliver the most performant, secure, and robust ALAC (Apple Lossless Audio Codec) encoder in the Rust ecosystem, suitable for mission-critical audio infrastructure, embedded devices, and high-throughput streaming services.

## Phase 1: Core Stabilization & Ergonomics (Rust)
**Focus:** API stabilization, basic streaming support, and comprehensive channel handling.

- [x] **v1.0.0 API Freeze:** Stabilize the `AlacEncoder` struct boundaries, configuration parameters, and error types to guarantee semver compatibility for downstream consumers.
- [x] **Streaming I/O Integration:** Implement `std::io::Write` and `tokio::io::AsyncWrite` trait wrappers. This will enable zero-allocation streaming of ALAC frames directly to disk, network sockets, or IPC pipes.
- [x] **Advanced Multi-Channel Support:** Introduce full support for 5.1 and 7.1 surround sound matrix encoding, ensuring compliance with ALAC multi-channel specifications.
- [x] **Dynamic Frame Sizing:** Support for variable frame sizes dynamically adjusted based on transient detection for optimal compression ratios.

## Phase 2: Extreme Performance & Portability (Rust)
**Focus:** Hardware acceleration, zero-copy pipelines, and embedded environments.

- [x] **SIMD Optimization Expansion:** 
  - Offload the stereo decorrelation phase to explicit NEON (ARM64) and AVX-512 (x86_64) intrinsic implementations.
  - Vectorize the Golomb-Rice entropy coding paths for a 15-20% throughput increase.
- [x] **`#![no_std]` Compliance:** Introduce a comprehensive `no_std` feature flag. This requires refactoring internal allocations to use `core` and custom allocators, enabling execution on bare-metal microcontrollers.
- [x] **Zero-Copy Pipeline Architecture:** Implement a unified `bytes::Bytes` based pipeline to completely eliminate intermediate buffer allocations during the predict-and-encode lifecycle.

## Phase 3: Go Integration & Multi-Language Bindings
**Focus:** Bringing our highly optimized Rust core to Go-based microservices and infrastructure.

- [x] **Idiomatic Go Wrapper (`cgo` bindings):** Develop a safe, zero-allocation Go module bridging the Rust FFI boundary, allowing Go servers to encode ALAC audio with native-like performance.
- [x] **Go `io.Reader`/`io.Writer` Interfaces:** Implement streaming encoding seamlessly compatible with the Go standard library's I/O ecosystem.
- [x] **Cloud-Native Go Orchestrator:** Build a reference distributed encoding pipeline in Go that manages pools of Rust-powered encoding worker nodes via gRPC.
- [x] **Cross-Language CI:** Expand testing to run Go-Rust integration tests and fuzzing to guarantee memory safety and thread safety across the FFI boundary.

## Phase 4: Advanced Rust Ecosystem Integrations
**Focus:** Leveraging cutting-edge Rust frameworks for high-throughput deployment.

- [x] **`tokio-uring` / `io_uring` Support:** Exploit Linux's `io_uring` via Rust async runtimes for zero-copy file and network I/O in high-concurrency encoding servers.
- [x] **Distributed Encoding via `tonic` (gRPC):** Develop a microservice scaffolding using Rust's `tonic` framework to scale ALAC encoding horizontally across Kubernetes clusters.
- [x] **eBPF Tracing Hooks:** Embed USDT (Userland Statically Defined Tracing) probes directly into the Rust encoder core for advanced latency profiling in production environments without overhead.

## Phase 5: Future-Proofing & Next-Gen Audio
**Focus:** Broadening the scope of the project beyond simple encoding.

- [x] **Hardware-Accelerated Cryptography (Rust/Go):** Combine ALAC encoding with AES-GCM streaming encryption to provide secure, DRM-ready lossless audio streams for enterprise use cases.
- [x] **WebAssembly (WASM) Module:** Ensure compilation to `wasm32-unknown-unknown` with web-workers support for running the Rust core directly in the browser or via Go WASM wrappers.
- [x] **Rust Audio Ecosystem Native Integration:** 
  - Provide direct encoding sinks for the `symphonia` framework.
  - Implement seamless integration modules for `rodio`.
- [x] **Custom Hardware DSP Targets:** Explore compilation and deployment patterns for specialized Digital Signal Processors (e.g., Hexagon DSPs).

## Phase 6: Advanced DSP & Mastering Upgrades
**Focus:** Expanding the crate from a lossless compressor into a mastering-grade digital audio pipeline for high-end audio delivery.

- [x] **Psychoacoustically Optimized Dithering:** Implement advanced noise-shaping dithering curves (e.g., Triangular PDF, POW-r equivalent models) to smoothly reduce 32-bit float or 24-bit high-resolution mastering sources down to 16-bit ALAC, effectively pushing quantization noise into inaudible frequency bands.
- [x] **Apodizing SRC (Sample Rate Conversion):** Develop a high-precision, 64-bit minimum-phase apodizing filter for downsampling high-resolution audio (e.g., 384kHz/192kHz to 44.1kHz). Apodizing filters will eliminate pre-ringing artifacts introduced by the original ADCs, offering a cleaner transient response for discerning listeners.
- [x] **DSD (Direct Stream Digital) Ingestion Pipeline:** Implement a mastering-grade decimation pipeline to ingest 1-bit DSD streams (DSD64/128/256/512). The pipeline will utilize extremely high-order FIR filtering to convert DSD into pristine 24-bit or 32-bit PCM prior to ALAC encoding.
- [x] **MQA-Aware Passthrough and Analysis:** Add capabilities to detect and preserve Master Quality Authenticated (MQA) folding patterns during the encoding process to ensure the encoded ALAC stream remains bit-perfect and decodable by MQA-certified hardware DACs.
- [x] **Bit-Perfect Verification Suite:** Provide an integrated cryptographic test suite that guarantees bit-perfect transparency of the entire DSP pipeline down to the ALAC frame level, aimed at proving mathematical lossless perfection to the professional market.

## Phase 7: Immersive Audio & Spatial Metadata Integration
**Focus:** Elevating the pipeline to handle object-based spatial audio and next-generation immersive sound formats.

- [x] **ADM BWF (Dolby Atmos) Metadata Preservation:** Support parsing Audio Definition Model (ADM) from Broadcast Wave files and multiplexing this spatial metadata directly into the MP4 container alongside the ALAC bitstream, enabling lossless transmission of Atmos masters.
- [x] **High-Density Immersive Channel Layouts:** Radically expand the ALAC channel coupling matrices beyond 7.1 to fully support modern immersive beds, such as 7.1.4, 9.1.6, and NHK 22.2 surround configurations.

## Phase 8: Advanced AI & Headphone Processing (Research Phase)
**Focus:** Exploring next-generation neural networks and custom convolutions for the ultimate listener experience.

- [ ] **AI-Assisted Spatial Upmixing:** Integrate hooks for ONNX-runtime or `tch-rs` (PyTorch for Rust) to perform real-time, AI-driven upmixing of legacy stereo tracks into binaural or spatial audio arrays immediately before encoding.
- [ ] **Personalized HRTF Convolution Engine:** Implement a zero-latency convolution module that loads standard SOFA (Spatially Oriented Format for Acoustics) files, allowing the encoder to render highly personalized 3D binaural mixes specifically tailored to the listener's ear anatomy for ultimate headphone fidelity.

## Completed Milestones (Historical)
- [x] Implemented core Apple Lossless Audio Codec (ALAC) compression algorithms.
- [x] Integrated SIMD-accelerated prediction and validated via `criterion` benchmarks.
- [x] Added `cargo-fuzz` targets and achieved robust boundary condition coverage against invalid PCM inputs.
- [x] Established strict Rust lints (`clippy` pedantic) and highly restricted local CI testing pipelines via OrbStack + Act.
