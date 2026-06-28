use rodio::Source;
use std::time::Duration;

pub struct RodioAlacSource {
    // Encoded ALAC frames ready to be decoded by downstream Rodio pipeline
    // or vice versa (Rodio source producing PCM that we encode).
    frames: Vec<Vec<u8>>,
}

impl Iterator for RodioAlacSource {
    type Item = i16; // Typically rodio sources output i16 or f32 samples

    fn next(&mut self) -> Option<Self::Item> {
        // Stub implementation
        None
    }
}

impl Source for RodioAlacSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        44100
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
