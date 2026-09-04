use maarch64_core::{cpu::CpuContext, memory::MemoryManager};

pub fn thunk_objc_msgSend(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let self_ptr = ctx.get_x(0);
    let sel_ptr = ctx.get_x(1);
    let sel_name = if let Ok(bytes) = mem.read_string(sel_ptr) {
        String::from_utf8_lossy(&bytes).to_string()
    } else {
        format!("{:#x}", sel_ptr)
    };
    tracing::info!("[Darwin Thunk] objc_msgSend(self={:#x}, sel='{}')", self_ptr, sel_name);
    // Return self or 1
    ctx.set_x(0, if self_ptr != 0 { self_ptr } else { 1 });
    Ok(())
}

pub fn thunk_objc_getClass(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let name_ptr = ctx.get_x(0);
    let class_name = if let Ok(bytes) = mem.read_string(name_ptr) {
        String::from_utf8_lossy(&bytes).to_string()
    } else {
        "Unknown".to_string()
    };
    tracing::info!("[Darwin Thunk] objc_getClass('{}')", class_name);
    ctx.set_x(0, 0x7f040000);
    Ok(())
}

pub fn thunk_sel_registerName(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let name_ptr = ctx.get_x(0);
    let sel_name = if let Ok(bytes) = mem.read_string(name_ptr) {
        String::from_utf8_lossy(&bytes).to_string()
    } else {
        "Unknown".to_string()
    };
    tracing::info!("[Darwin Thunk] sel_registerName('{}')", sel_name);
    ctx.set_x(0, name_ptr);
    Ok(())
}

pub fn thunk_objc_autoreleasePoolPush(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0x7f050000);
    Ok(())
}

pub fn thunk_objc_autoreleasePoolPop(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_os_log_create(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0x7f060000);
    Ok(())
}

pub fn thunk_os_log_type_enabled(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 1);
    Ok(())
}

pub fn thunk_os_unfair_lock_lock(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_os_unfair_lock_unlock(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_dispatch_async(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_dispatch_get_main_queue(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0x7f070000);
    Ok(())
}

pub fn thunk_dispatch_get_global_queue(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0x7f070008);
    Ok(())
}

pub fn thunk_arc4random(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let r = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    ctx.set_x(0, r as u64);
    Ok(())
}

pub fn thunk_arc4random_buf(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let buf_ptr = ctx.get_x(0);
    let nbytes = ctx.get_x(1) as usize;
    if buf_ptr != 0 && nbytes > 0 {
        let dummy_rand: Vec<u8> = (0..nbytes).map(|i| ((i * 37 + 13) % 256) as u8).collect();
        let _ = mem.write(buf_ptr, &dummy_rand);
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_cf_string_create_with_cstring(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let cstr_ptr = ctx.get_x(1);
    ctx.set_x(0, if cstr_ptr != 0 { cstr_ptr } else { 0x7f080000 });
    Ok(())
}

pub fn thunk_cf_release(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_cf_retain(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let cf_ptr = ctx.get_x(0);
    ctx.set_x(0, cf_ptr);
    Ok(())
}

pub fn register_darwin_thunks(manager: &mut crate::ThunkManager) {
    manager.register_symbol("objc_msgSend", thunk_objc_msgSend);
    manager.register_symbol("objc_msgSend_stret", thunk_objc_msgSend);
    manager.register_symbol("objc_msgSendSuper2", thunk_objc_msgSend);
    manager.register_symbol("objc_getClass", thunk_objc_getClass);
    manager.register_symbol("sel_registerName", thunk_sel_registerName);
    manager.register_symbol("objc_autoreleasePoolPush", thunk_objc_autoreleasePoolPush);
    manager.register_symbol("objc_autoreleasePoolPop", thunk_objc_autoreleasePoolPop);
    manager.register_symbol("os_log_create", thunk_os_log_create);
    manager.register_symbol("os_log_type_enabled", thunk_os_log_type_enabled);
    manager.register_symbol("os_unfair_lock_lock", thunk_os_unfair_lock_lock);
    manager.register_symbol("os_unfair_lock_unlock", thunk_os_unfair_lock_unlock);
    manager.register_symbol("dispatch_async", thunk_dispatch_async);
    manager.register_symbol("dispatch_get_main_queue", thunk_dispatch_get_main_queue);
    manager.register_symbol("dispatch_get_global_queue", thunk_dispatch_get_global_queue);
    manager.register_symbol("arc4random", thunk_arc4random);
    manager.register_symbol("arc4random_buf", thunk_arc4random_buf);
    manager.register_symbol("CFStringCreateWithCString", thunk_cf_string_create_with_cstring);
    manager.register_symbol("CFRelease", thunk_cf_release);
    manager.register_symbol("CFRetain", thunk_cf_retain);
}
