use maarch64_core::{cpu::CpuContext, memory::MemoryManager};

pub fn thunk_MTLCreateSystemDefaultDevice(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    println!("[Maarch64 Metal Passthrough] MTLCreateSystemDefaultDevice()");
    // Return a dummy Metal Device handle
    ctx.set_x(0, 0x4d544c00); // "MTL\0"
    Ok(())
}

pub fn register_metal_thunks(manager: &mut crate::ThunkManager) {
    manager.register_symbol("MTLCreateSystemDefaultDevice", thunk_MTLCreateSystemDefaultDevice);
}
