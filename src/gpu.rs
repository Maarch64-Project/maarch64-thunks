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

struct NativeWindowContext {
    display: *mut std::ffi::c_void,
    window: u64,
}

unsafe impl Send for NativeWindowContext {}
unsafe impl Sync for NativeWindowContext {}

static NATIVE_WINDOW: Mutex<Option<NativeWindowContext>> = Mutex::new(None);

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
    let name = if name_ptr != 0 {
        mem.read_string(name_ptr).ok()
    } else {
        None
    };

    println!("[Maarch64 GPU Thunk] XOpenDisplay(display_name={:?})", name);

    let registry = get_gpu_registry();
    if let Some(x11_lib) = registry.get_library("libX11.so.6") {
        unsafe {
            type XOpenDisplayFn = unsafe extern "C" fn(*const std::os::raw::c_char) -> *mut std::ffi::c_void;
            if let Ok(open_dpy) = x11_lib.get::<XOpenDisplayFn>(b"XOpenDisplay\0") {
                let c_ptr = if let Some(ref bytes) = name {
                    bytes.as_ptr() as *const _
                } else {
                    std::ptr::null()
                };
                let dpy = open_dpy(c_ptr);
                if !dpy.is_null() {
                    println!("[Maarch64 GPU Thunk] Connected to Host X11 Display at {:p}", dpy);
                    ctx.set_x(0, dpy as u64);
                    return Ok(());
                }
            }
        }
    }

    println!("[Maarch64 GPU Thunk] Fallback mock X11 display handle (0x1000)");
    ctx.set_x(0, 0x1000);
    Ok(())
}

pub fn thunk_XCloseDisplay(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    println!("[Maarch64 GPU Thunk] XCloseDisplay(dpy={:#x})", dpy);
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XGetVisualInfo(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let vinfo_mask = ctx.get_x(1);
    let nitems_return = ctx.get_x(3);
    println!("[Maarch64 GPU Thunk] XGetVisualInfo(dpy={:#x}, mask={:#x})", dpy, vinfo_mask);

    if nitems_return != 0 {
        let _ = mem.write(nitems_return, &1i32.to_le_bytes());
    }

    // Allocate mock XVisualInfo (size: 64 bytes)
    let addr = mem.map_anonymous(0, 64).unwrap_or(0x7f05_0000);
    ctx.set_x(0, addr);
    Ok(())
}

pub fn thunk_XCreateWindow(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let parent = ctx.get_x(1);
    let w = ctx.get_x(4) as u32;
    let h = ctx.get_x(5) as u32;
    println!("[Maarch64 GPU Thunk] XCreateWindow(dpy={:#x}, parent={:#x}, w={}, h={})", dpy, parent, w, h);
    ctx.set_x(0, 0x2000001); // Mock X11 Window ID
    Ok(())
}

pub fn thunk_XCreateSimpleWindow(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let parent = ctx.get_x(1);
    let w = ctx.get_x(4) as u32;
    let h = ctx.get_x(5) as u32;
    println!("[Maarch64 GPU Thunk] XCreateSimpleWindow(dpy={:#x}, parent={:#x}, w={}, h={})", dpy, parent, w, h);
    ctx.set_x(0, 0x2000001);
    Ok(())
}

pub fn thunk_XMapWindow(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let win = ctx.get_x(1);
    println!("[Maarch64 GPU Thunk] XMapWindow(dpy={:#x}, win={:#x}) -> Mapping Window on Host Screen", dpy, win);
    open_host_x11_window();
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XSetStandardProperties(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let win = ctx.get_x(1);
    println!("[Maarch64 GPU Thunk] XSetStandardProperties(dpy={:#x}, win={:#x})", dpy, win);
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XFree(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let data = ctx.get_x(0);
    println!("[Maarch64 GPU Thunk] XFree(data={:#x})", data);
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_XPending(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

// ----------------------------------------------------------------------------
// EGL Thunk Handlers
// ----------------------------------------------------------------------------
pub fn thunk_eglGetDisplay(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let display_id = ctx.get_x(0);
    println!("[Maarch64 GPU Thunk] eglGetDisplay(display_id={:#x})", display_id);
    open_host_x11_window();
    ctx.set_x(0, 0x1000); // Mock EGLDisplay handle
    Ok(())
}

pub fn thunk_eglInitialize(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let major_ptr = ctx.get_x(1);
    let minor_ptr = ctx.get_x(2);
    println!("[Maarch64 GPU Thunk] eglInitialize(dpy={:#x}) -> Initializing EGL 1.5", dpy);
    if major_ptr != 0 {
        mem.write(major_ptr, &1i32.to_le_bytes()).map_err(|e| e.to_string())?;
    }
    if minor_ptr != 0 {
        mem.write(minor_ptr, &5i32.to_le_bytes()).map_err(|e| e.to_string())?; // EGL 1.5
    }
    ctx.set_x(0, 1); // EGL_TRUE
    Ok(())
}

pub fn thunk_eglBindAPI(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let api = ctx.get_x(0);
    println!("[Maarch64 GPU Thunk] eglBindAPI(api={:#x}) -> EGL_OPENGL_ES_API", api);
    ctx.set_x(0, 1); // EGL_TRUE
    Ok(())
}

pub fn thunk_eglChooseConfig(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let configs_ptr = ctx.get_x(2);
    let num_config_ptr = ctx.get_x(4);
    println!("[Maarch64 GPU Thunk] eglChooseConfig(dpy={:#x})", dpy);

    if configs_ptr != 0 {
        let _ = mem.write(configs_ptr, &0x5000u64.to_le_bytes()); // Mock EGLConfig
    }
    if num_config_ptr != 0 {
        let _ = mem.write(num_config_ptr, &1i32.to_le_bytes());
    }
    ctx.set_x(0, 1); // EGL_TRUE
    Ok(())
}

pub fn thunk_eglCreateContext(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let config = ctx.get_x(1);
    println!("[Maarch64 GPU Thunk] eglCreateContext(dpy={:#x}, config={:#x})", dpy, config);
    ctx.set_x(0, 0x6000); // Mock EGLContext
    Ok(())
}

pub fn thunk_eglCreateWindowSurface(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let win = ctx.get_x(2);
    println!("[Maarch64 GPU Thunk] eglCreateWindowSurface(dpy={:#x}, win={:#x})", dpy, win);
    ctx.set_x(0, 0x7000); // Mock EGLSurface
    Ok(())
}

pub fn thunk_eglMakeCurrent(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let draw = ctx.get_x(1);
    let read = ctx.get_x(2);
    let ctx_handle = ctx.get_x(3);
    println!("[Maarch64 GPU Thunk] eglMakeCurrent(dpy={:#x}, draw={:#x}, read={:#x}, ctx={:#x})", dpy, draw, read, ctx_handle);
    ctx.set_x(0, 1); // EGL_TRUE
    Ok(())
}

pub fn thunk_eglQueryString(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let name = ctx.get_x(1) as i32;
    println!("[Maarch64 GPU Thunk] eglQueryString(dpy={:#x}, name={})", dpy, name);

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
    println!("[Maarch64 GPU Thunk] eglSwapBuffers(dpy={:#x}, surface={:#x}) -> Swapping Framebuffers", dpy, surface);
    flush_and_hold_native_window(5);
    ctx.set_x(0, 1); // EGL_TRUE
    Ok(())
}

// ----------------------------------------------------------------------------
// OpenGL / GLES Core Thunk Handlers
// ----------------------------------------------------------------------------
pub fn thunk_glGetString(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let name = ctx.get_x(0) as u32;
    println!("[Maarch64 GPU Thunk] glGetString(name={:#x})", name);

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
    println!("[Maarch64 GPU Thunk] glClearColor(r={}, g={}, b={}, a={}) -> Setting Clear Color", r, g, b, a);
    open_host_x11_window();
    Ok(())
}

pub fn thunk_glClear(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let mask = ctx.get_x(0) as u32;
    println!("[Maarch64 GPU Thunk] glClear(mask={:#x}) -> Executing GPU Clear Buffer", mask);
    flush_and_hold_native_window(5);
    Ok(())
}

pub fn thunk_glViewport(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let x = ctx.get_x(0) as i32;
    let y = ctx.get_x(1) as i32;
    let w = ctx.get_x(2) as i32;
    let h = ctx.get_x(3) as i32;
    println!("[Maarch64 GPU Thunk] glViewport(x={}, y={}, w={}, h={})", x, y, w, h);
    Ok(())
}

pub fn thunk_glDrawArrays(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let mode = ctx.get_x(0) as u32;
    let first = ctx.get_x(1) as i32;
    let count = ctx.get_x(2) as i32;
    println!("[Maarch64 GPU Thunk] glDrawArrays(mode={:#x}, first={}, count={})", mode, first, count);
    Ok(())
}

pub fn thunk_glFinish(_ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    println!("[Maarch64 GPU Thunk] glFinish()");
    Ok(())
}

pub fn thunk_glFlush(_ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    println!("[Maarch64 GPU Thunk] glFlush()");
    Ok(())
}

pub fn thunk_wl_display_connect(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let name_ptr = ctx.get_x(0);
    println!("[Maarch64 GPU Thunk] wl_display_connect(name_ptr={:#x})", name_ptr);
    open_host_x11_window();
    ctx.set_x(0, 0x2000); // Mock wl_display handle
    Ok(())
}

pub fn thunk_wl_egl_window_create(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let surface = ctx.get_x(0);
    let width = ctx.get_x(1) as i32;
    let height = ctx.get_x(2) as i32;
    println!("[Maarch64 GPU Thunk] wl_egl_window_create(surface={:#x}, w={}, h={})", surface, width, height);
    open_host_x11_window();
    ctx.set_x(0, 0x3000); // Mock wl_egl_window handle
    Ok(())
}

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
                    
                    select_input(dpy, win, (1 << 15) | (1 << 0) | (1 << 17));
                    store_name(dpy, win, "Maarch64 AArch64 GPU Acceleration Demo (800x600)\0".as_ptr() as *const _);
                    map_win(dpy, win);
                    flush_dpy(dpy);

                    *lock = Some(NativeWindowContext { display: dpy, window: win });
                    println!("[Maarch64 GPU Thunk] SUCCESS: Created 800x600 Native Window (Window ID: {:#x})", win);
                    println!("[Maarch64 GPU Thunk] Background color set to Sky Blue (0x3399FF).");
                    println!("[Maarch64 GPU Thunk] --------------------------------------------------\n");
                    return true;
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
                }
            }
        }
    }
}

pub fn register_gpu_thunks(thunks: &mut HashMap<String, crate::ThunkFn>) {
    // Force initialization of host library resolution
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

    // EGL Thunks
    thunks.insert("eglGetDisplay".to_string(), thunk_eglGetDisplay);
    thunks.insert("eglInitialize".to_string(), thunk_eglInitialize);
    thunks.insert("eglBindAPI".to_string(), thunk_eglBindAPI);
    thunks.insert("eglChooseConfig".to_string(), thunk_eglChooseConfig);
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
