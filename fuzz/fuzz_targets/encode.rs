#![no_main]

use libfuzzer_sys::fuzz_target;
use alac_encoder_rs_lucianari::{AlacEncoder, AlacConfig};

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }
    
    // We will just fuzz the default 16-bit stereo encoder with random PCM data.
    let config = AlacConfig {
        frame_size: 352,
        channels: 2,
        bit_depth: 16,
        sample_rate: 44100,
    };
    
    let mut encoder = AlacEncoder::new(config);
    
    // The encoder expects exactly frame_size * channels * (bit_depth/8) bytes.
    let required_len = 352 * 2 * 2;
    let mut pcm = vec![0u8; required_len];
    let copy_len = data.len().min(required_len);
    pcm[..copy_len].copy_from_slice(&data[..copy_len]);
    
    // Output buffer needs to be somewhat larger than PCM in the worst case verbatim.
    let mut out = vec![0u8; required_len * 2 + 1024];
    
    let _n = encoder.encode(&pcm, &mut out).unwrap();
});
