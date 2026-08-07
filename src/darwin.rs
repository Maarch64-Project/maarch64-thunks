use maarch64_core::{cpu::CpuContext, memory::MemoryManager};

pub fn thunk_objc_msgSend(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    println!("[Maarch64 Darwin Thunk] objc_msgSend(self={:#x}, sel={:#x})", ctx.get_x(0), ctx.get_x(1));
    ctx.set_x(0, 0);
    Ok(())
}

pub fn register_darwin_thunks(manager: &mut crate::ThunkManager) {
    manager.register_symbol("objc_msgSend", thunk_objc_msgSend);
}
