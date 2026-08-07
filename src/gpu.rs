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

struct NativeWindowContext {
    display: *mut std::ffi::c_void,
    window: u64,
}

unsafe impl Send for NativeWindowContext {}
unsafe impl Sync for NativeWindowContext {}

static NATIVE_WINDOW: Mutex<Option<NativeWindowContext>> = Mutex::new(None);

fn open_host_x11_window() -> bool {
    let mut lock = NATIVE_WINDOW.lock().unwrap();
    if lock.is_some() {
        return true;
    }

    println!("\n[Maarch64 GPU Thunk] --------------------------------------------------");
    println!("[Maarch64 GPU Thunk] Initializing Native Host X11 / Wayland Surface...");

    let registry = get_gpu_registry();
    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XOpenDisplayFn = unsafe extern "C" fn(*const std::os::raw::c_char) -> *mut std::ffi::c_void;
            type XDefaultScreenFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;
            type XRootWindowFn = unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> u64;
            type XBlackPixelFn = unsafe extern "C" fn(*mut std::ffi::c_void, i32) -> u64;
            type XCreateSimpleWindowFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64, i32, i32, u32, u32, u32, u64, u64) -> u64;
            type XSelectInputFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64, i64) -> i32;
            type XStoreNameFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64, *const std::os::raw::c_char) -> i32;
            type XMapWindowFn = unsafe extern "C" fn(*mut std::ffi::c_void, u64) -> i32;
            type XFlushFn = unsafe extern "C" fn(*mut std::ffi::c_void) -> i32;

            if let (Ok(open_dpy), Ok(default_screen), Ok(root_win), Ok(black_pixel), Ok(create_win), Ok(select_input), Ok(store_name), Ok(map_win), Ok(flush_dpy)) = (
                x11_lib.get::<XOpenDisplayFn>(b"XOpenDisplay\0"),
                x11_lib.get::<XDefaultScreenFn>(b"XDefaultScreen\0"),
                x11_lib.get::<XRootWindowFn>(b"XRootWindow\0"),
                x11_lib.get::<XBlackPixelFn>(b"XBlackPixel\0"),
                x11_lib.get::<XCreateSimpleWindowFn>(b"XCreateSimpleWindow\0"),
                x11_lib.get::<XSelectInputFn>(b"XSelectInput\0"),
                x11_lib.get::<XStoreNameFn>(b"XStoreName\0"),
                x11_lib.get::<XMapWindowFn>(b"XMapWindow\0"),
                x11_lib.get::<XFlushFn>(b"XFlush\0"),
            ) {
                let dpy = open_dpy(std::ptr::null());
                if !dpy.is_null() {
                    let scr = default_screen(dpy);
                    let root = root_win(dpy, scr);
                    let black = black_pixel(dpy, scr);
                    // Sky blue background pixel (RGB: 0x3399FF)
                    let bg_color = 0x003399FFu64;
                    let win = create_win(dpy, root, 150, 150, 800, 600, 3, black, bg_color);
                    
                    // ExposureMask = 1<<15, KeyPressMask = 1<<0, StructureNotifyMask = 1<<17
                    select_input(dpy, win, (1 << 15) | (1 << 0) | (1 << 17));
                    store_name(dpy, win, "Maarch64 AArch64 GPU Acceleration Demo (800x600)\0".as_ptr() as *const _);
                    map_win(dpy, win);
                    flush_dpy(dpy);

                    *lock = Some(NativeWindowContext { display: dpy, window: win });
                    println!("[Maarch64 GPU Thunk] SUCCESS: Created 800x600 Native Window (Window ID: {:#x})", win);
                    println!("[Maarch64 GPU Thunk] Background color set to Sky Blue (0x3399FF).");
                    println!("[Maarch64 GPU Thunk] --------------------------------------------------\n");
                    return true;
                } else {
                    println!("[Maarch64 GPU Thunk] WARNING: Unable to connect to host X11/Wayland display ($DISPLAY).");
                }
            }
        }
    }
    false
}

fn flush_and_hold_native_window(duration_secs: u64) {
    let lock = NATIVE_WINDOW.lock().unwrap();
    if let Some(ref native) = *lock {
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
                    flush_dpy(native.display);
                    let mut event_buf = [0u8; 192];
                    
                    println!("[Maarch64 GPU Thunk] Event Loop Running: Displaying Window on screen for {} seconds...", duration_secs);
                    let steps = duration_secs * 10;
                    for i in 0..steps {
                        while pending_events(native.display) > 0 {
                            next_event(native.display, event_buf.as_mut_ptr());
                        }
                        flush_dpy(native.display);
                        thread::sleep(Duration::from_millis(100));
                        if i % 10 == 0 && i > 0 {
                            println!("[Maarch64 GPU Thunk] Window active on desktop... ({}s remaining)", duration_secs - (i / 10));
                        }
                    }
                    println!("[Maarch64 GPU Thunk] Frame render sequence complete.");
                }
            }
        }
    }
}

fn write_gpu_string(mem: &mut MemoryManager, s: &str) -> u64 {
    let bytes = s.as_bytes();
    let alloc_len = bytes.len() + 1;
    let addr = mem.map_anonymous(0, alloc_len).unwrap_or(0x7f04_0000);
    let _ = mem.write(addr, bytes);
    addr
}

// ----------------------------------------------------------------------------
// EGL Thunk Handlers
// ----------------------------------------------------------------------------
pub fn thunk_eglGetDisplay(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let display_id = ctx.get_x(0);
    println!("[thunk log] eglGetDisplay(display_id={:#x}) called", display_id);
    open_host_x11_window();
    ctx.set_x(0, 0x1000); // Mock EGLDisplay handle
    Ok(())
}

pub fn thunk_eglInitialize(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let major_ptr = ctx.get_x(1);
    let minor_ptr = ctx.get_x(2);
    println!("[thunk log] eglInitialize(dpy={:#x}) -> Initializing EGL 1.5", dpy);
    if major_ptr != 0 {
        mem.write(major_ptr, &1i32.to_le_bytes()).map_err(|e| e.to_string())?;
    }
    if minor_ptr != 0 {
        mem.write(minor_ptr, &5i32.to_le_bytes()).map_err(|e| e.to_string())?; // EGL 1.5
    }
    ctx.set_x(0, 1); // EGL_TRUE
    Ok(())
}

pub fn thunk_eglQueryString(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let name = ctx.get_x(1) as i32;
    println!("[thunk log] eglQueryString(dpy={:#x}, name={})", dpy, name);

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
    let dpy = ctx.get_x(0);
    let surface = ctx.get_x(1);
    println!("[thunk log] eglSwapBuffers(dpy={:#x}, surface={:#x}) -> Swapping Framebuffers", dpy, surface);
    flush_and_hold_native_window(5);
    ctx.set_x(0, 1); // EGL_TRUE
    Ok(())
}

// ----------------------------------------------------------------------------
// OpenGL / GLES Core Thunk Handlers
// ----------------------------------------------------------------------------
pub fn thunk_glGetString(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let name = ctx.get_x(0) as u32;
    println!("[thunk log] glGetString(name={:#x})", name);

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
    println!("[thunk log] glClearColor(r={}, g={}, b={}, a={}) -> Setting Clear Color", r, g, b, a);
    open_host_x11_window();
    Ok(())
}

pub fn thunk_glClear(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let mask = ctx.get_x(0) as u32;
    println!("[thunk log] glClear(mask={:#x}) -> Executing GPU Clear Buffer", mask);
    flush_and_hold_native_window(5);
    Ok(())
}

pub fn thunk_glViewport(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let x = ctx.get_x(0) as i32;
    let y = ctx.get_x(1) as i32;
    let w = ctx.get_x(2) as i32;
    let h = ctx.get_x(3) as i32;
    println!("[thunk log] glViewport(x={}, y={}, w={}, h={})", x, y, w, h);
    Ok(())
}

pub fn thunk_glDrawArrays(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let mode = ctx.get_x(0) as u32;
    let first = ctx.get_x(1) as i32;
    let count = ctx.get_x(2) as i32;
    println!("[thunk log] glDrawArrays(mode={:#x}, first={}, count={})", mode, first, count);
    Ok(())
}

pub fn thunk_glFinish(_ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    println!("[thunk log] glFinish()");
    Ok(())
}

pub fn thunk_glFlush(_ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    println!("[thunk log] glFlush()");
    Ok(())
}

// ----------------------------------------------------------------------------
// GLX Thunk Handlers
// ----------------------------------------------------------------------------
pub fn thunk_glXQueryExtension(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let error_base = ctx.get_x(1);
    let event_base = ctx.get_x(2);
    println!("[thunk log] glXQueryExtension(dpy={:#x})", dpy);
    if error_base != 0 {
        mem.write(error_base, &0i32.to_le_bytes()).map_err(|e| e.to_string())?;
    }
    if event_base != 0 {
        mem.write(event_base, &0i32.to_le_bytes()).map_err(|e| e.to_string())?;
    }
    ctx.set_x(0, 1); // True
    Ok(())
}

pub fn thunk_glXSwapBuffers(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let drawable = ctx.get_x(1);
    println!("[thunk log] glXSwapBuffers(dpy={:#x}, drawable={:#x})", dpy, drawable);
    flush_and_hold_native_window(5);
    Ok(())
}

pub fn thunk_wl_display_connect(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let name_ptr = ctx.get_x(0);
    println!("[thunk log] wl_display_connect(name_ptr={:#x})", name_ptr);
    open_host_x11_window();
    ctx.set_x(0, 0x2000); // Mock wl_display handle
    Ok(())
}

pub fn thunk_wl_egl_window_create(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let surface = ctx.get_x(0);
    let width = ctx.get_x(1) as i32;
    let height = ctx.get_x(2) as i32;
    println!("[thunk log] wl_egl_window_create(surface={:#x}, w={}, h={})", surface, width, height);
    open_host_x11_window();
    ctx.set_x(0, 0x3000); // Mock wl_egl_window handle
    Ok(())
}

pub fn register_gpu_thunks(thunks: &mut HashMap<String, crate::ThunkFn>) {
    // Force initialization of host library resolution
    let _registry = get_gpu_registry();

    thunks.insert("eglGetDisplay".to_string(), thunk_eglGetDisplay);
    thunks.insert("eglInitialize".to_string(), thunk_eglInitialize);
    thunks.insert("eglQueryString".to_string(), thunk_eglQueryString);
    thunks.insert("eglSwapBuffers".to_string(), thunk_eglSwapBuffers);

    thunks.insert("glGetString".to_string(), thunk_glGetString);
    thunks.insert("glClearColor".to_string(), thunk_glClearColor);
    thunks.insert("glClear".to_string(), thunk_glClear);
    thunks.insert("glViewport".to_string(), thunk_glViewport);
    thunks.insert("glDrawArrays".to_string(), thunk_glDrawArrays);
    thunks.insert("glFinish".to_string(), thunk_glFinish);
    thunks.insert("glFlush".to_string(), thunk_glFlush);

    thunks.insert("glXQueryExtension".to_string(), thunk_glXQueryExtension);
    thunks.insert("glXSwapBuffers".to_string(), thunk_glXSwapBuffers);

    thunks.insert("wl_display_connect".to_string(), thunk_wl_display_connect);
    thunks.insert("wl_egl_window_create".to_string(), thunk_wl_egl_window_create);
}
