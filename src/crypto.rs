#[cfg(feature = "crypto")]
pub mod aes {
    use aes_gcm::{
        aead::{Aead, KeyInit, Payload},
        Aes256Gcm, Nonce, Key,
    };
    use rand::RngCore;
    use rand::rngs::OsRng;
    use crate::encoder::AlacError;

    pub struct EncryptedAlacStream {
        cipher: Aes256Gcm,
        nonce_counter: u64,
    }

    impl EncryptedAlacStream {
        /// Create a new AES-GCM encrypted ALAC stream from a 256-bit (32-byte) key.
        pub fn new(key_bytes: &[u8; 32]) -> Self {
            let key = Key::<Aes256Gcm>::from_slice(key_bytes);
            let cipher = Aes256Gcm::new(key);
            Self {
                cipher,
                nonce_counter: OsRng.next_u64(), // Randomize starting nonce for security
            }
        }

        /// Encrypt a single ALAC frame payload.
        pub fn encrypt_frame(&mut self, alac_frame: &[u8]) -> Result<Vec<u8>, AlacError> {
            let mut nonce_bytes = [0u8; 12];
            nonce_bytes[0..8].copy_from_slice(&self.nonce_counter.to_le_bytes());
            let nonce = Nonce::from_slice(&nonce_bytes);
            
            self.nonce_counter = self.nonce_counter.wrapping_add(1);

            let payload = Payload {
                msg: alac_frame,
                aad: b"ALAC_ENCRYPTED_FRAME_V1", // Optional authenticated associated data
            };

            self.cipher.encrypt(nonce, payload)
                .map_err(|_| AlacError::UnsupportedConfig { channels: 0, bit_depth: 0 }) // Generic error map
        }
    }
}
