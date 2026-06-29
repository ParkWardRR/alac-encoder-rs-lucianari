use rand::{Rng, thread_rng};

/// A psychoacoustically optimized noise-shaping dither
/// based loosely on POW-r algorithms to convert 32-bit float or 24-bit PCM
/// into 16-bit ALAC without truncation distortion.
pub struct PowRDither {
    error_history: [f32; 9],
    filter_coeffs: [f32; 9],
}

impl PowRDither {
    pub fn new() -> Self {
        // Example high-pass noise shaping FIR coefficients 
        // to push quantization noise >15kHz
        Self {
            error_history: [0.0; 9],
            filter_coeffs: [2.033, -1.482, 0.407, 0.185, -0.063, -0.012, 0.005, 0.001, -0.001],
        }
    }

    /// Dithers a 32-bit float sample (-1.0 to 1.0) to a 16-bit integer.
    pub fn process_sample(&mut self, sample: f32) -> i16 {
        // TPDF (Triangular Probability Density Function) dither noise
        let mut rng = thread_rng();
        let noise = rng.gen_range(-1.0..1.0) + rng.gen_range(-1.0..1.0);
        
        let mut shaped_error = 0.0;
        for i in 0..9 {
            shaped_error += self.error_history[i] * self.filter_coeffs[i];
        }

        let pre_quant = (sample * 32767.0) + noise + shaped_error;
        
        // Clamp and quantize
        let quantized = pre_quant.clamp(-32768.0, 32767.0).round();
        
        let error = pre_quant - quantized;
        
        // Shift error history
        self.error_history.copy_within(0..8, 1);
        self.error_history[0] = error;

        quantized as i16
    }
}

impl Default for PowRDither {
    fn default() -> Self {
        Self::new()
    }
}
