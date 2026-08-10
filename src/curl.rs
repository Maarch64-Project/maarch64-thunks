#![allow(non_snake_case)]

use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use std::collections::HashMap;
use std::os::raw::{c_char, c_int, c_long, c_void};
use std::sync::OnceLock;

pub struct CurlRegistry {
    loaded_libraries: HashMap<String, libloading::Library>,
}

impl CurlRegistry {
    pub fn new() -> Self {
        let mut loaded_libraries = HashMap::new();
        let libs_to_try = [
            ("libcurl.so.4", "libcurl.so"),
            ("libssl.so.3", "libssl.so"),
            ("libcrypto.so.3", "libcrypto.so"),
        ];

        for (name, alt_name) in libs_to_try {
            if let Ok(lib) = unsafe { libloading::Library::new(name) } {
                tracing::info!("[Maarch64 curl Passthrough] Successfully loaded host library: {}", name);
                loaded_libraries.insert(name.to_string(), lib);
            } else if let Ok(lib) = unsafe { libloading::Library::new(alt_name) } {
                tracing::info!("[Maarch64 curl Passthrough] Successfully loaded host library: {}", alt_name);
                loaded_libraries.insert(name.to_string(), lib);
            }
        }

        Self { loaded_libraries }
    }

    pub fn get_library(&self, name: &str) -> Option<&libloading::Library> {
        self.loaded_libraries.get(name)
            .or_else(|| self.loaded_libraries.values().next())
    }
}

static CURL_REGISTRY: OnceLock<CurlRegistry> = OnceLock::new();
pub fn get_curl_registry() -> &'static CurlRegistry {
    CURL_REGISTRY.get_or_init(CurlRegistry::new)
}

pub fn thunk_curl_global_init(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let flags = ctx.get_x(0) as c_long;
    tracing::info!("[Maarch64 curl Passthrough] curl_global_init(flags={})", flags);

    if let Some(curl_lib) = get_curl_registry().get_library("libcurl.so.4") {
        unsafe {
            type GlobalInitFn = unsafe extern "C" fn(c_long) -> c_int;
            if let Ok(global_init) = curl_lib.get::<GlobalInitFn>(b"curl_global_init\0") {
                let res = global_init(flags);
                ctx.set_x(0, res as i64 as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_curl_global_cleanup(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[Maarch64 curl Passthrough] curl_global_cleanup()");

    if let Some(curl_lib) = get_curl_registry().get_library("libcurl.so.4") {
        unsafe {
            type GlobalCleanupFn = unsafe extern "C" fn();
            if let Ok(global_cleanup) = curl_lib.get::<GlobalCleanupFn>(b"curl_global_cleanup\0") {
                global_cleanup();
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_curl_easy_init(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[Maarch64 curl Passthrough] curl_easy_init()");

    if let Some(curl_lib) = get_curl_registry().get_library("libcurl.so.4") {
        unsafe {
            type EasyInitFn = unsafe extern "C" fn() -> *mut c_void;
            if let Ok(easy_init) = curl_lib.get::<EasyInitFn>(b"curl_easy_init\0") {
                let handle = easy_init();
                ctx.set_x(0, handle as u64);
                return Ok(());
            }
        }
    }

    ctx.set_x(0, 0x8888_0000);
    Ok(())
}

pub fn thunk_curl_easy_cleanup(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let handle = ctx.get_x(0) as *mut c_void;
    tracing::info!("[Maarch64 curl Passthrough] curl_easy_cleanup(handle={:?})", handle);

    if !handle.is_null() && (handle as u64) > 0x1_0000_0000 {
        if let Some(curl_lib) = get_curl_registry().get_library("libcurl.so.4") {
            unsafe {
                type EasyCleanupFn = unsafe extern "C" fn(*mut c_void);
                if let Ok(easy_cleanup) = curl_lib.get::<EasyCleanupFn>(b"curl_easy_cleanup\0") {
                    easy_cleanup(handle);
                }
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_curl_easy_reset(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let handle = ctx.get_x(0) as *mut c_void;

    if !handle.is_null() && (handle as u64) > 0x1_0000_0000 {
        if let Some(curl_lib) = get_curl_registry().get_library("libcurl.so.4") {
            unsafe {
                type EasyResetFn = unsafe extern "C" fn(*mut c_void);
                if let Ok(easy_reset) = curl_lib.get::<EasyResetFn>(b"curl_easy_reset\0") {
                    easy_reset(handle);
                }
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_curl_easy_setopt(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let handle = ctx.get_x(0) as *mut c_void;
    let option = ctx.get_x(1) as u32;
    let param = ctx.get_x(2);

    tracing::info!("[Maarch64 curl Passthrough] curl_easy_setopt(handle={:?}, option={}, param={:#x})", handle, option, param);

    if !handle.is_null() && (handle as u64) > 0x1_0000_0000 {
        if let Some(curl_lib) = get_curl_registry().get_library("libcurl.so.4") {
            unsafe {
                type EasySetoptStringFn = unsafe extern "C" fn(*mut c_void, u32, *const c_char) -> c_int;
                type EasySetoptLongFn = unsafe extern "C" fn(*mut c_void, u32, c_long) -> c_int;

                if option == 10002 { // CURLOPT_URL
                    if let Ok(url_str) = mem.read_string(param) {
                        if let Ok(c_str) = std::ffi::CString::new(url_str) {
                            if let Ok(setopt) = curl_lib.get::<EasySetoptStringFn>(b"curl_easy_setopt\0") {
                                let res = setopt(handle, option, c_str.as_ptr());
                                ctx.set_x(0, res as i64 as u64);
                                return Ok(());
                            }
                        }
                    }
                } else {
                    if let Ok(setopt) = curl_lib.get::<EasySetoptLongFn>(b"curl_easy_setopt\0") {
                        let res = setopt(handle, option, param as c_long);
                        ctx.set_x(0, res as i64 as u64);
                        return Ok(());
                    }
                }
            }
        }
    }

    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_curl_easy_perform(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let handle = ctx.get_x(0) as *mut c_void;
    tracing::info!("[Maarch64 curl Passthrough] curl_easy_perform(handle={:?})", handle);

    if !handle.is_null() && (handle as u64) > 0x1_0000_0000 {
        if let Some(curl_lib) = get_curl_registry().get_library("libcurl.so.4") {
            unsafe {
                type EasyPerformFn = unsafe extern "C" fn(*mut c_void) -> c_int;
                if let Ok(easy_perform) = curl_lib.get::<EasyPerformFn>(b"curl_easy_perform\0") {
                    let res = easy_perform(handle);
                    ctx.set_x(0, res as i64 as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0); // CURLE_OK
    Ok(())
}

pub fn thunk_curl_easy_strerror(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let code = ctx.get_x(0) as c_int;
    if let Some(curl_lib) = get_curl_registry().get_library("libcurl.so.4") {
        unsafe {
            type EasyStrerrorFn = unsafe extern "C" fn(c_int) -> *const c_char;
            if let Ok(easy_strerror) = curl_lib.get::<EasyStrerrorFn>(b"curl_easy_strerror\0") {
                let err_ptr = easy_strerror(code);
                ctx.set_x(0, err_ptr as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0x8888_0010);
    Ok(())
}

pub fn thunk_curl_slist_append(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let list = ctx.get_x(0) as *mut c_void;
    let str_ptr = ctx.get_x(1);

    if let Ok(str_vec) = mem.read_string(str_ptr) {
        if let Ok(c_str) = std::ffi::CString::new(str_vec) {
            if let Some(curl_lib) = get_curl_registry().get_library("libcurl.so.4") {
                unsafe {
                    type SlistAppendFn = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;
                    if let Ok(slist_append) = curl_lib.get::<SlistAppendFn>(b"curl_slist_append\0") {
                        let res_list = slist_append(list, c_str.as_ptr());
                        ctx.set_x(0, res_list as u64);
                        return Ok(());
                    }
                }
            }
        }
    }

    ctx.set_x(0, 0x8888_0020);
    Ok(())
}

pub fn thunk_curl_slist_free_all(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let list = ctx.get_x(0) as *mut c_void;
    if !list.is_null() && (list as u64) > 0x1_0000_0000 {
        if let Some(curl_lib) = get_curl_registry().get_library("libcurl.so.4") {
            unsafe {
                type SlistFreeAllFn = unsafe extern "C" fn(*mut c_void);
                if let Ok(slist_free) = curl_lib.get::<SlistFreeAllFn>(b"curl_slist_free_all\0") {
                    slist_free(list);
                }
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_OPENSSL_init_ssl(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 1);
    Ok(())
}

pub fn thunk_SSL_ctx_new(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0x8888_0030);
    Ok(())
}

pub fn thunk_SSL_new(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0x8888_0040);
    Ok(())
}

pub fn thunk_SSL_free(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn register_curl_thunks(map: &mut HashMap<String, super::ThunkFn>) {
    map.insert("curl_global_init".to_string(), thunk_curl_global_init);
    map.insert("curl_global_cleanup".to_string(), thunk_curl_global_cleanup);
    map.insert("curl_easy_init".to_string(), thunk_curl_easy_init);
    map.insert("curl_easy_cleanup".to_string(), thunk_curl_easy_cleanup);
    map.insert("curl_easy_reset".to_string(), thunk_curl_easy_reset);
    map.insert("curl_easy_setopt".to_string(), thunk_curl_easy_setopt);
    map.insert("curl_easy_perform".to_string(), thunk_curl_easy_perform);
    map.insert("curl_easy_strerror".to_string(), thunk_curl_easy_strerror);
    map.insert("curl_slist_append".to_string(), thunk_curl_slist_append);
    map.insert("curl_slist_free_all".to_string(), thunk_curl_slist_free_all);
    map.insert("OPENSSL_init_ssl".to_string(), thunk_OPENSSL_init_ssl);
    map.insert("SSL_ctx_new".to_string(), thunk_SSL_ctx_new);
    map.insert("SSL_new".to_string(), thunk_SSL_new);
    map.insert("SSL_free".to_string(), thunk_SSL_free);
}
