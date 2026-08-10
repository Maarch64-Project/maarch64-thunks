#![allow(non_snake_case)]

use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use std::collections::HashMap;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::OnceLock;

pub struct SdlRegistry {
    loaded_libraries: HashMap<String, libloading::Library>,
}

impl SdlRegistry {
    pub fn new() -> Self {
        let mut loaded_libraries = HashMap::new();
        let libs_to_try = [
            ("libSDL2-2.0.so.0", "libSDL2.so"),
            ("libSDL2.so", "libSDL2.so"),
        ];

        for (name, alt_name) in libs_to_try {
            if let Ok(lib) = unsafe { libloading::Library::new(name) } {
                tracing::info!("[Maarch64 SDL2 Passthrough] Successfully loaded host SDL2 library: {}", name);
                loaded_libraries.insert(name.to_string(), lib);
                break;
            } else if let Ok(lib) = unsafe { libloading::Library::new(alt_name) } {
                tracing::info!("[Maarch64 SDL2 Passthrough] Successfully loaded host SDL2 library: {}", alt_name);
                loaded_libraries.insert(name.to_string(), lib);
                break;
            }
        }

        Self { loaded_libraries }
    }

    pub fn get_library(&self) -> Option<&libloading::Library> {
        self.loaded_libraries.values().next()
    }
}

static SDL_REGISTRY: OnceLock<SdlRegistry> = OnceLock::new();
pub fn get_sdl_registry() -> &'static SdlRegistry {
    SDL_REGISTRY.get_or_init(SdlRegistry::new)
}

pub fn thunk_SDL_Init(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let flags = ctx.get_x(0) as u32;
    tracing::info!("[Maarch64 SDL2 Passthrough] SDL_Init(flags={:#x})", flags);

    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type InitFn = unsafe extern "C" fn(u32) -> c_int;
            if let Ok(init) = sdl_lib.get::<InitFn>(b"SDL_Init\0") {
                let res = init(flags);
                ctx.set_x(0, res as i64 as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_InitSubSystem(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let flags = ctx.get_x(0) as u32;
    tracing::info!("[Maarch64 SDL2 Passthrough] SDL_InitSubSystem(flags={:#x})", flags);

    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type InitSubFn = unsafe extern "C" fn(u32) -> c_int;
            if let Ok(init_sub) = sdl_lib.get::<InitSubFn>(b"SDL_InitSubSystem\0") {
                let res = init_sub(flags);
                ctx.set_x(0, res as i64 as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_Quit(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[Maarch64 SDL2 Passthrough] SDL_Quit()");

    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type QuitFn = unsafe extern "C" fn();
            if let Ok(quit) = sdl_lib.get::<QuitFn>(b"SDL_Quit\0") {
                quit();
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_QuitSubSystem(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let flags = ctx.get_x(0) as u32;
    tracing::info!("[Maarch64 SDL2 Passthrough] SDL_QuitSubSystem(flags={:#x})", flags);

    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type QuitSubFn = unsafe extern "C" fn(u32);
            if let Ok(quit_sub) = sdl_lib.get::<QuitSubFn>(b"SDL_QuitSubSystem\0") {
                quit_sub(flags);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_CreateWindow(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let title_ptr = ctx.get_x(0);
    let x = ctx.get_x(1) as c_int;
    let y = ctx.get_x(2) as c_int;
    let w = ctx.get_x(3) as c_int;
    let h = ctx.get_x(4) as c_int;
    let flags = ctx.get_x(5) as u32;

    let title_str = if title_ptr != 0 {
        mem.read_string(title_ptr)
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_else(|_| "Maarch64 Window".to_string())
    } else {
        "Maarch64 Window".to_string()
    };

    tracing::info!("[Maarch64 SDL2 Passthrough] SDL_CreateWindow(title={:?}, x={}, y={}, w={}, h={}, flags={:#x})", title_str, x, y, w, h, flags);

    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type CreateWindowFn = unsafe extern "C" fn(*const c_char, c_int, c_int, c_int, c_int, u32) -> *mut c_void;
            if let Ok(create_window) = sdl_lib.get::<CreateWindowFn>(b"SDL_CreateWindow\0") {
                let c_title = std::ffi::CString::new(title_str).unwrap();
                let window_ptr = create_window(c_title.as_ptr(), x, y, w, h, flags);
                ctx.set_x(0, window_ptr as u64);
                return Ok(());
            }
        }
    }

    ctx.set_x(0, 0x5555_0000);
    Ok(())
}

pub fn thunk_SDL_DestroyWindow(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let window_ptr = ctx.get_x(0) as *mut c_void;
    tracing::info!("[Maarch64 SDL2 Passthrough] SDL_DestroyWindow(window={:?})", window_ptr);

    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type DestroyWindowFn = unsafe extern "C" fn(*mut c_void);
            if let Ok(destroy_window) = sdl_lib.get::<DestroyWindowFn>(b"SDL_DestroyWindow\0") {
                destroy_window(window_ptr);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_ShowWindow(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let window_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type ShowWindowFn = unsafe extern "C" fn(*mut c_void);
            if let Ok(show_window) = sdl_lib.get::<ShowWindowFn>(b"SDL_ShowWindow\0") {
                show_window(window_ptr);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_HideWindow(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let window_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type HideWindowFn = unsafe extern "C" fn(*mut c_void);
            if let Ok(hide_window) = sdl_lib.get::<HideWindowFn>(b"SDL_HideWindow\0") {
                hide_window(window_ptr);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_CreateRenderer(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let window_ptr = ctx.get_x(0) as *mut c_void;
    let index = ctx.get_x(1) as c_int;
    let flags = ctx.get_x(2) as u32;

    tracing::info!("[Maarch64 SDL2 Passthrough] SDL_CreateRenderer(window={:?}, index={}, flags={:#x})", window_ptr, index, flags);

    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type CreateRendererFn = unsafe extern "C" fn(*mut c_void, c_int, u32) -> *mut c_void;
            if let Ok(create_renderer) = sdl_lib.get::<CreateRendererFn>(b"SDL_CreateRenderer\0") {
                let renderer_ptr = create_renderer(window_ptr, index, flags);
                ctx.set_x(0, renderer_ptr as u64);
                return Ok(());
            }
        }
    }

    ctx.set_x(0, 0x5555_0008);
    Ok(())
}

pub fn thunk_SDL_DestroyRenderer(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let renderer_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type DestroyRendererFn = unsafe extern "C" fn(*mut c_void);
            if let Ok(destroy_renderer) = sdl_lib.get::<DestroyRendererFn>(b"SDL_DestroyRenderer\0") {
                destroy_renderer(renderer_ptr);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_RenderClear(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let renderer_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type RenderClearFn = unsafe extern "C" fn(*mut c_void) -> c_int;
            if let Ok(render_clear) = sdl_lib.get::<RenderClearFn>(b"SDL_RenderClear\0") {
                let res = render_clear(renderer_ptr);
                ctx.set_x(0, res as i64 as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_RenderPresent(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let renderer_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type RenderPresentFn = unsafe extern "C" fn(*mut c_void);
            if let Ok(render_present) = sdl_lib.get::<RenderPresentFn>(b"SDL_RenderPresent\0") {
                render_present(renderer_ptr);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_SetRenderDrawColor(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let renderer_ptr = ctx.get_x(0) as *mut c_void;
    let r = ctx.get_x(1) as u8;
    let g = ctx.get_x(2) as u8;
    let b = ctx.get_x(3) as u8;
    let a = ctx.get_x(4) as u8;

    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type SetColorFn = unsafe extern "C" fn(*mut c_void, u8, u8, u8, u8) -> c_int;
            if let Ok(set_color) = sdl_lib.get::<SetColorFn>(b"SDL_SetRenderDrawColor\0") {
                let res = set_color(renderer_ptr, r, g, b, a);
                ctx.set_x(0, res as i64 as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_PollEvent(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let event_ptr = ctx.get_x(0);
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type PollEventFn = unsafe extern "C" fn(*mut [u8; 56]) -> c_int;
            if let Ok(poll_event) = sdl_lib.get::<PollEventFn>(b"SDL_PollEvent\0") {
                let mut event_buf = [0u8; 56];
                let res = poll_event(&mut event_buf);
                if res != 0 && event_ptr != 0 {
                    let _ = mem.write(event_ptr, &event_buf);
                }
                ctx.set_x(0, res as i64 as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_WaitEvent(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let event_ptr = ctx.get_x(0);
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type WaitEventFn = unsafe extern "C" fn(*mut [u8; 56]) -> c_int;
            if let Ok(wait_event) = sdl_lib.get::<WaitEventFn>(b"SDL_WaitEvent\0") {
                let mut event_buf = [0u8; 56];
                let res = wait_event(&mut event_buf);
                if res != 0 && event_ptr != 0 {
                    let _ = mem.write(event_ptr, &event_buf);
                }
                ctx.set_x(0, res as i64 as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_GetError(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type GetErrorFn = unsafe extern "C" fn() -> *const c_char;
            if let Ok(get_error) = sdl_lib.get::<GetErrorFn>(b"SDL_GetError\0") {
                let err_ptr = get_error();
                ctx.set_x(0, err_ptr as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_ClearError(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type ClearErrorFn = unsafe extern "C" fn();
            if let Ok(clear_error) = sdl_lib.get::<ClearErrorFn>(b"SDL_ClearError\0") {
                clear_error();
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_GL_CreateContext(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let window_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type CreateCtxFn = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
            if let Ok(create_ctx) = sdl_lib.get::<CreateCtxFn>(b"SDL_GL_CreateContext\0") {
                let gl_ctx = create_ctx(window_ptr);
                ctx.set_x(0, gl_ctx as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0x5555_0010);
    Ok(())
}

pub fn thunk_SDL_GL_SwapWindow(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let window_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type SwapWinFn = unsafe extern "C" fn(*mut c_void);
            if let Ok(swap_win) = sdl_lib.get::<SwapWinFn>(b"SDL_GL_SwapWindow\0") {
                swap_win(window_ptr);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_SDL_GetTicks(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type GetTicksFn = unsafe extern "C" fn() -> u32;
            if let Ok(get_ticks) = sdl_lib.get::<GetTicksFn>(b"SDL_GetTicks\0") {
                let ticks = get_ticks();
                ctx.set_x(0, ticks as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 100);
    Ok(())
}

pub fn thunk_SDL_Delay(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let ms = ctx.get_x(0) as u32;
    if let Some(sdl_lib) = get_sdl_registry().get_library() {
        unsafe {
            type DelayFn = unsafe extern "C" fn(u32);
            if let Ok(delay) = sdl_lib.get::<DelayFn>(b"SDL_Delay\0") {
                delay(ms);
                ctx.set_x(0, 0);
                return Ok(());
            }
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    ctx.set_x(0, 0);
    Ok(())
}

pub fn register_sdl_thunks(map: &mut HashMap<String, super::ThunkFn>) {
    map.insert("SDL_Init".to_string(), thunk_SDL_Init);
    map.insert("SDL_InitSubSystem".to_string(), thunk_SDL_InitSubSystem);
    map.insert("SDL_Quit".to_string(), thunk_SDL_Quit);
    map.insert("SDL_QuitSubSystem".to_string(), thunk_SDL_QuitSubSystem);
    map.insert("SDL_CreateWindow".to_string(), thunk_SDL_CreateWindow);
    map.insert("SDL_DestroyWindow".to_string(), thunk_SDL_DestroyWindow);
    map.insert("SDL_ShowWindow".to_string(), thunk_SDL_ShowWindow);
    map.insert("SDL_HideWindow".to_string(), thunk_SDL_HideWindow);
    map.insert("SDL_CreateRenderer".to_string(), thunk_SDL_CreateRenderer);
    map.insert("SDL_DestroyRenderer".to_string(), thunk_SDL_DestroyRenderer);
    map.insert("SDL_RenderClear".to_string(), thunk_SDL_RenderClear);
    map.insert("SDL_RenderPresent".to_string(), thunk_SDL_RenderPresent);
    map.insert("SDL_SetRenderDrawColor".to_string(), thunk_SDL_SetRenderDrawColor);
    map.insert("SDL_PollEvent".to_string(), thunk_SDL_PollEvent);
    map.insert("SDL_WaitEvent".to_string(), thunk_SDL_WaitEvent);
    map.insert("SDL_GetError".to_string(), thunk_SDL_GetError);
    map.insert("SDL_ClearError".to_string(), thunk_SDL_ClearError);
    map.insert("SDL_GL_CreateContext".to_string(), thunk_SDL_GL_CreateContext);
    map.insert("SDL_GL_SwapWindow".to_string(), thunk_SDL_GL_SwapWindow);
    map.insert("SDL_GetTicks".to_string(), thunk_SDL_GetTicks);
    map.insert("SDL_Delay".to_string(), thunk_SDL_Delay);
}

