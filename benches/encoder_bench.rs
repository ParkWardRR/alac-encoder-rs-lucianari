use criterion::{black_box, criterion_group, criterion_main, Criterion};
use alac_encoder_rs_lucianari::{AlacEncoder, AlacConfig};

fn make_pcm_sine(num_samples: usize) -> Vec<u8> {
    let mut pcm = vec![0u8; num_samples * 4];
    for i in 0..num_samples {
        let t = i as f64 / 44100.0;
        let sample = (f64::sin(2.0 * std::f64::consts::PI * 440.0 * t) * 16000.0) as i16;
        let bytes = sample.to_le_bytes();
        pcm[i * 4] = bytes[0];
        pcm[i * 4 + 1] = bytes[1];
        pcm[i * 4 + 2] = bytes[0];
        pcm[i * 4 + 3] = bytes[1];
    }
    pcm
}

fn bench_encode(c: &mut Criterion) {
    let config = AlacConfig::default();
    let mut enc = AlacEncoder::new(config.clone());
    let pcm = make_pcm_sine(352);
    let mut out = vec![0u8; 8192];
    let mut workspace = vec![0i32; AlacEncoder::required_workspace(config.channels, config.frame_size)];
    
    // Warmup encoder to converge predictor
    for _ in 0..10 {
        enc.encode(&pcm, &mut workspace, &mut out);
    }

    c.bench_function("encode_stereo_16_sine", |b| b.iter(|| {
        enc.encode(black_box(&pcm), black_box(&mut workspace), black_box(&mut out))
    }));
}

criterion_group!(benches, bench_encode);
criterion_main!(benches);
