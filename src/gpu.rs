#![allow(non_snake_case)]

use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use std::collections::HashMap;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

pub struct GpuThunkRegistry {
    loaded_libraries: HashMap<String, libloading::Library>,
}

impl GpuThunkRegistry {
    pub fn new() -> Self {
        let mut loaded_libraries = HashMap::new();
        let libs_to_try = [
            ("libGL.so.1", "libGL.so"),
            ("libGLESv2.so.2", "libGLESv2.so"),
            ("libEGL.so.1", "libEGL.so"),
            ("libX11.so.6", "libX11.so"),
            ("libwayland-client.so.0", "libwayland-client.so"),
            ("libwayland-egl.so.1", "libwayland-egl.so"),
            ("libvulkan.so.1", "libvulkan.so"),
        ];

        for (name, alt_name) in libs_to_try {
            if let Ok(lib) = unsafe { libloading::Library::new(name) } {
                tracing::info!("Successfully loaded host GPU library: {}", name);
                loaded_libraries.insert(name.to_string(), lib);
            } else if let Ok(lib) = unsafe { libloading::Library::new(alt_name) } {
                tracing::info!("Successfully loaded host GPU library: {}", alt_name);
                loaded_libraries.insert(name.to_string(), lib);
            } else {
                tracing::debug!("Host GPU library {} not available in environment", name);
            }
        }

        Self { loaded_libraries }
    }

    pub fn get_library(&self, name: &str) -> Option<&libloading::Library> {
        self.loaded_libraries.get(name)
    }
}

static GPU_REGISTRY: std::sync::OnceLock<GpuThunkRegistry> = std::sync::OnceLock::new();
pub fn get_gpu_registry() -> &'static GpuThunkRegistry {
    GPU_REGISTRY.get_or_init(GpuThunkRegistry::new)
}

struct GlobalGpuState {
    host_x11_dpy: *mut std::ffi::c_void,
    host_window: u64,
    host_egl_dpy: *mut std::ffi::c_void,
    host_egl_config: *mut std::ffi::c_void,
    host_egl_ctx: *mut std::ffi::c_void,
    host_egl_surface: *mut std::ffi::c_void,
}

unsafe impl Send for GlobalGpuState {}
unsafe impl Sync for GlobalGpuState {}

static GPU_STATE: Mutex<GlobalGpuState> = Mutex::new(GlobalGpuState {
    host_x11_dpy: std::ptr::null_mut(),
    host_window: 0,
    host_egl_dpy: std::ptr::null_mut(),
    host_egl_config: std::ptr::null_mut(),
    host_egl_ctx: std::ptr::null_mut(),
    host_egl_surface: std::ptr::null_mut(),
});

fn write_gpu_string(mem: &mut MemoryManager, s: &str) -> u64 {
    let bytes = s.as_bytes();
    let alloc_len = bytes.len() + 1;
    let addr = mem.map_anonymous(0, alloc_len).unwrap_or(0x7f04_0000);
    let _ = mem.write(addr, bytes);
    addr
}

// ----------------------------------------------------------------------------
// Host X11 Forwarding Thunks
// ----------------------------------------------------------------------------
pub fn thunk_XOpenDisplay(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let name_ptr = ctx.get_x(0);
    let name = if name_ptr != 0 { mem.read_string(name_ptr).ok() } else { None };
    println!("[Maarch64 GPU Passthrough] XOpenDisplay(display={:?})", name);

    let registry = get_gpu_registry();
    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XOpenDisplayFn = unsafe extern "C" fn(*const std::os::raw::c_char) -> *mut std::ffi::c_void;
            if let Ok(open_dpy) = x11_lib.get::<XOpenDisplayFn>(b"XOpenDisplay\0") {
                let display_env = std::env::var("DISPLAY").unwrap_or_else(|_| ":0".to_string());
                let display_cstring = std::ffi::CString::new(display_env).unwrap();
                let c_ptr = if let Some(ref bytes) = name {
                    bytes.as_ptr() as *const _
                } else {
                    display_cstring.as_ptr()
                };
                let dpy = open_dpy(c_ptr);
                if !dpy.is_null() {
                    let mut state = GPU_STATE.lock().unwrap();
                    state.host_x11_dpy = dpy;
                    println!("[Maarch64 GPU Passthrough] Connected to Host X11 Display ({:p})", dpy);
                    ctx.set_x(0, dpy as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0x1000);
    Ok(())
}

pub fn thunk_XCloseDisplay(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    println!("[Maarch64 GPU Passthrough] XCloseDisplay(dpy={:#x})", dpy);
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XGetVisualInfo(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let vinfo_mask = ctx.get_x(1);
    let vinfo_template = ctx.get_x(2);
    let nitems_return = ctx.get_x(3);
    println!("[Maarch64 GPU Passthrough] XGetVisualInfo(mask={:#x})", vinfo_mask);

    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XGetVisualInfoFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64, *const u8, *mut i32) -> *mut u8;
            if let Ok(get_vinfo) = x11_lib.get::<XGetVisualInfoFn>(b"XGetVisualInfo\0") {
                let host_dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { dpy_ptr as *mut _ };
                let template_bytes = if vinfo_template != 0 {
                    mem.read(vinfo_template, 128).unwrap_or_else(|_| vec![0u8; 128])
                } else {
                    vec![0u8; 128]
                };
                let mut nitems = 0i32;
                let vinfo_ptr = get_vinfo(host_dpy, vinfo_mask, template_bytes.as_ptr(), &mut nitems);
                if !vinfo_ptr.is_null() {
                    if nitems_return != 0 { let _ = mem.write(nitems_return, &nitems.to_le_bytes()).unwrap_or_default(); }
                    ctx.set_x(0, vinfo_ptr as u64);
                    return Ok(());
                }
            }
        }
    }

    if nitems_return != 0 { let _ = mem.write(nitems_return, &1i32.to_le_bytes()).unwrap_or_default(); }
    let addr = mem.map_anonymous(0, 256).unwrap_or(0x7f05_0000);
    ctx.set_x(0, addr);
    Ok(())
}

pub fn thunk_XCreateWindow(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let width = ctx.get_x(4) as u32;
    let height = ctx.get_x(5) as u32;
    println!("[Maarch64 GPU Passthrough] XCreateWindow(w={}, h={})", width, height);

    let mut state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XOpenDisplayFn = unsafe extern "C" fn(*const std::os::raw::c_char) -> *mut std::ffi::c_void;
            type XDefaultScreenFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
            type XRootWindowFn = unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> u64;
            type XBlackPixelFn = unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> u64;
            type XCreateSimpleWindowFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64, i32, i32, u32, u32, u32, u64, u64) -> u64;
            type XStoreNameFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64, *const std::os::raw::c_char) -> i32;
            type XMapWindowFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64) -> i32;
            type XFlushFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;

            if let (Ok(open_dpy), Ok(default_screen), Ok(root_win), Ok(black_pixel), Ok(create_win), Ok(store_name), Ok(map_win), Ok(flush_dpy)) = (
                x11_lib.get::<XOpenDisplayFn>(b"XOpenDisplay\0"),
                x11_lib.get::<XDefaultScreenFn>(b"XDefaultScreen\0"),
                x11_lib.get::<XRootWindowFn>(b"XRootWindow\0"),
                x11_lib.get::<XBlackPixelFn>(b"XBlackPixel\0"),
                x11_lib.get::<XCreateSimpleWindowFn>(b"XCreateSimpleWindow\0"),
                x11_lib.get::<XStoreNameFn>(b"XStoreName\0"),
                x11_lib.get::<XMapWindowFn>(b"XMapWindow\0"),
                x11_lib.get::<XFlushFn>(b"XFlush\0"),
            ) {
                let dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { open_dpy(std::ptr::null()) };
                if !dpy.is_null() {
                    let scr = default_screen(dpy);
                    let root = root_win(dpy, scr);
                    let black = black_pixel(dpy, scr);
                    let win = create_win(dpy, root, 100, 100, if width > 0 { width } else { 800 }, if height > 0 { height } else { 600 }, 2, black, 0x003399FFu64);
                    store_name(dpy, win, "Maarch64 AArch64 3D GPU Window\0".as_ptr() as *const _);
                    map_win(dpy, win);
                    flush_dpy(dpy);

                    state.host_x11_dpy = dpy;
                    state.host_window = win;
                    println!("[Maarch64 GPU Passthrough] SUCCESS: Created Native X11 Window ID {:#x}", win);
                    ctx.set_x(0, win);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0x2000001);
    Ok(())
}

pub fn thunk_XCreateSimpleWindow(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    thunk_XCreateWindow(ctx, mem)
}

pub fn thunk_XMapWindow(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let win = ctx.get_x(1);
    println!("[Maarch64 GPU Input] XMapWindow(win={:#x})", win);

    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XMapWindowFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64) -> i32;
            type XFlushFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
            if let (Ok(map_win), Ok(flush_dpy)) = (
                x11_lib.get::<XMapWindowFn>(b"XMapWindow\0"),
                x11_lib.get::<XFlushFn>(b"XFlush\0"),
            ) {
                let host_dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { dpy_ptr as *mut _ };
                let target_win = if state.host_window != 0 { state.host_window } else { win };
                map_win(host_dpy, target_win);
                flush_dpy(host_dpy);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XDefaultScreen(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XDefaultScreenFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
            if let Ok(default_scr) = x11_lib.get::<XDefaultScreenFn>(b"XDefaultScreen\0") {
                let host_dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { dpy_ptr as *mut _ };
                let scr = default_scr(host_dpy);
                ctx.set_x(0, scr as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XRootWindow(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let scr = ctx.get_x(1) as i32;
    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XRootWindowFn = unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> u64;
            if let Ok(root_win) = x11_lib.get::<XRootWindowFn>(b"XRootWindow\0") {
                let host_dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { dpy_ptr as *mut _ };
                let win = root_win(host_dpy, scr);
                ctx.set_x(0, win);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0x100);
    Ok(())
}

pub fn thunk_XSelectInput(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let win = ctx.get_x(1);
    let mask = ctx.get_x(2);
    println!("[Maarch64 GPU Input] XSelectInput(win={:#x}, event_mask={:#x})", win, mask);

    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XSelectInputFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64, u64) -> i32;
            if let Ok(select_input) = x11_lib.get::<XSelectInputFn>(b"XSelectInput\0") {
                let host_dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { dpy_ptr as *mut _ };
                let target_win = if state.host_window != 0 { state.host_window } else { win };
                select_input(host_dpy, target_win, mask);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XFlush(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XFlushFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
            if let Ok(flush_dpy) = x11_lib.get::<XFlushFn>(b"XFlush\0") {
                let host_dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { dpy_ptr as *mut _ };
                flush_dpy(host_dpy);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XNextEvent(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let event_ptr = ctx.get_x(1);

    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XNextEventFn = unsafe extern "C" fn(*mut std::ffi::c_void, *mut u8) -> i32;
            if let Ok(next_event) = x11_lib.get::<XNextEventFn>(b"XNextEvent\0") {
                let host_dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { dpy_ptr as *mut _ };
                let mut host_event = [0u8; 192];
                next_event(host_dpy, host_event.as_mut_ptr());
                if event_ptr != 0 {
                    let _ = mem.write(event_ptr, &host_event);
                }
                ctx.set_x(0, 0);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XLookupString(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let event_ptr = ctx.get_x(0);
    let buf_ptr = ctx.get_x(1);
    let buf_bytes = ctx.get_x(2) as i32;
    let keysym_ptr = ctx.get_x(3);
    let status_ptr = ctx.get_x(4);

    let registry = get_gpu_registry();
    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XLookupStringFn = unsafe extern "C" fn(*mut u8, *mut u8, i32, *mut u64, *mut std::ffi::c_void) -> i32;
            if let Ok(lookup) = x11_lib.get::<XLookupStringFn>(b"XLookupString\0") {
                let ev_bytes = if event_ptr != 0 {
                    mem.read(event_ptr, 192).unwrap_or_else(|_| vec![0u8; 192])
                } else {
                    vec![0u8; 192]
                };
                let mut out_buf = vec![0u8; (buf_bytes as usize).max(32)];
                let mut keysym = 0u64;
                let count = lookup(ev_bytes.as_ptr() as *mut u8, out_buf.as_mut_ptr(), buf_bytes, &mut keysym, status_ptr as *mut _);
                if buf_ptr != 0 && count > 0 {
                    let _ = mem.write(buf_ptr, &out_buf[..count as usize]);
                }
                if keysym_ptr != 0 {
                    let _ = mem.write(keysym_ptr, &keysym.to_le_bytes());
                }
                ctx.set_x(0, count as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XCreateGC(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let win = ctx.get_x(1);
    let mask = ctx.get_x(2);
    let values_ptr = ctx.get_x(3);

    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XCreateGCFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64, u64, *const std::ffi::c_void) -> *mut std::ffi::c_void;
            if let Ok(create_gc) = x11_lib.get::<XCreateGCFn>(b"XCreateGC\0") {
                let host_dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { dpy_ptr as *mut _ };
                let target_win = if state.host_window != 0 { state.host_window } else { win };
                let gc = create_gc(host_dpy, target_win, mask, values_ptr as *const _);
                if !gc.is_null() {
                    ctx.set_x(0, gc as u64);
                    return Ok(());
                }
            }
        }
    }
    ctx.set_x(0, 0x5000);
    Ok(())
}

pub fn thunk_XSetForeground(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let gc_ptr = ctx.get_x(1);
    let fg = ctx.get_x(2);

    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XSetForegroundFn = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, u64) -> i32;
            if let Ok(set_fg) = x11_lib.get::<XSetForegroundFn>(b"XSetForeground\0") {
                let host_dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { dpy_ptr as *mut _ };
                set_fg(host_dpy, gc_ptr as *mut _, fg);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XFillRectangle(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let win = ctx.get_x(1);
    let gc_ptr = ctx.get_x(2);
    let x = ctx.get_x(3) as i32;
    let y = ctx.get_x(4) as i32;
    let w = ctx.get_x(5) as u32;
    let h = ctx.get_x(6) as u32;

    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XFillRectangleFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64, *mut std::ffi::c_void, i32, i32, u32, u32) -> i32;
            if let Ok(fill_rect) = x11_lib.get::<XFillRectangleFn>(b"XFillRectangle\0") {
                let host_dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { dpy_ptr as *mut _ };
                let target_win = if state.host_window != 0 { state.host_window } else { win };
                fill_rect(host_dpy, target_win, gc_ptr as *mut _, x, y, w, h);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XStoreName(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let win = ctx.get_x(1);
    let name_ptr = ctx.get_x(2);
    let name = if name_ptr != 0 { mem.read_string(name_ptr).ok() } else { None };

    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XStoreNameFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64, *const std::os::raw::c_char) -> i32;
            if let Ok(store_name) = x11_lib.get::<XStoreNameFn>(b"XStoreName\0") {
                let host_dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { dpy_ptr as *mut _ };
                let target_win = if state.host_window != 0 { state.host_window } else { win };
                let c_name = name.as_ref().map(|bytes| bytes.as_ptr() as *const std::os::raw::c_char).unwrap_or(std::ptr::null());
                store_name(host_dpy, target_win, c_name);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XSetStandardProperties(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XFree(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XPending(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

// ----------------------------------------------------------------------------
// EGL Passthrough Thunks
// ----------------------------------------------------------------------------
pub fn thunk_eglGetDisplay(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let display_id = ctx.get_x(0);
    println!("[Maarch64 GPU Passthrough] eglGetDisplay(native_display={:#x})", display_id);

    let mut state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(egl_lib) = registry.get_library("libEGL.so.1") {
        unsafe {
            type EglGetDisplayFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> *mut std::ffi::c_void;
            if let Ok(egl_get_dpy) = egl_lib.get::<EglGetDisplayFn>(b"eglGetDisplay\0") {
                let native_dpy = if !state.host_x11_dpy.is_null() { state.host_x11_dpy } else { display_id as *mut _ };
                let host_egl_dpy = egl_get_dpy(native_dpy);
                if !host_egl_dpy.is_null() {
                    state.host_egl_dpy = host_egl_dpy;
                    println!("[Maarch64 GPU Passthrough] Connected to Host EGLDisplay ({:p})", host_egl_dpy);
                    ctx.set_x(0, host_egl_dpy as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0x1000);
    Ok(())
}

pub fn thunk_eglInitialize(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let major_ptr = ctx.get_x(1);
    let minor_ptr = ctx.get_x(2);
    println!("[Maarch64 GPU Passthrough] eglInitialize(dpy={:#x})", dpy_ptr);

    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(egl_lib) = registry.get_library("libEGL.so.1") {
        unsafe {
            type EglInitializeFn = unsafe extern "C" fn(*mut std::ffi::c_void, *mut i32, *mut i32) -> i32;
            if let Ok(egl_init) = egl_lib.get::<EglInitializeFn>(b"eglInitialize\0") {
                let mut maj = 1i32;
                let mut min = 5i32;
                let host_dpy = if !state.host_egl_dpy.is_null() { state.host_egl_dpy } else { dpy_ptr as *mut _ };
                let res = egl_init(host_dpy, &mut maj, &mut min);
                if res != 0 {
                    if major_ptr != 0 { let _ = mem.write(major_ptr, &maj.to_le_bytes()); }
                    if minor_ptr != 0 { let _ = mem.write(minor_ptr, &min.to_le_bytes()); }
                    ctx.set_x(0, 1);
                    return Ok(());
                }
            }
        }
    }

    if major_ptr != 0 { let _ = mem.write(major_ptr, &1i32.to_le_bytes()); }
    if minor_ptr != 0 { let _ = mem.write(minor_ptr, &5i32.to_le_bytes()); }
    ctx.set_x(0, 1);
    Ok(())
}

pub fn thunk_eglBindAPI(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let api = ctx.get_x(0) as u32;
    println!("[Maarch64 GPU Passthrough] eglBindAPI(api={:#x})", api);

    let registry = get_gpu_registry();
    if let Some(egl_lib) = registry.get_library("libEGL.so.1") {
        unsafe {
            type EglBindAPIFn = unsafe extern "C" fn(u32) -> i32;
            if let Ok(egl_bind) = egl_lib.get::<EglBindAPIFn>(b"eglBindAPI\0") {
                let res = egl_bind(api);
                ctx.set_x(0, res as u64);
                return Ok(());
            }
        }
    }

    ctx.set_x(0, 1);
    Ok(())
}

pub fn thunk_eglChooseConfig(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let configs_ptr = ctx.get_x(2);
    let num_config_ptr = ctx.get_x(4);
    println!("[Maarch64 GPU Passthrough] eglChooseConfig(dpy={:#x})", dpy_ptr);

    let mut state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(egl_lib) = registry.get_library("libEGL.so.1") {
        unsafe {
            type EglChooseConfigFn = unsafe extern "C" fn(*mut std::ffi::c_void, *const i32, *mut *mut std::ffi::c_void, i32, *mut i32) -> i32;
            if let Ok(egl_choose) = egl_lib.get::<EglChooseConfigFn>(b"eglChooseConfig\0") {
                let attribs: [i32; 11] = [
                    0x3024, 8, // EGL_RED_SIZE
                    0x3023, 8, // EGL_GREEN_SIZE
                    0x3022, 8, // EGL_BLUE_SIZE
                    0x3021, 8, // EGL_ALPHA_SIZE
                    0x3040, 4, // EGL_RENDERABLE_TYPE = EGL_OPENGL_ES2_BIT
                    0x3038     // EGL_NONE
                ];
                let mut host_cfg: *mut std::ffi::c_void = std::ptr::null_mut();
                let mut num_cfg = 0i32;
                let host_dpy = if !state.host_egl_dpy.is_null() { state.host_egl_dpy } else { dpy_ptr as *mut _ };
                let res = egl_choose(host_dpy, attribs.as_ptr(), &mut host_cfg, 1, &mut num_cfg);
                if res != 0 && !host_cfg.is_null() {
                    state.host_egl_config = host_cfg;
                    if configs_ptr != 0 { let _ = mem.write(configs_ptr, &(host_cfg as u64).to_le_bytes()); }
                    if num_config_ptr != 0 { let _ = mem.write(num_config_ptr, &num_cfg.to_le_bytes()); }
                    ctx.set_x(0, 1);
                    return Ok(());
                }
            }
        }
    }

    if configs_ptr != 0 { let _ = mem.write(configs_ptr, &0x5000u64.to_le_bytes()); }
    if num_config_ptr != 0 { let _ = mem.write(num_config_ptr, &1i32.to_le_bytes()); }
    ctx.set_x(0, 1);
    Ok(())
}

pub fn thunk_eglCreateContext(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let cfg_ptr = ctx.get_x(1);
    println!("[Maarch64 GPU Passthrough] eglCreateContext(dpy={:#x}, cfg={:#x})", dpy_ptr, cfg_ptr);

    let mut state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(egl_lib) = registry.get_library("libEGL.so.1") {
        unsafe {
            type EglCreateContextFn = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, *mut std::ffi::c_void, *const i32) -> *mut std::ffi::c_void;
            if let Ok(egl_create_ctx) = egl_lib.get::<EglCreateContextFn>(b"eglCreateContext\0") {
                let attribs: [i32; 3] = [0x3098, 2, 0x3038]; // EGL_CONTEXT_CLIENT_VERSION = 2
                let host_dpy = if !state.host_egl_dpy.is_null() { state.host_egl_dpy } else { dpy_ptr as *mut _ };
                let host_cfg = if !state.host_egl_config.is_null() { state.host_egl_config } else { cfg_ptr as *mut _ };
                let host_ctx = egl_create_ctx(host_dpy, host_cfg, std::ptr::null_mut(), attribs.as_ptr());
                if !host_ctx.is_null() {
                    state.host_egl_ctx = host_ctx;
                    println!("[Maarch64 GPU Passthrough] SUCCESS: Created Host EGLContext ({:p})", host_ctx);
                    ctx.set_x(0, host_ctx as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0x6000);
    Ok(())
}

pub fn thunk_eglCreateWindowSurface(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let cfg_ptr = ctx.get_x(1);
    let win_handle = ctx.get_x(2);
    println!("[Maarch64 GPU Passthrough] eglCreateWindowSurface(dpy={:#x}, win={:#x})", dpy_ptr, win_handle);

    let mut state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(egl_lib) = registry.get_library("libEGL.so.1") {
        unsafe {
            type EglCreateWindowSurfaceFn = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, u64, *const i32) -> *mut std::ffi::c_void;
            if let Ok(create_surface) = egl_lib.get::<EglCreateWindowSurfaceFn>(b"eglCreateWindowSurface\0") {
                let host_dpy = if !state.host_egl_dpy.is_null() { state.host_egl_dpy } else { dpy_ptr as *mut _ };
                let host_cfg = if !state.host_egl_config.is_null() { state.host_egl_config } else { cfg_ptr as *mut _ };
                let target_win = if state.host_window != 0 { state.host_window } else { win_handle };
                let surface = create_surface(host_dpy, host_cfg, target_win, std::ptr::null());
                if !surface.is_null() {
                    state.host_egl_surface = surface;
                    println!("[Maarch64 GPU Passthrough] SUCCESS: Created Host EGLSurface ({:p})", surface);
                    ctx.set_x(0, surface as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0x7000);
    Ok(())
}

pub fn thunk_eglMakeCurrent(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let draw_ptr = ctx.get_x(1);
    let read_ptr = ctx.get_x(2);
    let ctx_ptr = ctx.get_x(3);
    println!("[Maarch64 GPU Passthrough] eglMakeCurrent(dpy={:#x}, ctx={:#x})", dpy_ptr, ctx_ptr);

    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(egl_lib) = registry.get_library("libEGL.so.1") {
        unsafe {
            type EglMakeCurrentFn = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, *mut std::ffi::c_void, *mut std::ffi::c_void) -> i32;
            if let Ok(make_current) = egl_lib.get::<EglMakeCurrentFn>(b"eglMakeCurrent\0") {
                let host_dpy = if !state.host_egl_dpy.is_null() { state.host_egl_dpy } else { dpy_ptr as *mut _ };
                let host_surf = if !state.host_egl_surface.is_null() { state.host_egl_surface } else { draw_ptr as *mut _ };
                let host_ctx = if !state.host_egl_ctx.is_null() { state.host_egl_ctx } else { ctx_ptr as *mut _ };
                let res = make_current(host_dpy, host_surf, host_surf, host_ctx);
                if res != 0 {
                    println!("[Maarch64 GPU Passthrough] Activated 3D Hardware Acceleration on Host GPU!");
                    ctx.set_x(0, 1);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 1);
    Ok(())
}

pub fn thunk_eglGetConfigAttrib(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let cfg_ptr = ctx.get_x(1);
    let attribute = ctx.get_x(2) as i32;
    let value_ptr = ctx.get_x(3);
    println!("[Maarch64 GPU Passthrough] eglGetConfigAttrib(attribute={:#x})", attribute);

    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(egl_lib) = registry.get_library("libEGL.so.1") {
        unsafe {
            type EglGetConfigAttribFn = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void, i32, *mut i32) -> i32;
            if let Ok(get_cfg_attrib) = egl_lib.get::<EglGetConfigAttribFn>(b"eglGetConfigAttrib\0") {
                let mut val = 0i32;
                let host_dpy = if !state.host_egl_dpy.is_null() { state.host_egl_dpy } else { dpy_ptr as *mut _ };
                let host_cfg = if !state.host_egl_config.is_null() { state.host_egl_config } else { cfg_ptr as *mut _ };
                let res = get_cfg_attrib(host_dpy, host_cfg, attribute, &mut val);
                if res != 0 {
                    if value_ptr != 0 { let _ = mem.write(value_ptr, &val.to_le_bytes()).unwrap_or_default(); }
                    ctx.set_x(0, 1);
                    return Ok(());
                }
            }
        }
    }

    if value_ptr != 0 { let _ = mem.write(value_ptr, &0i32.to_le_bytes()).unwrap_or_default(); }
    ctx.set_x(0, 1); // EGL_TRUE
    Ok(())
}

pub fn thunk_eglQueryString(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let name = ctx.get_x(1) as i32;
    println!("[Maarch64 GPU Passthrough] eglQueryString(dpy={:#x}, name={})", dpy, name);

    let str_val = match name {
        0x3053 => "Maarch64 EGL Thunk Engine\0", // EGL_VENDOR
        0x3054 => "1.5 Maarch64 GPU Passthrough\0", // EGL_VERSION
        0x3055 => "EGL_KHR_surfaceless_context EGL_EXT_platform_base\0", // EGL_EXTENSIONS
        _ => "EGL_DEFAULT\0",
    };

    let addr = write_gpu_string(mem, str_val);
    ctx.set_x(0, addr);
    Ok(())
}

pub fn thunk_eglSwapBuffers(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy_ptr = ctx.get_x(0);
    let surface_ptr = ctx.get_x(1);
    println!("[Maarch64 GPU Passthrough] eglSwapBuffers -> Rendering Frame to Screen");

    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();

    if let Some(egl_lib) = registry.get_library("libEGL.so.1") {
        unsafe {
            type EglSwapBuffersFn = unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void) -> i32;
            if let Ok(swap_buf) = egl_lib.get::<EglSwapBuffersFn>(b"eglSwapBuffers\0") {
                let host_dpy = if !state.host_egl_dpy.is_null() { state.host_egl_dpy } else { dpy_ptr as *mut _ };
                let host_surf = if !state.host_egl_surface.is_null() { state.host_egl_surface } else { surface_ptr as *mut _ };
                swap_buf(host_dpy, host_surf);
            }
        }
    }

    flush_and_hold_native_window(5);
    ctx.set_x(0, 1);
    Ok(())
}

// ----------------------------------------------------------------------------
// OpenGL / GLES Core Thunks
// ----------------------------------------------------------------------------
pub fn thunk_glGetString(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let name = ctx.get_x(0) as u32;
    println!("[Maarch64 GPU Passthrough] glGetString(name={:#x})", name);

    let str_val = match name {
        0x1F00 => "Maarch64 Project\0", // GL_VENDOR
        0x1F01 => "Maarch64 GPU Thunk Passthrough (x86_64)\0", // GL_RENDERER
        0x1F02 => "OpenGL ES 3.2 Maarch64\0", // GL_VERSION
        0x1F03 => "GL_EXT_texture_format_BGRA8888\0", // GL_EXTENSIONS
        _ => "OpenGL ES 3.2\0",
    };

    let addr = write_gpu_string(mem, str_val);
    ctx.set_x(0, addr);
    Ok(())
}

pub fn thunk_glClearColor(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let r = f32::from_bits(ctx.get_x(0) as u32);
    let g = f32::from_bits(ctx.get_x(1) as u32);
    let b = f32::from_bits(ctx.get_x(2) as u32);
    let a = f32::from_bits(ctx.get_x(3) as u32);
    println!("[Maarch64 GPU Passthrough] glClearColor(r={}, g={}, b={}, a={})", r, g, b, a);

    let registry = get_gpu_registry();
    if let Some(gles_lib) = registry.get_library("libGLESv2.so.2") {
        unsafe {
            type GlClearColorFn = unsafe extern "C" fn(f32, f32, f32, f32);
            if let Ok(clear_color) = gles_lib.get::<GlClearColorFn>(b"glClearColor\0") {
                clear_color(r, g, b, a);
            }
        }
    }
    Ok(())
}

pub fn thunk_glClear(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let mask = ctx.get_x(0) as u32;
    println!("[Maarch64 GPU Passthrough] glClear(mask={:#x})", mask);

    let registry = get_gpu_registry();
    if let Some(gles_lib) = registry.get_library("libGLESv2.so.2") {
        unsafe {
            type GlClearFn = unsafe extern "C" fn(u32);
            if let Ok(clear_fn) = gles_lib.get::<GlClearFn>(b"glClear\0") {
                clear_fn(mask);
            }
        }
    }
    flush_and_hold_native_window(5);
    Ok(())
}

pub fn thunk_glViewport(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let x = ctx.get_x(0) as i32;
    let y = ctx.get_x(1) as i32;
    let w = ctx.get_x(2) as i32;
    let h = ctx.get_x(3) as i32;
    println!("[Maarch64 GPU Passthrough] glViewport(x={}, y={}, w={}, h={})", x, y, w, h);

    let registry = get_gpu_registry();
    if let Some(gles_lib) = registry.get_library("libGLESv2.so.2") {
        unsafe {
            type GlViewportFn = unsafe extern "C" fn(i32, i32, i32, i32);
            if let Ok(vp_fn) = gles_lib.get::<GlViewportFn>(b"glViewport\0") {
                vp_fn(x, y, w, h);
            }
        }
    }
    Ok(())
}

pub fn thunk_glDrawArrays(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let mode = ctx.get_x(0) as u32;
    let first = ctx.get_x(1) as i32;
    let count = ctx.get_x(2) as i32;
    println!("[Maarch64 GPU Passthrough] glDrawArrays(mode={:#x}, first={}, count={})", mode, first, count);

    let registry = get_gpu_registry();
    if let Some(gles_lib) = registry.get_library("libGLESv2.so.2") {
        unsafe {
            type GlDrawArraysFn = unsafe extern "C" fn(u32, i32, i32);
            if let Ok(draw_fn) = gles_lib.get::<GlDrawArraysFn>(b"glDrawArrays\0") {
                draw_fn(mode, first, count);
            }
        }
    }
    Ok(())
}

pub fn thunk_glFinish(_ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    println!("[Maarch64 GPU Passthrough] glFinish()");
    Ok(())
}

pub fn thunk_glFlush(_ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    println!("[Maarch64 GPU Passthrough] glFlush()");
    Ok(())
}

pub fn thunk_wl_display_connect(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let name_ptr = ctx.get_x(0);
    println!("[Maarch64 GPU Passthrough] wl_display_connect(name_ptr={:#x})", name_ptr);
    ctx.set_x(0, 0x2000);
    Ok(())
}

pub fn thunk_wl_egl_window_create(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let surface = ctx.get_x(0);
    let width = ctx.get_x(1) as i32;
    let height = ctx.get_x(2) as i32;
    println!("[Maarch64 GPU Passthrough] wl_egl_window_create(surface={:#x}, w={}, h={})", surface, width, height);
    ctx.set_x(0, 0x3000);
    Ok(())
}

fn flush_and_hold_native_window(duration_secs: u64) {
    let state = GPU_STATE.lock().unwrap();
    let registry = get_gpu_registry();
    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XFlushFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
            type XPendingFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
            type XNextEventFn = unsafe extern "C" fn(*mut std::ffi::c_void, *mut u8) -> i32;

            if let (Ok(flush_dpy), Ok(pending_events), Ok(next_event)) = (
                x11_lib.get::<XFlushFn>(b"XFlush\0"),
                x11_lib.get::<XPendingFn>(b"XPending\0"),
                x11_lib.get::<XNextEventFn>(b"XNextEvent\0"),
            ) {
                if !state.host_x11_dpy.is_null() {
                    flush_dpy(state.host_x11_dpy);
                    let mut event_buf = [0u8; 192];
                    
                    let steps = duration_secs * 10;
                    for i in 0..steps {
                        while pending_events(state.host_x11_dpy) > 0 {
                            next_event(state.host_x11_dpy, event_buf.as_mut_ptr());
                        }
                        flush_dpy(state.host_x11_dpy);
                        thread::sleep(Duration::from_millis(100));
                        if i % 10 == 0 && i > 0 {
                            println!("[Maarch64 GPU Passthrough] Window active on desktop... ({}s remaining)", duration_secs - (i / 10));
                        }
                    }
                }
            }
        }
    }
}

pub fn register_gpu_thunks(thunks: &mut HashMap<String, crate::ThunkFn>) {
    let _registry = get_gpu_registry();

    // X11 Thunks
    thunks.insert("XOpenDisplay".to_string(), thunk_XOpenDisplay);
    thunks.insert("XCloseDisplay".to_string(), thunk_XCloseDisplay);
    thunks.insert("XGetVisualInfo".to_string(), thunk_XGetVisualInfo);
    thunks.insert("XCreateWindow".to_string(), thunk_XCreateWindow);
    thunks.insert("XCreateSimpleWindow".to_string(), thunk_XCreateSimpleWindow);
    thunks.insert("XMapWindow".to_string(), thunk_XMapWindow);
    thunks.insert("XSetStandardProperties".to_string(), thunk_XSetStandardProperties);
    thunks.insert("XFree".to_string(), thunk_XFree);
    thunks.insert("XPending".to_string(), thunk_XPending);
    thunks.insert("XDefaultScreen".to_string(), thunk_XDefaultScreen);
    thunks.insert("XRootWindow".to_string(), thunk_XRootWindow);
    thunks.insert("XSelectInput".to_string(), thunk_XSelectInput);
    thunks.insert("XFlush".to_string(), thunk_XFlush);
    thunks.insert("XNextEvent".to_string(), thunk_XNextEvent);
    thunks.insert("XLookupString".to_string(), thunk_XLookupString);
    thunks.insert("XCreateGC".to_string(), thunk_XCreateGC);
    thunks.insert("XSetForeground".to_string(), thunk_XSetForeground);
    thunks.insert("XFillRectangle".to_string(), thunk_XFillRectangle);
    thunks.insert("XStoreName".to_string(), thunk_XStoreName);

    // EGL Thunks
    thunks.insert("eglGetDisplay".to_string(), thunk_eglGetDisplay);
    thunks.insert("eglInitialize".to_string(), thunk_eglInitialize);
    thunks.insert("eglBindAPI".to_string(), thunk_eglBindAPI);
    thunks.insert("eglChooseConfig".to_string(), thunk_eglChooseConfig);
    thunks.insert("eglGetConfigAttrib".to_string(), thunk_eglGetConfigAttrib);
    thunks.insert("eglCreateContext".to_string(), thunk_eglCreateContext);
    thunks.insert("eglCreateWindowSurface".to_string(), thunk_eglCreateWindowSurface);
    thunks.insert("eglMakeCurrent".to_string(), thunk_eglMakeCurrent);
    thunks.insert("eglQueryString".to_string(), thunk_eglQueryString);
    thunks.insert("eglSwapBuffers".to_string(), thunk_eglSwapBuffers);

    // OpenGL ES Core Thunks
    thunks.insert("glGetString".to_string(), thunk_glGetString);
    thunks.insert("glClearColor".to_string(), thunk_glClearColor);
    thunks.insert("glClear".to_string(), thunk_glClear);
    thunks.insert("glViewport".to_string(), thunk_glViewport);
    thunks.insert("glDrawArrays".to_string(), thunk_glDrawArrays);
    thunks.insert("glFinish".to_string(), thunk_glFinish);
    thunks.insert("glFlush".to_string(), thunk_glFlush);

    // Wayland Thunks
    thunks.insert("wl_display_connect".to_string(), thunk_wl_display_connect);
    thunks.insert("wl_egl_window_create".to_string(), thunk_wl_egl_window_create);
}
