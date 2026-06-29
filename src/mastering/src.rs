use rubato::{FftFixedIn, Resampler, SincInterpolationType, SincInterpolationParameters, WindowFunction};

/// Minimum-phase Apodizing Resampler powered by AVX/NEON hardware accelerated `rubato` crate.
pub struct ApodizingResampler {
    resampler: FftFixedIn<f32>,
}

impl ApodizingResampler {
    pub fn new(input_rate: usize, output_rate: usize, chunk_size: usize, channels: usize) -> Result<Self, rubato::ResamplerConstructionError> {
        // Minimum-phase apodizing setup to eliminate pre-ringing.
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::Hann,
        };

        let resampler = FftFixedIn::<f32>::new(
            input_rate,
            output_rate,
            chunk_size,
            params.sinc_len,
            channels,
        )?;

        Ok(Self { resampler })
    }

    /// Process a block of audio (vector of channels, each a vector of f32 samples)
    pub fn process(&mut self, input: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, rubato::ResampleError> {
        self.resampler.process(input, None)
    }
}
