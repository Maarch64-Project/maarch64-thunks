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

    let msg = if fmt_ptr != 0 {
        mem.read_string(fmt_ptr)
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_else(|_| "".to_string())
    } else {
        "".to_string()
    };

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

/// Thunk handler stub for `ANativeWindow_fromSurface`
pub fn thunk_ANativeWindow_fromSurface(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[Android NDK] ANativeWindow_fromSurface stub called");
    ctx.set_x(0, 0x2000); // Return dummy native window handle
    Ok(())
}
