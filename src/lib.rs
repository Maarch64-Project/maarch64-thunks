use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use std::collections::HashMap;

pub type ThunkFn = fn(&mut CpuContext, &mut MemoryManager) -> Result<(), String>;

pub struct ThunkManager {
    wrapped_symbols: HashMap<String, ThunkFn>,
    address_thunks: HashMap<u64, ThunkFn>,
}

impl ThunkManager {
    pub fn new() -> Self {
        let mut manager = Self {
            wrapped_symbols: HashMap::new(),
            address_thunks: HashMap::new(),
        };
        manager.register_builtin_thunks();
        manager
    }

    pub fn register_thunk(&mut self, name: &str, handler: ThunkFn) {
        self.wrapped_symbols.insert(name.to_string(), handler);
    }

    pub fn register_thunk_address(&mut self, vaddr: u64, handler: ThunkFn) {
        self.address_thunks.insert(vaddr, handler);
    }

    pub fn get_thunk(&self, name: &str) -> Option<ThunkFn> {
        self.wrapped_symbols.get(name).copied()
    }

    pub fn get_thunk_by_address(&self, vaddr: u64) -> Option<ThunkFn> {
        self.address_thunks.get(&vaddr).copied()
    }

    fn register_builtin_thunks(&mut self) {
        self.register_thunk("malloc", thunk_malloc);
        self.register_thunk("free", thunk_free);
        self.register_thunk("puts", thunk_puts);
        self.register_thunk("putchar", thunk_putchar);
        self.register_thunk("strlen", thunk_strlen);
        self.register_thunk("memcpy", thunk_memcpy);
        self.register_thunk("memset", thunk_memset);
        self.register_thunk("exit", thunk_exit);
    }
}

// Built-in Standard C Library Thunks
pub fn thunk_malloc(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let size = ctx.get_x(0) as usize;
    if size == 0 {
        ctx.set_x(0, 0);
        return Ok(());
    }
    // Align allocation size to page boundary
    let page_size = 4096;
    let aligned_size = ((size + page_size - 1) / page_size) * page_size;
    let vaddr = mem
        .map_anonymous(0, aligned_size)
        .map_err(|e| format!("malloc error: {}", e))?;
    ctx.set_x(0, vaddr);
    Ok(())
}

pub fn thunk_free(_ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    // Memory arena managed, free is no-op for guest safety
    Ok(())
}

pub fn thunk_puts(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let str_ptr = ctx.get_x(0);
    let bytes = mem
        .read_string(str_ptr)
        .map_err(|e| format!("puts error: {}", e))?;
    let s = String::from_utf8_lossy(&bytes);
    println!("{}", s);
    ctx.set_x(0, (bytes.len() + 1) as u64);
    Ok(())
}

pub fn thunk_putchar(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let c = ctx.get_x(0) as u8;
    print!("{}", c as char);
    ctx.set_x(0, c as u64);
    Ok(())
}

pub fn thunk_strlen(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let str_ptr = ctx.get_x(0);
    let bytes = mem
        .read_string(str_ptr)
        .map_err(|e| format!("strlen error: {}", e))?;
    ctx.set_x(0, bytes.len() as u64);
    Ok(())
}

pub fn thunk_memcpy(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dest = ctx.get_x(0);
    let src = ctx.get_x(1);
    let n = ctx.get_x(2) as usize;

    if n > 0 {
        let src_bytes = mem
            .read(src, n)
            .map_err(|e| format!("memcpy src error: {}", e))?
            .to_vec();
        mem.write(dest, &src_bytes)
            .map_err(|e| format!("memcpy dest error: {}", e))?;
    }
    ctx.set_x(0, dest);
    Ok(())
}

pub fn thunk_memset(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let s = ctx.get_x(0);
    let c = ctx.get_x(1) as u8;
    let n = ctx.get_x(2) as usize;

    if n > 0 {
        let buf = vec![c; n];
        mem.write(s, &buf)
            .map_err(|e| format!("memset error: {}", e))?;
    }
    ctx.set_x(0, s);
    Ok(())
}

pub fn thunk_exit(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let code = ctx.get_x(0) as i32;
    ctx.exited = true;
    ctx.exit_code = code;
    Ok(())
}
