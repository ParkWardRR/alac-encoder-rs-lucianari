#[cfg(feature = "wasm")]
pub mod bindings {
    use wasm_bindgen::prelude::*;
    use crate::encoder::{AlacEncoder, AlacConfig, ChannelLayout};

    #[wasm_bindgen]
    pub struct WasmAlacEncoder {
        inner: AlacEncoder,
        workspace: Vec<i32>,
    }

    #[wasm_bindgen]
    impl WasmAlacEncoder {
        #[wasm_bindgen(constructor)]
        pub fn new(frame_size: usize, channels: usize, bit_depth: usize, sample_rate: usize) -> Result<WasmAlacEncoder, JsValue> {
            let layout = match channels {
                1 => ChannelLayout::Mono,
                2 => ChannelLayout::Stereo,
                6 => ChannelLayout::Surround5Point1,
                8 => ChannelLayout::Surround7Point1,
                _ => ChannelLayout::Custom(channels as u32),
            };

            let config = AlacConfig {
                frame_size: frame_size as u32,
                channels: channels as u32,
                layout,
                bit_depth: bit_depth as u32,
                sample_rate: sample_rate as u32,
            };

            let ws_size = AlacEncoder::required_workspace(channels as u32, frame_size as u32);
            
            Ok(WasmAlacEncoder {
                inner: AlacEncoder::new(config),
                workspace: vec![0i32; ws_size],
            })
        }

        #[wasm_bindgen]
        pub fn encode(&mut self, pcm_data: &[u8]) -> Result<js_sys::Uint8Array, JsValue> {
            let mut out_buffer = vec![0u8; pcm_data.len() + 8192];
            match self.inner.encode(pcm_data, &mut self.workspace, &mut out_buffer) {
                Ok(size) => {
                    let array = js_sys::Uint8Array::new_with_length(size as u32);
                    array.copy_from(&out_buffer[..size]);
                    Ok(array)
                }
                Err(_) => Err(JsValue::from_str("Encoding failed")),
            }
        }
    }
}
