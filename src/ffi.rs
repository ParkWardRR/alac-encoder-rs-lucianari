use crate::encoder::{AlacConfig, AlacEncoder, ChannelLayout};
use core::ffi::{c_uchar, c_uint, c_void};
use core::slice;

#[repr(C)]
pub struct CAlacConfig {
    pub frame_size: c_uint,
    pub channels: c_uint,
    pub layout: c_uint, // 0 = Mono, 1 = Stereo, 2 = 5.1, 3 = 7.1, 4 = Custom
    pub bit_depth: c_uint,
    pub sample_rate: c_uint,
}

impl From<&CAlacConfig> for AlacConfig {
    fn from(c_config: &CAlacConfig) -> Self {
        let layout = match c_config.layout {
            0 => ChannelLayout::Mono,
            1 => ChannelLayout::Stereo,
            2 => ChannelLayout::Surround5Point1,
            3 => ChannelLayout::Surround7Point1,
            _ => ChannelLayout::Custom(c_config.channels),
        };
        AlacConfig {
            frame_size: c_config.frame_size,
            channels: c_config.channels,
            layout,
            bit_depth: c_config.bit_depth,
            sample_rate: c_config.sample_rate,
        }
    }
}

/// Create a new ALAC encoder.
/// Returns a pointer to the encoder instance, or null if creation fails.
#[no_mangle]
pub extern "C" fn alac_encoder_create(config: *const CAlacConfig) -> *mut c_void {
    if config.is_null() {
        return core::ptr::null_mut();
    }
    
    let c_config = unsafe { &*config };
    let alac_config: AlacConfig = c_config.into();
    
    let encoder = alloc::boxed::Box::new(AlacEncoder::new(alac_config));
    alloc::boxed::Box::into_raw(encoder) as *mut c_void
}

/// Free the ALAC encoder instance.
#[no_mangle]
pub extern "C" fn alac_encoder_free(encoder_ptr: *mut c_void) {
    if !encoder_ptr.is_null() {
        unsafe {
            let _ = alloc::boxed::Box::from_raw(encoder_ptr as *mut AlacEncoder);
        }
    }
}

/// Get the required workspace size (in number of i32 elements).
#[no_mangle]
pub extern "C" fn alac_encoder_required_workspace(channels: c_uint, frame_size: c_uint) -> usize {
    AlacEncoder::required_workspace(channels, frame_size)
}

/// Encode a frame of PCM data.
/// Returns the number of bytes written, or a negative error code on failure.
#[no_mangle]
pub extern "C" fn alac_encoder_encode(
    encoder_ptr: *mut c_void,
    pcm_data: *const c_uchar,
    pcm_len: usize,
    workspace: *mut i32,
    workspace_len: usize,
    out_buffer: *mut c_uchar,
    out_len: usize,
) -> isize {
    if encoder_ptr.is_null() || pcm_data.is_null() || workspace.is_null() || out_buffer.is_null() {
        return -1; // Invalid arguments
    }
    
    let encoder = unsafe { &mut *(encoder_ptr as *mut AlacEncoder) };
    let pcm = unsafe { slice::from_raw_parts(pcm_data, pcm_len) };
    let ws = unsafe { slice::from_raw_parts_mut(workspace, workspace_len) };
    let out = unsafe { slice::from_raw_parts_mut(out_buffer, out_len) };
    
    match encoder.encode(pcm, ws, out) {
        Ok(size) => size as isize,
        Err(_) => -2, // Encoding error
    }
}
