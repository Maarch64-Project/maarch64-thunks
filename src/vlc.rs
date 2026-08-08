#![allow(non_snake_case)]

use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use std::collections::HashMap;
use std::sync::atomic::{AtomicPtr, Ordering};

static LAST_VLC_INSTANCE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

pub struct VlcRegistry {
    loaded_libraries: HashMap<String, libloading::Library>,
}

impl VlcRegistry {
    pub fn new() -> Self {
        let mut loaded_libraries = HashMap::new();
        let libs_to_try = [
            ("libvlc.so.5", "libvlc.so"),
            ("libvlccore.so.9", "libvlccore.so"),
        ];

        for (name, alt_name) in libs_to_try {
            if let Ok(lib) = unsafe { libloading::Library::new(name) } {
                tracing::info!("[Maarch64 VLC Passthrough] Successfully loaded host VLC library: {}", name);
                loaded_libraries.insert(name.to_string(), lib);
            } else if let Ok(lib) = unsafe { libloading::Library::new(alt_name) } {
                tracing::info!("[Maarch64 VLC Passthrough] Successfully loaded host VLC library: {}", alt_name);
                loaded_libraries.insert(name.to_string(), lib);
            }
        }

        Self { loaded_libraries }
    }

    pub fn get_library(&self, name: &str) -> Option<&libloading::Library> {
        self.loaded_libraries.get(name)
    }
}

static VLC_REGISTRY: std::sync::OnceLock<VlcRegistry> = std::sync::OnceLock::new();
pub fn get_vlc_registry() -> &'static VlcRegistry {
    VLC_REGISTRY.get_or_init(VlcRegistry::new)
}

pub fn get_active_vlc_instance() -> *mut std::ffi::c_void {
    LAST_VLC_INSTANCE.load(Ordering::SeqCst)
}

pub fn thunk_libvlc_new(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let argc = ctx.get_x(0) as i32;
    let argv_ptr = ctx.get_x(1);

    tracing::info!("[Maarch64 VLC Passthrough] libvlc_new(argc={})", argc);

    let registry = get_vlc_registry();
    if let Some(vlc_lib) = registry.get_library("libvlc.so.5") {
        unsafe {
            type LibVlcNewFn = unsafe extern "C" fn(i32, *const *const std::os::raw::c_char) -> *mut std::ffi::c_void;
            if let Ok(vlc_new) = vlc_lib.get::<LibVlcNewFn>(b"libvlc_new\0") {
                let mut c_args: Vec<std::ffi::CString> = Vec::new();
                let mut c_ptrs: Vec<*const std::os::raw::c_char> = Vec::new();

                for i in 0..argc {
                    let ptr_addr = argv_ptr + (i as u64) * 8;
                    if let Ok(arg_addr_bytes) = mem.read(ptr_addr, 8) {
                        let arg_addr = u64::from_le_bytes(arg_addr_bytes.try_into().unwrap());
                        if let Ok(str_bytes) = mem.read_string(arg_addr) {
                            if let Ok(cstr) = std::ffi::CString::new(str_bytes) {
                                c_ptrs.push(cstr.as_ptr());
                                c_args.push(cstr);
                            }
                        }
                    }
                }

                let handle = vlc_new(c_ptrs.len() as i32, c_ptrs.as_ptr());
                if !handle.is_null() {
                    LAST_VLC_INSTANCE.store(handle, Ordering::SeqCst);
                    tracing::info!("[Maarch64 VLC Passthrough] SUCCESS: Created Host libvlc Instance ({:p})", handle);
                    ctx.set_x(0, handle as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0x6000);
    Ok(())
}

pub fn thunk_libvlc_release(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let inst_ptr = ctx.get_x(0);
    let registry = get_vlc_registry();
    if let Some(vlc_lib) = registry.get_library("libvlc.so.5") {
        unsafe {
            type LibVlcReleaseFn = unsafe extern "C" fn(*mut std::ffi::c_void);
            if let Ok(vlc_rel) = vlc_lib.get::<LibVlcReleaseFn>(b"libvlc_release\0") {
                if inst_ptr != 0 && inst_ptr != 0x6000 {
                    vlc_rel(inst_ptr as *mut _);
                }
            }
        }
    }
    LAST_VLC_INSTANCE.store(std::ptr::null_mut(), Ordering::SeqCst);
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_libvlc_get_version(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let registry = get_vlc_registry();
    if let Some(vlc_lib) = registry.get_library("libvlc.so.5") {
        unsafe {
            type LibVlcGetVersionFn = unsafe extern "C" fn() -> *const std::os::raw::c_char;
            if let Ok(vlc_ver) = vlc_lib.get::<LibVlcGetVersionFn>(b"libvlc_get_version\0") {
                let ver_ptr = vlc_ver();
                ctx.set_x(0, ver_ptr as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_libvlc_get_changeset(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let registry = get_vlc_registry();
    if let Some(vlc_lib) = registry.get_library("libvlc.so.5") {
        unsafe {
            type LibVlcGetChangesetFn = unsafe extern "C" fn() -> *const std::os::raw::c_char;
            if let Ok(vlc_cs) = vlc_lib.get::<LibVlcGetChangesetFn>(b"libvlc_get_changeset\0") {
                let cs_ptr = vlc_cs();
                ctx.set_x(0, cs_ptr as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_libvlc_set_user_agent(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let inst_ptr = ctx.get_x(0);
    let name_ptr = ctx.get_x(1);
    let http_ptr = ctx.get_x(2);

    let name = if name_ptr != 0 { mem.read_string(name_ptr).ok() } else { None };
    let http = if http_ptr != 0 { mem.read_string(http_ptr).ok() } else { None };

    let registry = get_vlc_registry();
    if let Some(vlc_lib) = registry.get_library("libvlc.so.5") {
        unsafe {
            type LibVlcSetUserAgentFn = unsafe extern "C" fn(*mut std::ffi::c_void, *const std::os::raw::c_char, *const std::os::raw::c_char);
            if let Ok(set_ua) = vlc_lib.get::<LibVlcSetUserAgentFn>(b"libvlc_set_user_agent\0") {
                let c_name = name.map(|s| std::ffi::CString::new(s).unwrap());
                let c_http = http.map(|s| std::ffi::CString::new(s).unwrap());
                if inst_ptr != 0 && inst_ptr != 0x6000 {
                    set_ua(
                        inst_ptr as *mut _,
                        c_name.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                        c_http.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                    );
                }
            }
        }
    }

    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_libvlc_set_app_id(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let inst_ptr = ctx.get_x(0);
    let id_ptr = ctx.get_x(1);
    let ver_ptr = ctx.get_x(2);
    let icon_ptr = ctx.get_x(3);

    let id = if id_ptr != 0 { mem.read_string(id_ptr).ok() } else { None };
    let ver = if ver_ptr != 0 { mem.read_string(ver_ptr).ok() } else { None };
    let icon = if icon_ptr != 0 { mem.read_string(icon_ptr).ok() } else { None };

    let registry = get_vlc_registry();
    if let Some(vlc_lib) = registry.get_library("libvlc.so.5") {
        unsafe {
            type LibVlcSetAppIdFn = unsafe extern "C" fn(*mut std::ffi::c_void, *const std::os::raw::c_char, *const std::os::raw::c_char, *const std::os::raw::c_char);
            if let Ok(set_app) = vlc_lib.get::<LibVlcSetAppIdFn>(b"libvlc_set_app_id\0") {
                let c_id = id.map(|s| std::ffi::CString::new(s).unwrap());
                let c_ver = ver.map(|s| std::ffi::CString::new(s).unwrap());
                let c_icon = icon.map(|s| std::ffi::CString::new(s).unwrap());
                if inst_ptr != 0 && inst_ptr != 0x6000 {
                    set_app(
                        inst_ptr as *mut _,
                        c_id.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                        c_ver.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                        c_icon.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                    );
                }
            }
        }
    }

    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_libvlc_add_intf(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let inst_ptr = ctx.get_x(0);
    let name_ptr = ctx.get_x(1);
    let name = if name_ptr != 0 { mem.read_string(name_ptr).ok() } else { None };

    tracing::info!("[Maarch64 VLC Passthrough] libvlc_add_intf(name={:?})", name);

    let registry = get_vlc_registry();
    if let Some(vlc_lib) = registry.get_library("libvlc.so.5") {
        unsafe {
            type LibVlcAddIntfFn = unsafe extern "C" fn(*mut std::ffi::c_void, *const std::os::raw::c_char) -> i32;
            if let Ok(add_intf) = vlc_lib.get::<LibVlcAddIntfFn>(b"libvlc_add_intf\0") {
                let c_name = name.map(|s| std::ffi::CString::new(s).unwrap());
                if inst_ptr != 0 && inst_ptr != 0x6000 {
                    let ret = add_intf(inst_ptr as *mut _, c_name.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null()));
                    ctx.set_x(0, ret as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_libvlc_playlist_play(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[Maarch64 VLC Passthrough] libvlc_playlist_play()");
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_libvlc_wait(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let target_ptr = LAST_VLC_INSTANCE.load(Ordering::SeqCst);

    tracing::info!("[Maarch64 VLC Passthrough] libvlc_wait(instance={:p})", target_ptr);
    if !target_ptr.is_null() {
        let registry = get_vlc_registry();
        if let Some(vlc_lib) = registry.get_library("libvlc.so.5") {
            unsafe {
                type LibVlcWaitFn = unsafe extern "C" fn(*mut std::ffi::c_void);
                if let Ok(vlc_wait) = vlc_lib.get::<LibVlcWaitFn>(b"libvlc_wait\0") {
                    vlc_wait(target_ptr);
                    tracing::info!("[Maarch64 VLC Passthrough] Host VLC window closed cleanly.");
                    ctx.set_x(0, 0);
                    return Ok(());
                }
            }
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
    ctx.set_x(0, 0);
    Ok(())
}

pub fn register_vlc_thunks(thunks: &mut HashMap<String, crate::ThunkFn>) {

    thunks.insert("libvlc_new".to_string(), thunk_libvlc_new);
    thunks.insert("libvlc_release".to_string(), thunk_libvlc_release);
    thunks.insert("libvlc_get_version".to_string(), thunk_libvlc_get_version);
    thunks.insert("libvlc_get_changeset".to_string(), thunk_libvlc_get_changeset);
    thunks.insert("libvlc_set_user_agent".to_string(), thunk_libvlc_set_user_agent);
    thunks.insert("libvlc_set_app_id".to_string(), thunk_libvlc_set_app_id);
    thunks.insert("libvlc_add_intf".to_string(), thunk_libvlc_add_intf);
    thunks.insert("libvlc_playlist_play".to_string(), thunk_libvlc_playlist_play);
    thunks.insert("libvlc_wait".to_string(), thunk_libvlc_wait);
}
