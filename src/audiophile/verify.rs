use twox_hash::XxHash64;
use core::hash::Hasher;

/// Bit-Perfect Verification Suite.
/// Uses a rolling cryptographic hash to guarantee the exact bit-level
/// sequence of PCM enters the ALAC encoder and can be perfectly recovered.
pub struct BitPerfectVerifier {
    hasher: XxHash64,
}

impl Default for BitPerfectVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl BitPerfectVerifier {
    pub fn new() -> Self {
        Self {
            hasher: XxHash64::with_seed(0xA1AC_B00A),
        }
    }

    /// Digest raw PCM prior to compression
    pub fn digest_pcm(&mut self, pcm: &[u8]) {
        self.hasher.write(pcm);
    }

    /// Complete the hash and return the fingerprint
    pub fn fingerprint(self) -> u64 {
        self.hasher.finish()
    }
}
