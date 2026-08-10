#![allow(non_snake_case)]

use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use std::collections::HashMap;

pub struct AlbertRegistry {
    loaded_libraries: HashMap<String, libloading::Library>,
}

impl AlbertRegistry {
    pub fn new() -> Self {
        let mut loaded_libraries = HashMap::new();
        let libs_to_try = [
            "libalbert.so.35",
            "libalbert.so.35.1",
            "libalbert.so.34",
            "libalbert.so",
        ];

        for name in libs_to_try {
            if let Ok(lib) = unsafe { libloading::Library::new(name) } {
                println!("[Maarch64 Albert Passthrough] Successfully loaded host Albert library: {}", name);
                loaded_libraries.insert("libalbert.so".to_string(), lib);
                break;
            }
        }

        Self { loaded_libraries }
    }

    pub fn get_library(&self) -> Option<&libloading::Library> {
        self.loaded_libraries.get("libalbert.so")
    }
}

static ALBERT_REGISTRY: std::sync::OnceLock<AlbertRegistry> = std::sync::OnceLock::new();
pub fn get_albert_registry() -> &'static AlbertRegistry {
    ALBERT_REGISTRY.get_or_init(AlbertRegistry::new)
}

pub fn thunk_albert_run(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let argc = ctx.get_x(0) as i32;
    let argv_ptr = ctx.get_x(1);

    println!("[Maarch64 Albert Passthrough] albert::run(argc={})", argc);

    let registry = get_albert_registry();
    if let Some(lib) = registry.get_library() {
        unsafe {
            type AlbertRunFn = unsafe extern "C" fn(i32, *const *const std::os::raw::c_char) -> i32;

            let sym_detail = lib.get::<AlbertRunFn>(b"_ZN6albert6detail3runEiPPc\0");
            let sym_main = lib.get::<AlbertRunFn>(b"_ZN6albert3runEiPPc\0");

            let run_fn = sym_detail.or(sym_main);

            if let Ok(albert_run) = run_fn {
                let mut c_args: Vec<std::ffi::CString> = Vec::new();
                let mut c_ptrs: Vec<*const std::os::raw::c_char> = Vec::new();

                for i in 0..argc {
                    let ptr_addr = argv_ptr + (i as u64) * 8;
                    if let Ok(arg_addr_bytes) = mem.read(ptr_addr, 8) {
                        let arg_addr = u64::from_le_bytes(arg_addr_bytes.try_into().unwrap());
                        if let Ok(str_bytes) = mem.read_string(arg_addr) {
                            if let Ok(cstr) = std::ffi::CString::new(str_bytes) {
                                c_ptrs.push(cstr.as_ptr());
                                c_args.push(cstr);
                            }
                        }
                    }
                }

                let ret = albert_run(c_ptrs.len() as i32, c_ptrs.as_ptr());
                println!("[Maarch64 Albert Passthrough] Host albert::run completed with code {}", ret);
                ctx.set_x(0, ret as u64);
                return Ok(());
            }
        }
    }

    println!("[Maarch64 Albert Passthrough] Could not load host libalbert.so");
    ctx.set_x(0, 0);
    Ok(())
}

pub fn register_albert_thunks(map: &mut HashMap<String, crate::ThunkFn>) {
    map.insert("_ZN6albert6detail3runEiPPc".to_string(), thunk_albert_run);
    map.insert("_ZN6albert3runEiPPc".to_string(), thunk_albert_run);
}
