#![allow(non_snake_case)]

use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use std::collections::HashMap;

pub struct AudioRegistry {
    loaded_libraries: HashMap<String, libloading::Library>,
}

impl AudioRegistry {
    pub fn new() -> Self {
        let mut loaded_libraries = HashMap::new();
        let libs_to_try = [
            ("libpulse-simple.so.0", "libpulse-simple.so"),
            ("libasound.so.2", "libasound.so"),
            ("libpulse.so.0", "libpulse.so"),
        ];

        for (name, alt_name) in libs_to_try {
            if let Ok(lib) = unsafe { libloading::Library::new(name) } {
                tracing::info!("[Maarch64 Audio Passthrough] Successfully loaded host audio library: {}", name);
                loaded_libraries.insert(name.to_string(), lib);
            } else if let Ok(lib) = unsafe { libloading::Library::new(alt_name) } {
                tracing::info!("[Maarch64 Audio Passthrough] Successfully loaded host audio library: {}", alt_name);
                loaded_libraries.insert(name.to_string(), lib);
            }
        }

        Self { loaded_libraries }
    }

    pub fn get_library(&self, name: &str) -> Option<&libloading::Library> {
        self.loaded_libraries.get(name)
    }
}

static AUDIO_REGISTRY: std::sync::OnceLock<AudioRegistry> = std::sync::OnceLock::new();
pub fn get_audio_registry() -> &'static AudioRegistry {
    AUDIO_REGISTRY.get_or_init(AudioRegistry::new)
}

// ----------------------------------------------------------------------------
// PulseAudio Simple API Thunks
// ----------------------------------------------------------------------------
pub fn thunk_pa_simple_new(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let server_ptr = ctx.get_x(0);
    let name_ptr = ctx.get_x(1);
    let dir = ctx.get_x(2) as i32;
    let dev_ptr = ctx.get_x(3);
    let stream_name_ptr = ctx.get_x(4);
    let ss_ptr = ctx.get_x(5);
    let map_ptr = ctx.get_x(6);
    let attr_ptr = ctx.get_x(7);

    let name = if name_ptr != 0 { mem.read_string(name_ptr).ok() } else { None };
    let stream_name = if stream_name_ptr != 0 { mem.read_string(stream_name_ptr).ok() } else { None };

    tracing::info!("[Maarch64 Audio Passthrough] pa_simple_new(app_name={:?}, stream_name={:?})", name, stream_name);

    let registry = get_audio_registry();
    if let Some(pulse_lib) = registry.get_library("libpulse-simple.so.0") {
        unsafe {
            type PaSimpleNewFn = unsafe extern "C" fn(
                *const std::os::raw::c_char,
                *const std::os::raw::c_char,
                i32,
                *const std::os::raw::c_char,
                *const std::os::raw::c_char,
                *const u8,
                *const std::ffi::c_void,
                *const std::ffi::c_void,
                *mut i32,
            ) -> *mut std::ffi::c_void;

            if let Ok(pa_new) = pulse_lib.get::<PaSimpleNewFn>(b"pa_simple_new\0") {
                let c_server = if server_ptr != 0 { mem.read_string(server_ptr).ok().map(|s| std::ffi::CString::new(s).unwrap()) } else { None };
                let c_name = name.as_ref().map(|s| std::ffi::CString::new(s.as_slice()).unwrap());
                let c_dev = if dev_ptr != 0 { mem.read_string(dev_ptr).ok().map(|s| std::ffi::CString::new(s).unwrap()) } else { None };
                let c_stream = stream_name.as_ref().map(|s| std::ffi::CString::new(s.as_slice()).unwrap());

                let default_ss: [i32; 3] = [3i32, 44100i32, 2i32]; // PA_SAMPLE_S16LE (3), 44100Hz, 2 channels
                let ss_bytes = if ss_ptr != 0 { mem.read(ss_ptr, 12).unwrap_or_else(|_| default_ss.align_to::<u8>().1.to_vec()) } else { default_ss.align_to::<u8>().1.to_vec() };

                let mut err = 0i32;
                let pa_handle = pa_new(
                    c_server.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                    c_name.as_ref().map(|s| s.as_ptr()).unwrap_or("Maarch64 Audio\0".as_ptr() as *const _),
                    dir,
                    c_dev.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                    c_stream.as_ref().map(|s| s.as_ptr()).unwrap_or("Playback\0".as_ptr() as *const _),
                    ss_bytes.as_ptr(),
                    map_ptr as *const _,
                    attr_ptr as *const _,
                    &mut err,
                );

                if !pa_handle.is_null() {
                    tracing::info!("[Maarch64 Audio Passthrough] SUCCESS: Connected to Host PulseAudio Stream ({:p})", pa_handle);
                    ctx.set_x(0, pa_handle as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0x4000);
    Ok(())
}

pub fn thunk_pa_simple_write(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let handle_ptr = ctx.get_x(0);
    let data_ptr = ctx.get_x(1);
    let bytes_len = ctx.get_x(2) as usize;
    let error_ptr = ctx.get_x(3);

    let registry = get_audio_registry();
    if let Some(pulse_lib) = registry.get_library("libpulse-simple.so.0") {
        unsafe {
            type PaSimpleWriteFn = unsafe extern "C" fn(*mut std::ffi::c_void, *const u8, usize, *mut i32) -> i32;
            if let Ok(pa_write) = pulse_lib.get::<PaSimpleWriteFn>(b"pa_simple_write\0") {
                if handle_ptr != 0 && handle_ptr != 0x4000 {
                    let audio_bytes = mem.read(data_ptr, bytes_len).unwrap_or_default();
                    let mut err = 0i32;
                    let ret = pa_write(handle_ptr as *mut _, audio_bytes.as_ptr(), bytes_len, &mut err);
                    if error_ptr != 0 { let _ = mem.write(error_ptr, &err.to_le_bytes()); }
                    ctx.set_x(0, ret as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_pa_simple_drain(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let handle_ptr = ctx.get_x(0);
    let error_ptr = ctx.get_x(1);

    let registry = get_audio_registry();
    if let Some(pulse_lib) = registry.get_library("libpulse-simple.so.0") {
        unsafe {
            type PaSimpleDrainFn = unsafe extern "C" fn(*mut std::ffi::c_void, *mut i32) -> i32;
            if let Ok(pa_drain) = pulse_lib.get::<PaSimpleDrainFn>(b"pa_simple_drain\0") {
                if handle_ptr != 0 && handle_ptr != 0x4000 {
                    let mut err = 0i32;
                    let ret = pa_drain(handle_ptr as *mut _, &mut err);
                    if error_ptr != 0 { let _ = mem.write(error_ptr, &err.to_le_bytes()); }
                    ctx.set_x(0, ret as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_pa_simple_free(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let handle_ptr = ctx.get_x(0);
    let registry = get_audio_registry();
    if let Some(pulse_lib) = registry.get_library("libpulse-simple.so.0") {
        unsafe {
            type PaSimpleFreeFn = unsafe extern "C" fn(*mut std::ffi::c_void);
            if let Ok(pa_free) = pulse_lib.get::<PaSimpleFreeFn>(b"pa_free\0") {
                if handle_ptr != 0 && handle_ptr != 0x4000 {
                    pa_free(handle_ptr as *mut _);
                }
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

// ----------------------------------------------------------------------------
// ALSA API Thunks
// ----------------------------------------------------------------------------
pub fn thunk_snd_pcm_open(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let pcm_ret_ptr = ctx.get_x(0);
    let name_ptr = ctx.get_x(1);
    let stream = ctx.get_x(2) as i32;
    let mode = ctx.get_x(3) as i32;

    let name = if name_ptr != 0 { mem.read_string(name_ptr).ok() } else { None };
    tracing::info!("[Maarch64 Audio Passthrough] snd_pcm_open(dev={:?}, stream={}, mode={})", name, stream, mode);

    let registry = get_audio_registry();
    if let Some(alsa_lib) = registry.get_library("libasound.so.2") {
        unsafe {
            type SndPcmOpenFn = unsafe extern "C" fn(*mut *mut std::ffi::c_void, *const std::os::raw::c_char, i32, i32) -> i32;
            if let Ok(pcm_open) = alsa_lib.get::<SndPcmOpenFn>(b"snd_pcm_open\0") {
                let c_name = if let Some(bytes) = name {
                    std::ffi::CString::new(bytes).unwrap()
                } else {
                    std::ffi::CString::new("default").unwrap()
                };
                let mut host_pcm: *mut std::ffi::c_void = std::ptr::null_mut();
                let ret = pcm_open(&mut host_pcm, c_name.as_ptr(), stream, mode);
                if ret == 0 && !host_pcm.is_null() {
                    if pcm_ret_ptr != 0 {
                        let _ = mem.write(pcm_ret_ptr, &(host_pcm as u64).to_le_bytes());
                    }
                    tracing::info!("[Maarch64 Audio Passthrough] SUCCESS: Opened Host ALSA PCM Handle ({:p})", host_pcm);
                    ctx.set_x(0, 0);
                    return Ok(());
                }
            }
        }
    }

    if pcm_ret_ptr != 0 { let _ = mem.write(pcm_ret_ptr, &0x5000u64.to_le_bytes()); }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_snd_pcm_set_params(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let pcm_ptr = ctx.get_x(0);
    let format = ctx.get_x(1) as i32;
    let access = ctx.get_x(2) as i32;
    let channels = ctx.get_x(3) as u32;
    let rate = ctx.get_x(4) as u32;
    let soft_resample = ctx.get_x(5) as i32;
    let latency = ctx.get_x(6) as u32;

    tracing::info!("[Maarch64 Audio Passthrough] snd_pcm_set_params(format={}, channels={}, rate={}Hz)", format, channels, rate);

    let registry = get_audio_registry();
    if let Some(alsa_lib) = registry.get_library("libasound.so.2") {
        unsafe {
            type SndPcmSetParamsFn = unsafe extern "C" fn(*mut std::ffi::c_void, i32, i32, u32, u32, i32, u32) -> i32;
            if let Ok(set_params) = alsa_lib.get::<SndPcmSetParamsFn>(b"snd_pcm_set_params\0") {
                if pcm_ptr != 0 && pcm_ptr != 0x5000 {
                    let ret = set_params(pcm_ptr as *mut _, format, access, channels, rate, soft_resample, latency);
                    ctx.set_x(0, ret as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_snd_pcm_writei(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let pcm_ptr = ctx.get_x(0);
    let buffer_ptr = ctx.get_x(1);
    let frames = ctx.get_x(2) as usize;

    let registry = get_audio_registry();
    if let Some(alsa_lib) = registry.get_library("libasound.so.2") {
        unsafe {
            type SndPcmWriteiFn = unsafe extern "C" fn(*mut std::ffi::c_void, *const u8, usize) -> i64;
            if let Ok(pcm_writei) = alsa_lib.get::<SndPcmWriteiFn>(b"snd_pcm_writei\0") {
                if pcm_ptr != 0 && pcm_ptr != 0x5000 {
                    let bytes_len = frames * 4; // 16-bit stereo frame = 4 bytes
                    let audio_bytes = mem.read(buffer_ptr, bytes_len).unwrap_or_default();
                    let written = pcm_writei(pcm_ptr as *mut _, audio_bytes.as_ptr(), frames);
                    ctx.set_x(0, written as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, frames as u64);
    Ok(())
}

pub fn thunk_snd_pcm_prepare(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let pcm_ptr = ctx.get_x(0);
    let registry = get_audio_registry();
    if let Some(alsa_lib) = registry.get_library("libasound.so.2") {
        unsafe {
            type SndPcmPrepareFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
            if let Ok(pcm_prep) = alsa_lib.get::<SndPcmPrepareFn>(b"snd_pcm_prepare\0") {
                if pcm_ptr != 0 && pcm_ptr != 0x5000 {
                    let ret = pcm_prep(pcm_ptr as *mut _);
                    ctx.set_x(0, ret as u64);
                    return Ok(());
                }
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_snd_pcm_drain(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let pcm_ptr = ctx.get_x(0);
    let registry = get_audio_registry();
    if let Some(alsa_lib) = registry.get_library("libasound.so.2") {
        unsafe {
            type SndPcmDrainFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
            if let Ok(pcm_drain) = alsa_lib.get::<SndPcmDrainFn>(b"snd_pcm_drain\0") {
                if pcm_ptr != 0 && pcm_ptr != 0x5000 {
                    let ret = pcm_drain(pcm_ptr as *mut _);
                    ctx.set_x(0, ret as u64);
                    return Ok(());
                }
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_snd_pcm_close(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let pcm_ptr = ctx.get_x(0);
    let registry = get_audio_registry();
    if let Some(alsa_lib) = registry.get_library("libasound.so.2") {
        unsafe {
            type SndPcmCloseFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
            if let Ok(pcm_close) = alsa_lib.get::<SndPcmCloseFn>(b"snd_pcm_close\0") {
                if pcm_ptr != 0 && pcm_ptr != 0x5000 {
                    let ret = pcm_close(pcm_ptr as *mut _);
                    ctx.set_x(0, ret as u64);
                    return Ok(());
                }
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn register_audio_thunks(thunks: &mut HashMap<String, crate::ThunkFn>) {

    // PulseAudio Thunks
    thunks.insert("pa_simple_new".to_string(), thunk_pa_simple_new);
    thunks.insert("pa_simple_write".to_string(), thunk_pa_simple_write);
    thunks.insert("pa_simple_drain".to_string(), thunk_pa_simple_drain);
    thunks.insert("pa_simple_free".to_string(), thunk_pa_simple_free);

    // ALSA Thunks
    thunks.insert("snd_pcm_open".to_string(), thunk_snd_pcm_open);
    thunks.insert("snd_pcm_set_params".to_string(), thunk_snd_pcm_set_params);
    thunks.insert("snd_pcm_writei".to_string(), thunk_snd_pcm_writei);
    thunks.insert("snd_pcm_prepare".to_string(), thunk_snd_pcm_prepare);
    thunks.insert("snd_pcm_drain".to_string(), thunk_snd_pcm_drain);
    thunks.insert("snd_pcm_close".to_string(), thunk_snd_pcm_close);
}
