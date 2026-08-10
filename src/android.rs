use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use crate::ThunkManager;

/// Register Android Bionic and NDK thunks into ThunkManager
pub fn register_android_thunks(manager: &mut ThunkManager) {
    manager.register_thunk("__android_log_print", thunk_android_log_print);
    manager.register_thunk("__android_log_write", thunk_android_log_print);
    manager.register_thunk("__android_log_vprint", thunk_android_log_vprint);
    manager.register_thunk("__system_property_get", thunk_system_property_get);
    manager.register_thunk("AAssetManager_open", thunk_AAssetManager_open);
    manager.register_thunk("ANativeWindow_fromSurface", thunk_ANativeWindow_fromSurface);
    manager.register_thunk("ANativeWindow_getWidth", thunk_ANativeWindow_getWidth);
    manager.register_thunk("ANativeWindow_getHeight", thunk_ANativeWindow_getHeight);
    manager.register_thunk("ANativeWindow_setBuffersGeometry", thunk_ANativeWindow_setBuffersGeometry);
    manager.register_thunk("ANativeWindow_acquire", thunk_ANativeWindow_acquire);
    manager.register_thunk("ANativeWindow_release", thunk_ANativeWindow_release);
}

/// Thunk handler for `__android_log_print(int prio, const char *tag, const char *fmt, ...)`
pub fn thunk_android_log_print(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let prio = ctx.get_x(0) as i32;
    let tag_ptr = ctx.get_x(1);
    let fmt_ptr = ctx.get_x(2);

    let tag = if tag_ptr != 0 {
        mem.read_string(tag_ptr)
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_else(|_| "UnknownTag".to_string())
    } else {
        "AndroidApp".to_string()
    };

    let mut msg = String::new();
    if fmt_ptr != 0 {
        if let Ok(fmt_bytes) = mem.read_string(fmt_ptr) {
            let fmt = String::from_utf8_lossy(&fmt_bytes);
            let mut arg_idx = 3;
            let mut chars = fmt.chars().peekable();
            while let Some(c) = chars.next() {
                if c == '%' {
                    if let Some(&next_c) = chars.peek() {
                        match next_c {
                            's' => {
                                chars.next();
                                let str_ptr = ctx.get_x(arg_idx);
                                arg_idx += 1;
                                if str_ptr != 0 {
                                    if let Ok(s_bytes) = mem.read_string(str_ptr) {
                                        msg.push_str(&String::from_utf8_lossy(&s_bytes));
                                    }
                                }
                            }
                            'd' | 'i' => {
                                chars.next();
                                let val = ctx.get_x(arg_idx) as i64 as i32;
                                arg_idx += 1;
                                msg.push_str(&val.to_string());
                            }
                            'u' => {
                                chars.next();
                                let val = ctx.get_x(arg_idx) as u32;
                                arg_idx += 1;
                                msg.push_str(&val.to_string());
                            }
                            'x' | 'p' => {
                                chars.next();
                                let val = ctx.get_x(arg_idx);
                                arg_idx += 1;
                                msg.push_str(&format!("{:#x}", val));
                            }
                            '%' => {
                                chars.next();
                                msg.push('%');
                            }
                            _ => {
                                msg.push('%');
                            }
                        }
                    } else {
                        msg.push('%');
                    }
                } else {
                    msg.push(c);
                }
            }
        }
    }

    let prio_str = match prio {
        2 => "VERBOSE",
        3 => "DEBUG",
        4 => "INFO",
        5 => "WARN",
        6 => "ERROR",
        7 => "FATAL",
        _ => "LOG",
    };

    println!("[Android Log: {}/{}] {}", tag, prio_str, msg);
    ctx.set_x(0, msg.len() as u64);
    Ok(())
}

/// Thunk handler for `__android_log_vprint`
pub fn thunk_android_log_vprint(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    thunk_android_log_print(ctx, mem)
}

/// Thunk handler for `__system_property_get(const char *name, char *value)`
pub fn thunk_system_property_get(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let name_ptr = ctx.get_x(0);
    let value_ptr = ctx.get_x(1);

    if name_ptr == 0 || value_ptr == 0 {
        ctx.set_x(0, 0);
        return Ok(());
    }

    let name = mem.read_string(name_ptr)
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .unwrap_or_default();

    let val = match name.as_str() {
        "ro.build.version.sdk" => "33",
        "ro.build.version.release" => "13",
        "ro.product.model" => "Maarch64-Linux-Device",
        "ro.product.brand" => "Maarch64",
        "ro.hardware" => "maarch64",
        "ro.arch" => "arm64",
        _ => "",
    };

    let val_bytes = val.as_bytes();
    let _ = mem.write(value_ptr, val_bytes);
    let _ = mem.write(value_ptr + val_bytes.len() as u64, &[0u8]);

    tracing::info!("[Android System Property] {} -> '{}'", name, val);
    ctx.set_x(0, val_bytes.len() as u64);
    Ok(())
}

/// Thunk handler stub for `AAssetManager_open`
pub fn thunk_AAssetManager_open(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[Android NDK] AAssetManager_open stub called");
    ctx.set_x(0, 0x1000); // Return dummy asset pointer handle
    Ok(())
}

/// Thunk handler for `ANativeWindow_fromSurface`
pub fn thunk_ANativeWindow_fromSurface(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[Android NDK] ANativeWindow_fromSurface creating native X11 GUI window");
    let _ = crate::gpu::thunk_XCreateWindow(ctx, mem);
    let win = ctx.get_x(0);
    ctx.set_x(0, win);
    Ok(())
}

/// Thunk handler for `ANativeWindow_getWidth`
pub fn thunk_ANativeWindow_getWidth(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 800);
    Ok(())
}

/// Thunk handler for `ANativeWindow_getHeight`
pub fn thunk_ANativeWindow_getHeight(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 600);
    Ok(())
}

/// Thunk handler for `ANativeWindow_setBuffersGeometry`
pub fn thunk_ANativeWindow_setBuffersGeometry(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0); // 0 = SUCCESS
    Ok(())
}

/// Thunk handler for `ANativeWindow_acquire`
pub fn thunk_ANativeWindow_acquire(_ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    Ok(())
}

/// Thunk handler for `ANativeWindow_release`
pub fn thunk_ANativeWindow_release(_ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    Ok(())
}
