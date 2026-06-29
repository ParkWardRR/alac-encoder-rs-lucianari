/// A 1-bit PDM (Direct Stream Digital) to 24-bit PCM decimation filter.
/// Employs a Cascaded Integrator-Comb (CIC) filter.
pub struct DsdDecimator {
    integrators: [i64; 4],
    combs: [i64; 4],
    decimation_factor: usize,
    count: usize,
}

impl DsdDecimator {
    pub fn new(decimation_factor: usize) -> Self {
        Self {
            integrators: [0; 4],
            combs: [0; 4],
            decimation_factor,
            count: 0,
        }
    }

    /// Process a stream of 1-bit DSD samples packed into a u8 stream (8 samples per byte)
    pub fn decimate(&mut self, dsd_bytes: &[u8]) -> Vec<i32> {
        let mut pcm_out = Vec::with_capacity(dsd_bytes.len() * 8 / self.decimation_factor);

        for &byte in dsd_bytes {
            for bit in (0..8).rev() {
                // Convert 1-bit (0 or 1) into +1 or -1
                let sample = if ((byte >> bit) & 1) == 1 { 1 } else { -1 };
                
                // Integrator stages
                self.integrators[0] = self.integrators[0].wrapping_add(sample);
                self.integrators[1] = self.integrators[1].wrapping_add(self.integrators[0]);
                self.integrators[2] = self.integrators[2].wrapping_add(self.integrators[1]);
                self.integrators[3] = self.integrators[3].wrapping_add(self.integrators[2]);

                self.count += 1;
                
                // Decimation and Comb stages
                if self.count >= self.decimation_factor {
                    self.count = 0;
                    
                    let v = self.integrators[3];
                    let c0 = v.wrapping_sub(self.combs[0]); self.combs[0] = v;
                    let c1 = c0.wrapping_sub(self.combs[1]); self.combs[1] = c0;
                    let c2 = c1.wrapping_sub(self.combs[2]); self.combs[2] = c1;
                    let c3 = c2.wrapping_sub(self.combs[3]); self.combs[3] = c2;
                    
                    // Normalize the output to 24-bit PCM domain
                    let out_sample = (c3 >> (4 * 2)) as i32; 
                    pcm_out.push(out_sample);
                }
            }
        }

        pcm_out
    }
}
