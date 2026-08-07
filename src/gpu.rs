#![allow(non_snake_case)]

use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use std::collections::HashMap;

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

    pub fn has_library(&self, name: &str) -> bool {
        self.loaded_libraries.contains_key(name)
    }
}

static GPU_REGISTRY: std::sync::OnceLock<GpuThunkRegistry> = std::sync::OnceLock::new();
pub fn get_gpu_registry() -> &'static GpuThunkRegistry {
    GPU_REGISTRY.get_or_init(GpuThunkRegistry::new)
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
    tracing::debug!("[thunk] eglGetDisplay(display_id={:#x})", display_id);
    ctx.set_x(0, 0x1000); // Mock EGLDisplay handle
    Ok(())
}

pub fn thunk_eglInitialize(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let major_ptr = ctx.get_x(1);
    let minor_ptr = ctx.get_x(2);
    tracing::debug!("[thunk] eglInitialize(dpy={:#x}, major_ptr={:#x}, minor_ptr={:#x})", dpy, major_ptr, minor_ptr);
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
    tracing::debug!("[thunk] eglQueryString(dpy={:#x}, name={})", dpy, name);

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
    tracing::debug!("[thunk] eglSwapBuffers(dpy={:#x}, surface={:#x})", dpy, surface);
    ctx.set_x(0, 1); // EGL_TRUE
    Ok(())
}

// ----------------------------------------------------------------------------
// OpenGL / GLES Core Thunk Handlers
// ----------------------------------------------------------------------------
pub fn thunk_glGetString(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let name = ctx.get_x(0) as u32;
    tracing::debug!("[thunk] glGetString(name={:#x})", name);

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
    tracing::debug!("[thunk] glClearColor(r={}, g={}, b={}, a={})", r, g, b, a);
    Ok(())
}

pub fn thunk_glClear(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let mask = ctx.get_x(0) as u32;
    tracing::debug!("[thunk] glClear(mask={:#x})", mask);
    Ok(())
}

pub fn thunk_glViewport(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let x = ctx.get_x(0) as i32;
    let y = ctx.get_x(1) as i32;
    let w = ctx.get_x(2) as i32;
    let h = ctx.get_x(3) as i32;
    tracing::debug!("[thunk] glViewport(x={}, y={}, w={}, h={})", x, y, w, h);
    Ok(())
}

pub fn thunk_glDrawArrays(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let mode = ctx.get_x(0) as u32;
    let first = ctx.get_x(1) as i32;
    let count = ctx.get_x(2) as i32;
    tracing::debug!("[thunk] glDrawArrays(mode={:#x}, first={}, count={})", mode, first, count);
    Ok(())
}

pub fn thunk_glFinish(_ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::debug!("[thunk] glFinish()");
    Ok(())
}

pub fn thunk_glFlush(_ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::debug!("[thunk] glFlush()");
    Ok(())
}

// ----------------------------------------------------------------------------
// GLX Thunk Handlers
// ----------------------------------------------------------------------------
pub fn thunk_glXQueryExtension(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dpy = ctx.get_x(0);
    let error_base = ctx.get_x(1);
    let event_base = ctx.get_x(2);
    tracing::debug!("[thunk] glXQueryExtension(dpy={:#x}, error_base={:#x}, event_base={:#x})", dpy, error_base, event_base);
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
    tracing::debug!("[thunk] glXSwapBuffers(dpy={:#x}, drawable={:#x})", dpy, drawable);
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
}
