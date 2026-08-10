use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use std::collections::HashMap;

pub type ThunkFn = fn(&mut CpuContext, &mut MemoryManager) -> Result<(), String>;

pub struct ThunkManager {
    wrapped_symbols: HashMap<String, ThunkFn>,
    address_thunks: HashMap<u64, ThunkFn>,
}

mod generated;
pub mod gpu;
pub mod audio;
pub mod vlc;
pub mod albert;
pub mod darwin;
pub mod metal;
pub mod sdl;
pub mod gtk;
pub mod qt;
pub mod curl;

pub fn thunk_stub(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    println!("[Maarch64 Thunk Warning] Unhandled dynamic symbol stub called!");
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_dlsym(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let _handle = ctx.get_x(0);
    let symbol_ptr = ctx.get_x(1);
    if symbol_ptr != 0 {
        if let Ok(symbol_bytes) = mem.read_string(symbol_ptr) {
            let name = String::from_utf8_lossy(&symbol_bytes);
            println!("[Maarch64 Thunk] dlsym(symbol={:?})", name);
            let addr = match name.as_ref() {
                "libvlc_new" => 0x7f000130,
                "libvlc_set_app_id" => 0x7f000138,
                "libvlc_set_user_agent" => 0x7f000100,
                "libvlc_get_version" => 0x7f0000e0,
                "libvlc_get_changeset" => 0x7f000058,
                "libvlc_release" => 0x7f0000d8,
                "libvlc_add_intf" => 0x7f0000c8,
                "libvlc_playlist_play" => 0x7f000080,
                "libvlc_set_exit_handler" => 0x7f000090,
                _ => 0x7f000000,
            };
            ctx.set_x(0, addr);
            return Ok(());
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_sigwait(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    println!("[Maarch64 Thunk] sigwait - delegating to host event wait loop...");
    vlc::thunk_libvlc_wait(ctx, mem)
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

    pub fn resolve_dynamic_symbol(&mut self, name: &str, vaddr: u64) {
        eprintln!("[Maarch64 Thunk] Dynamic symbol resolve: {} -> {:#x}", name, vaddr);
        let handler = self.get_thunk(name).unwrap_or(thunk_stub);
        self.register_thunk_address(vaddr, handler);
    }

    pub fn get_thunk(&self, name: &str) -> Option<ThunkFn> {
        if name.contains("path_get_dirname") {
            return Some(thunk_glibmm_path_get_dirname);
        }
        if name.contains("get_user_config_dir") {
            return Some(thunk_glibmm_get_user_config_dir);
        }
        if name.contains("build_filename") {
            return Some(thunk_glibmm_build_filename);
        }
        if name.contains("file_test") {
            return Some(thunk_glibmm_file_test);
        }
        if name.contains("getenv") && name.contains("Glib") {
            return Some(thunk_glibmm_getenv);
        }
        if name.contains("canonicalize_filename") {
            return Some(thunk_glibmm_canonicalize_filename);
        }
        if let Some(h) = self.wrapped_symbols.get(name).copied() {
            return Some(h);
        }
        None
    }

    pub fn get_thunk_by_address(&self, vaddr: u64) -> Option<ThunkFn> {
        if let Some(handler) = self.address_thunks.get(&vaddr).copied() {
            tracing::debug!("[Thunk Exec] vaddr={:#x}", vaddr);
            Some(handler)
        } else {
            None
        }
    }

    pub fn register_symbol(&mut self, name: &str, handler: ThunkFn) {
        self.register_thunk(name, handler);
    }

    fn register_builtin_thunks(&mut self) {
        generated::register_generated_thunks(self);
        gpu::register_gpu_thunks(&mut self.wrapped_symbols);
        audio::register_audio_thunks(&mut self.wrapped_symbols);
        vlc::register_vlc_thunks(&mut self.wrapped_symbols);
        albert::register_albert_thunks(&mut self.wrapped_symbols);
        sdl::register_sdl_thunks(&mut self.wrapped_symbols);
        gtk::register_gtk_thunks(&mut self.wrapped_symbols);
        qt::register_qt_thunks(&mut self.wrapped_symbols);
        curl::register_curl_thunks(&mut self.wrapped_symbols);
        darwin::register_darwin_thunks(self);
        metal::register_metal_thunks(self);
        self.register_thunk("dlsym", thunk_dlsym);
        self.register_thunk("sigwait", thunk_sigwait);
        self.register_thunk("__libc_start_main", thunk___libc_start_main);
        self.register_thunk_address(0x7f000fff, thunk_exit);
        self.register_thunk("malloc", thunk_malloc);
        self.register_thunk("realloc", thunk_realloc);
        self.register_thunk("calloc", thunk_calloc);
        self.register_thunk("ctime", thunk_ctime);
        self.register_thunk("ctime_r", thunk_ctime_r);
        self.register_thunk("strchr", thunk_strchr);
        self.register_thunk("strrchr", thunk_strrchr);
        self.register_thunk("getcwd", thunk_getcwd);
        self.register_thunk("uname", thunk_uname);
        self.register_thunk("printf", thunk_printf);
        self.register_thunk("vasprintf", thunk_vasprintf);
        self.register_thunk("asprintf", thunk_vasprintf);
        self.register_thunk("__vasprintf_chk", thunk_vasprintf);
        self.register_thunk("__asprintf_chk", thunk_vasprintf);
        self.register_thunk("vsnprintf", thunk_vsnprintf);
        self.register_thunk("snprintf", thunk_vsnprintf);
        self.register_thunk("sprintf", thunk_vsnprintf);
        self.register_thunk("__vsnprintf_chk", thunk_vsnprintf);
        self.register_thunk("__snprintf_chk", thunk_vsnprintf);
        self.register_thunk("__sprintf_chk", thunk_vsnprintf);
        self.register_thunk("time", thunk_time);
        self.register_thunk("localtime_r", thunk_localtime_r);
        self.register_thunk("strftime", thunk_strftime);
        self.register_thunk("getpwuid", thunk_getpwuid);
        self.register_thunk("getpwuid_r", thunk_getpwuid_r);
        self.register_thunk("__getpwuid_r", thunk_getpwuid_r);
        self.register_thunk("getuid", thunk_getuid);
        self.register_thunk("geteuid", thunk_geteuid);
        self.register_thunk("getgid", thunk_getgid);
        self.register_thunk("getegid", thunk_getegid);
        self.register_thunk("__geteuid", thunk_geteuid);
        self.register_thunk("__getuid", thunk_getuid);
        self.register_thunk("fopen", thunk_fopen);
        self.register_thunk("fopen64", thunk_fopen);
        self.register_thunk("fgets", thunk_fgets);
        self.register_thunk("getc_unlocked", thunk_getc_unlocked);
        self.register_thunk("fgetc_unlocked", thunk_getc_unlocked);
        self.register_thunk("getc", thunk_getc_unlocked);
        self.register_thunk("fgetc", thunk_getc_unlocked);
        self.register_thunk("memcpy", thunk_memcpy);
        self.register_thunk("memmove", thunk_memmove);
        self.register_thunk("strtoul", thunk_strtoul);
        self.register_thunk("__isoc23_strtoul", thunk_strtoul);
        self.register_thunk("strtol", thunk_strtoul);
        self.register_thunk("__isoc23_strtol", thunk_strtoul);
        self.register_thunk("fclose", thunk_fclose);
        self.register_thunk("free", thunk_free);
        self.register_thunk("puts", thunk_puts);
        self.register_thunk("fputs", thunk_fputs);
        self.register_thunk("fputs_unlocked", thunk_fputs);
        self.register_thunk("putchar", thunk_putchar);
        self.register_thunk("putchar_unlocked", thunk_putchar);
        self.register_thunk("fputc", thunk_putchar);
        self.register_thunk("fputc_unlocked", thunk_putchar);
        self.register_thunk("fwrite", thunk_fwrite);
        self.register_thunk("fwrite_unlocked", thunk_fwrite);
        self.register_thunk("strlen", thunk_strlen);
        self.register_thunk("memcpy", thunk_memcpy);
        self.register_thunk("memset", thunk_memset);
        self.register_thunk("strcmp", thunk_strcmp);
        self.register_thunk("memcmp", thunk_memcmp);
        self.register_thunk("bcmp", thunk_memcmp);
        self.register_thunk("strcpy", thunk_strcpy);
        self.register_thunk("strncpy", thunk_strncpy);
        self.register_thunk("stpcpy", thunk_stpcpy);
        self.register_thunk("stpncpy", thunk_stpncpy);
        self.register_thunk("strcat", thunk_strcat);
        self.register_thunk("strdup", thunk_strdup);
        self.register_thunk("getopt", thunk_getopt);
        self.register_thunk("getopt_long", thunk_getopt_long);
        self.register_thunk("getopt_long_only", thunk_getopt_long);
        self.register_thunk("g_build_filename", thunk_g_build_filename);
        self.register_thunk("g_path_get_dirname", thunk_g_path_get_dirname);
        self.register_thunk("g_get_user_config_dir", thunk_g_get_user_config_dir);
        self.register_thunk("g_get_user_data_dir", thunk_g_get_user_data_dir);
        self.register_thunk("g_file_test", thunk_g_file_test);
        self.register_thunk("__errno_location", thunk___errno_location);
        self.register_thunk("write", thunk_write);
        self.register_thunk("writev", thunk_writev);
        self.register_thunk("read", thunk_read);
        self.register_thunk("open", thunk_open);
        self.register_thunk("open64", thunk_open);
        self.register_thunk("openat", thunk_openat);
        self.register_thunk("openat64", thunk_openat);
        self.register_thunk("__openat_2", thunk_openat);
        self.register_thunk("close", thunk_close);
        self.register_thunk("sendfile", thunk_sendfile);
        self.register_thunk("sendfile64", thunk_sendfile);
        self.register_thunk("sysconf", thunk_sysconf);
        self.register_thunk("pthread_attr_getstack", thunk_pthread_attr_getstack);
        self.register_thunk("pthread_getattr_np", thunk_pthread_getattr_np);
        self.register_thunk("pthread_self", thunk_pthread_self);
        self.register_thunk("stat", thunk_stat64);
        self.register_thunk("stat64", thunk_stat64);
        self.register_thunk("__xstat", thunk_xstat);
        self.register_thunk("__xstat64", thunk_xstat);
        self.register_thunk("lstat", thunk_stat64);
        self.register_thunk("lstat64", thunk_stat64);
        self.register_thunk("__lxstat", thunk_xstat);
        self.register_thunk("__lxstat64", thunk_xstat);
        self.register_thunk("fstat", thunk_fstat64);
        self.register_thunk("fstat64", thunk_fstat64);
        self.register_thunk("__fxstat", thunk_fxstat);
        self.register_thunk("__fxstat64", thunk_fxstat);
        self.register_thunk("opendir", thunk_opendir);
        self.register_thunk("opendir64", thunk_opendir);
        self.register_thunk("readdir", thunk_readdir);
        self.register_thunk("readdir64", thunk_readdir);
        self.register_thunk("closedir", thunk_closedir);
        self.register_thunk("closedir64", thunk_closedir);
        self.register_thunk("exit", thunk_exit);
        self.register_thunk("exit_group", thunk_exit);
        self.register_thunk("_exit", thunk_exit);
        self.register_thunk("_Exit", thunk_exit);
        self.register_thunk("abort", thunk_abort);
    }
}

#[allow(non_snake_case)]
pub fn thunk___libc_start_main(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let main_ptr = ctx.get_x(0);
    let argc = ctx.get_x(1);
    let argv = ctx.get_x(2);

    if ctx.tpidr_el0 == 0 {
        let tls_addr = mem.map_anonymous(0, 4096).unwrap_or(0);
        ctx.tpidr_el0 = tls_addr;
    }

    let envp = ctx.get_x(3);
    let envp_ptr = if envp != 0 { envp } else { argv + (argc + 1) * 8 };

    let effective_main = if main_ptr < 0x400000 { main_ptr + 0x400000 } else { main_ptr };

    ctx.set_x(0, argc);
    ctx.set_x(1, argv);
    ctx.set_x(2, envp_ptr);
    ctx.set_x(30, 0x7f000fff);
    ctx.pc = effective_main;
    Ok(())
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

pub fn thunk_realloc(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let ptr = ctx.get_x(0);
    let size = ctx.get_x(1) as usize;
    if size == 0 {
        ctx.set_x(0, 0);
        return Ok(());
    }
    let page_size = 4096;
    let aligned_size = ((size + page_size - 1) / page_size) * page_size;
    let new_ptr = mem
        .map_anonymous(0, aligned_size)
        .map_err(|e| format!("realloc error: {}", e))?;
    if ptr != 0 {
        if let Ok(old_data) = mem.read(ptr, size) {
            let _ = mem.write(new_ptr, &old_data);
        }
    }
    ctx.set_x(0, new_ptr);
    Ok(())
}

pub fn thunk_calloc(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let nmemb = ctx.get_x(0) as usize;
    let size = ctx.get_x(1) as usize;
    let total = nmemb * size;
    if total == 0 {
        ctx.set_x(0, 0);
        return Ok(());
    }
    let page_size = 4096;
    let aligned_size = ((total + page_size - 1) / page_size) * page_size;
    let vaddr = mem
        .map_anonymous(0, aligned_size)
        .map_err(|e| format!("calloc error: {}", e))?;
    ctx.set_x(0, vaddr);
    Ok(())
}

pub fn thunk_ctime(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let _time_ptr = ctx.get_x(0);
    let formatted = "Thu Jan  1 00:00:00 1970\n\0";
    let buf_addr = 0x7f010600u64;
    let _ = mem.write(buf_addr, formatted.as_bytes());
    ctx.set_x(0, buf_addr);
    Ok(())
}

pub fn thunk_ctime_r(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let _time_ptr = ctx.get_x(0);
    let buf_addr = ctx.get_x(1);
    let formatted = "Thu Jan  1 00:00:00 1970\n\0";
    if buf_addr != 0 {
        let _ = mem.write(buf_addr, formatted.as_bytes());
    }
    ctx.set_x(0, buf_addr);
    Ok(())
}

pub fn thunk_strchr(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let s_ptr = ctx.get_x(0);
    let c = (ctx.get_x(1) & 0xff) as u8;
    if s_ptr == 0 {
        ctx.set_x(0, 0);
        return Ok(());
    }
    let s_str = String::from_utf8_lossy(&mem.read_string(s_ptr).unwrap_or_default()).to_string();
    let mut offset = 0u64;
    loop {
        if let Ok(b) = mem.read(s_ptr + offset, 1) {
            let ch = b[0];
            if ch == c {
                tracing::info!("[Thunk: strchr] s={:?} c={:?} -> found at offset {}", s_str, c as char, offset);
                ctx.set_x(0, s_ptr + offset);
                return Ok(());
            }
            if ch == 0 {
                break;
            }
            offset += 1;
        } else {
            break;
        }
    }
    tracing::info!("[Thunk: strchr] s={:?} c={:?} -> NULL", s_str, c as char);
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_strrchr(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let s_ptr = ctx.get_x(0);
    let c = (ctx.get_x(1) & 0xff) as u8;
    if s_ptr == 0 {
        ctx.set_x(0, 0);
        return Ok(());
    }
    let mut offset = 0u64;
    let mut last_match = None;
    loop {
        if let Ok(b) = mem.read(s_ptr + offset, 1) {
            let ch = b[0];
            if ch == c {
                last_match = Some(s_ptr + offset);
            }
            if ch == 0 {
                break;
            }
            offset += 1;
        } else {
            break;
        }
    }
    ctx.set_x(0, last_match.unwrap_or(0));
    Ok(())
}

pub fn thunk_getcwd(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let buf_ptr = ctx.get_x(0);
    let size = ctx.get_x(1) as usize;

    if let Ok(cwd) = std::env::current_dir() {
        let cwd_str = cwd.to_string_lossy();
        let mut cwd_bytes = cwd_str.as_bytes().to_vec();
        cwd_bytes.push(0);

        let dest_ptr = if buf_ptr == 0 {
            let alloc_size = if size == 0 { cwd_bytes.len() } else { size };
            let page_size = 4096;
            let aligned = ((alloc_size + page_size - 1) / page_size) * page_size;
            mem.map_anonymous(0, aligned).unwrap_or(0)
        } else {
            buf_ptr
        };

        if dest_ptr != 0 {
            let _ = mem.write(dest_ptr, &cwd_bytes);
            ctx.set_x(0, dest_ptr);
            return Ok(());
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_uname(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let buf_addr = ctx.get_x(0);
    if buf_addr != 0 {
        let zeros = [0u8; 390];
        let _ = mem.write(buf_addr, &zeros);
        let sysname = b"Linux\0";
        let nodename = b"maarch64\0";
        let release = b"6.1.0-maarch64\0";
        let version = b"#1 SMP PREEMPT\0";
        let machine = b"aarch64\0";

        let _ = mem.write(buf_addr + 0, sysname);
        let _ = mem.write(buf_addr + 65, nodename);
        let _ = mem.write(buf_addr + 130, release);
        let _ = mem.write(buf_addr + 195, version);
        let _ = mem.write(buf_addr + 260, machine);
        ctx.set_x(0, 0);
    } else {
        ctx.set_x(0, -1i64 as u64);
    }
    Ok(())
}

pub fn thunk_printf(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let fmt_ptr = ctx.get_x(0);
    if fmt_ptr == 0 {
        ctx.set_x(0, 0);
        return Ok(());
    }
    let fmt_bytes = mem.read_string(fmt_ptr).unwrap_or_default();
    let fmt_str = String::from_utf8_lossy(&fmt_bytes);

    let mut arg_idx = 1;
    let mut out = String::new();
    let chars: Vec<char> = fmt_str.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            let spec = chars[i + 1];
            match spec {
                's' => {
                    let str_ptr = ctx.get_x(arg_idx);
                    arg_idx += 1;
                    if str_ptr != 0 {
                        if let Ok(s_bytes) = mem.read_string(str_ptr) {
                            out.push_str(&String::from_utf8_lossy(&s_bytes));
                        }
                    }
                    i += 2;
                    continue;
                }
                'u' | 'd' | 'i' => {
                    let val = ctx.get_x(arg_idx);
                    arg_idx += 1;
                    out.push_str(&val.to_string());
                    i += 2;
                    continue;
                }
                'x' => {
                    let val = ctx.get_x(arg_idx);
                    arg_idx += 1;
                    out.push_str(&format!("{:x}", val));
                    i += 2;
                    continue;
                }
                'p' => {
                    let val = ctx.get_x(arg_idx);
                    arg_idx += 1;
                    out.push_str(&format!("{:#x}", val));
                    i += 2;
                    continue;
                }
                '%' => {
                    out.push('%');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(chars[i]);
        i += 1;
    }

    print!("{}", out);
    use std::io::Write;
    let _ = std::io::stdout().flush();
    ctx.set_x(0, out.len() as u64);
    Ok(())
}

pub fn thunk_time(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let t_ptr = ctx.get_x(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if t_ptr != 0 {
        let _ = mem.write(t_ptr, &(now as i64).to_le_bytes());
    }
    ctx.set_x(0, now as u64);
    Ok(())
}

pub fn thunk_localtime_r(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let _time_ptr = ctx.get_x(0);
    let tm_ptr = ctx.get_x(1);

    if tm_ptr != 0 {
        let zeros = [0u8; 56];
        let _ = mem.write(tm_ptr, &zeros);
        let hour = 12i32;
        let mday = 26i32;
        let mon = 6i32;
        let year = 126i32;
        let wday = 0i32;
        let yday = 206i32;

        let _ = mem.write(tm_ptr + 8, &hour.to_le_bytes());
        let _ = mem.write(tm_ptr + 12, &mday.to_le_bytes());
        let _ = mem.write(tm_ptr + 16, &mon.to_le_bytes());
        let _ = mem.write(tm_ptr + 20, &year.to_le_bytes());
        let _ = mem.write(tm_ptr + 24, &wday.to_le_bytes());
        let _ = mem.write(tm_ptr + 28, &yday.to_le_bytes());

        ctx.set_x(0, tm_ptr);
    } else {
        ctx.set_x(0, 0);
    }
    Ok(())
}

pub fn thunk_strftime(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let s_ptr = ctx.get_x(0);
    let max_size = ctx.get_x(1) as usize;

    let default_str = "Sun Jul 26 12:00:00 UTC 2026\0";
    if s_ptr != 0 && max_size > 0 {
        let bytes = default_str.as_bytes();
        let len = bytes.len().min(max_size);
        let _ = mem.write(s_ptr, &bytes[..len]);
        ctx.set_x(0, (len - 1) as u64);
    } else {
        ctx.set_x(0, 0);
    }
    Ok(())
}

pub fn thunk_getpwuid(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let uid = ctx.get_x(0);
    tracing::info!("[Thunk: getpwuid] uid={}", uid);
    let host_user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let user_nul = format!("{}\0", host_user).into_bytes();
    let name_bytes = unsafe {
        let pw = libc::getpwuid(uid as libc::uid_t);
        if !pw.is_null() && !(*pw).pw_name.is_null() {
            std::ffi::CStr::from_ptr((*pw).pw_name).to_bytes_with_nul().to_vec()
        } else if uid == 0 {
            b"root\0".to_vec()
        } else {
            user_nul
        }
    };
    let buf_addr = mem.map_anonymous(0, 4096).unwrap_or(0);
    if buf_addr != 0 {
        let name_addr = buf_addr + 128;
        let _ = mem.write(name_addr, &name_bytes);

        let _ = mem.write(buf_addr + 0, &name_addr.to_le_bytes()); // pw_name
        let _ = mem.write(buf_addr + 8, &name_addr.to_le_bytes()); // pw_passwd
        let _ = mem.write(buf_addr + 16, &(uid as u32).to_le_bytes()); // pw_uid
        let _ = mem.write(buf_addr + 20, &(uid as u32).to_le_bytes()); // pw_gid
        let _ = mem.write(buf_addr + 24, &name_addr.to_le_bytes()); // pw_gecos
        let _ = mem.write(buf_addr + 32, &name_addr.to_le_bytes()); // pw_dir
        let _ = mem.write(buf_addr + 40, &name_addr.to_le_bytes()); // pw_shell

        ctx.set_x(0, buf_addr);
    } else {
        ctx.set_x(0, 0);
    }
    Ok(())
}

pub fn thunk_getpwuid_r(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let uid = ctx.get_x(0);
    let pwd_ptr = ctx.get_x(1);
    let buf_ptr = ctx.get_x(2);
    let buflen = ctx.get_x(3);
    let result_ptr = ctx.get_x(4);
    tracing::info!("[Thunk: getpwuid_r] uid={} pwd_ptr=0x{:x} buf_ptr=0x{:x} len={} res_ptr=0x{:x}", uid, pwd_ptr, buf_ptr, buflen, result_ptr);

    if pwd_ptr != 0 && buf_ptr != 0 {
        let host_user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        let user_nul = format!("{}\0", host_user).into_bytes();
        let name_bytes = unsafe {
            let pw = libc::getpwuid(uid as libc::uid_t);
            if !pw.is_null() && !(*pw).pw_name.is_null() {
                std::ffi::CStr::from_ptr((*pw).pw_name).to_bytes_with_nul().to_vec()
            } else if uid == 0 {
                b"root\0".to_vec()
            } else {
                user_nul
            }
        };
        let _ = mem.write(buf_ptr, &name_bytes);

        let _ = mem.write(pwd_ptr + 0, &buf_ptr.to_le_bytes()); // pw_name
        let _ = mem.write(pwd_ptr + 8, &buf_ptr.to_le_bytes()); // pw_passwd
        let _ = mem.write(pwd_ptr + 16, &(uid as u32).to_le_bytes()); // pw_uid
        let _ = mem.write(pwd_ptr + 20, &(uid as u32).to_le_bytes()); // pw_gid
        let _ = mem.write(pwd_ptr + 24, &buf_ptr.to_le_bytes()); // pw_gecos
        let _ = mem.write(pwd_ptr + 32, &buf_ptr.to_le_bytes()); // pw_dir
        let _ = mem.write(pwd_ptr + 40, &buf_ptr.to_le_bytes()); // pw_shell

        if result_ptr != 0 {
            let _ = mem.write(result_ptr, &pwd_ptr.to_le_bytes());
        }
        ctx.set_x(0, 0); // 0 = success
    } else {
        if result_ptr != 0 {
            let _ = mem.write(result_ptr, &0u64.to_le_bytes());
        }
        ctx.set_x(0, 2); // ENOENT
    }
    Ok(())
}

pub fn thunk_getuid(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let uid = unsafe { libc::getuid() };
    ctx.set_x(0, uid as u64);
    Ok(())
}

pub fn thunk_geteuid(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let euid = unsafe { libc::geteuid() };
    ctx.set_x(0, euid as u64);
    Ok(())
}

pub fn thunk_getgid(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let gid = unsafe { libc::getgid() };
    ctx.set_x(0, gid as u64);
    Ok(())
}

pub fn thunk_getegid(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let egid = unsafe { libc::getegid() };
    ctx.set_x(0, egid as u64);
    Ok(())
}

pub fn thunk_fopen(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let path_ptr = ctx.get_x(0);
    let mode_ptr = ctx.get_x(1);
    let path_bytes = mem.read_string(path_ptr).unwrap_or_default();
    let mode_bytes = if mode_ptr != 0 { mem.read_string(mode_ptr).unwrap_or_default() } else { Vec::new() };
    let path = String::from_utf8_lossy(&path_bytes);
    let mode = String::from_utf8_lossy(&mode_bytes);
    tracing::info!("[Thunk: fopen] path = {:?}, mode = {:?}", path, mode);

    let is_write = mode.contains('w') || mode.contains('a') || mode.contains('+');

    let content = if maarch64_core::vfs::Vfs::is_passwd_path(&path) {
        maarch64_core::vfs::Vfs::get_passwd_content()
    } else if is_write {
        Vec::new()
    } else if let Ok(mut file) = std::fs::File::open(&*path) {
        use std::io::Read;
        let mut buf = Vec::new();
        let _ = file.read_to_end(&mut buf);
        buf
    } else {
        ctx.set_x(0, 0);
        return Ok(());
    };

    let handle = mem.map_anonymous(0, 65536).unwrap_or(0);
    if handle != 0 {
        let buf_base = handle + 256;
        let buf_end = buf_base + content.len() as u64;
        if !content.is_empty() {
            let _ = mem.write(buf_base, &content);
        }

        let _ = mem.write(handle + 0, &0xfbad8000u32.to_le_bytes()); // _flags
        let _ = mem.write(handle + 8, &buf_base.to_le_bytes()); // _IO_read_ptr
        let _ = mem.write(handle + 16, &buf_end.to_le_bytes()); // _IO_read_end
        let _ = mem.write(handle + 24, &buf_base.to_le_bytes()); // _IO_read_base
        let _ = mem.write(handle + 56, &buf_base.to_le_bytes()); // _IO_buf_base
        let _ = mem.write(handle + 64, &buf_end.to_le_bytes()); // _IO_buf_end

        ctx.set_x(0, handle);
    } else {
        ctx.set_x(0, 0);
    }
    Ok(())
}

pub fn thunk_fgets(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let s_ptr = ctx.get_x(0);
    let size = ctx.get_x(1) as usize;
    let stream = ctx.get_x(2);

    if stream != 0 && s_ptr != 0 && size > 0 {
        let cur_ptr = u64::from_le_bytes(mem.read(stream + 8, 8).unwrap_or_default().try_into().unwrap_or([0; 8]));
        let end_ptr = u64::from_le_bytes(mem.read(stream + 16, 8).unwrap_or_default().try_into().unwrap_or([0; 8]));

        if cur_ptr < end_ptr {
            let mut bytes = Vec::new();
            let mut p = cur_ptr;
            while p < end_ptr && bytes.len() < size - 1 {
                let b = mem.read(p, 1).unwrap_or(vec![0])[0];
                bytes.push(b);
                p += 1;
                if b == b'\n' {
                    break;
                }
            }
            bytes.push(0);
            let _ = mem.write(stream + 8, &p.to_le_bytes());
            let _ = mem.write(s_ptr, &bytes);
            ctx.set_x(0, s_ptr);
            return Ok(());
        }
    }

    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_getc_unlocked(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let stream = ctx.get_x(0);
    if stream != 0 {
        let cur_ptr = u64::from_le_bytes(mem.read(stream + 8, 8).unwrap_or_default().try_into().unwrap_or([0; 8]));
        let end_ptr = u64::from_le_bytes(mem.read(stream + 16, 8).unwrap_or_default().try_into().unwrap_or([0; 8]));

        if cur_ptr < end_ptr {
            let b = mem.read(cur_ptr, 1).unwrap_or(vec![0])[0];
            let _ = mem.write(stream + 8, &(cur_ptr + 1).to_le_bytes());
            ctx.set_x(0, b as u64);
            return Ok(());
        }
    }
    ctx.set_x(0, -1i64 as u64);
    Ok(())
}

pub fn thunk_memcpy(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dest = ctx.get_x(0);
    let src = ctx.get_x(1);
    let n = ctx.get_x(2) as usize;
    if dest != 0 && src != 0 && n > 0 {
        if let Ok(bytes) = mem.read(src, n) {
            let _ = mem.write(dest, &bytes);
        }
    }
    ctx.set_x(0, dest);
    Ok(())
}

pub fn thunk_memmove(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    thunk_memcpy(ctx, mem)
}

pub fn thunk_strtoul(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let str_ptr = ctx.get_x(0);
    let endptr_ptr = ctx.get_x(1);
    let mut base = ctx.get_x(2) as u32;

    if str_ptr != 0 {
        if let Ok(s_bytes) = mem.read_string(str_ptr) {
            let mut i = 0;
            while i < s_bytes.len() && (s_bytes[i] as char).is_ascii_whitespace() {
                i += 1;
            }
            let initial_idx = i;
            if base == 0 {
                if i + 1 < s_bytes.len() && s_bytes[i] == b'0' && (s_bytes[i+1] == b'x' || s_bytes[i+1] == b'X') {
                    base = 16;
                    i += 2;
                } else if i < s_bytes.len() && s_bytes[i] == b'0' {
                    base = 8;
                } else {
                    base = 10;
                }
            } else if base == 16 && i + 1 < s_bytes.len() && s_bytes[i] == b'0' && (s_bytes[i+1] == b'x' || s_bytes[i+1] == b'X') {
                i += 2;
            }

            let _start = i;
            let mut val: u64 = 0;
            while i < s_bytes.len() {
                let digit = match s_bytes[i] {
                    b'0'..=b'9' => (s_bytes[i] - b'0') as u32,
                    b'a'..=b'z' => (s_bytes[i] - b'a' + 10) as u32,
                    b'A'..=b'Z' => (s_bytes[i] - b'A' + 10) as u32,
                    _ => 255,
                };
                if digit >= base {
                    break;
                }
                val = val.wrapping_mul(base as u64).wrapping_add(digit as u64);
                i += 1;
            }

            if endptr_ptr != 0 {
                let advanced = if i > initial_idx { str_ptr + i as u64 } else { str_ptr };
                let _ = mem.write(endptr_ptr, &advanced.to_le_bytes());
            }

            ctx.set_x(0, val);
            return Ok(());
        }
    }

    if endptr_ptr != 0 {
        let _ = mem.write(endptr_ptr, &str_ptr.to_le_bytes());
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_fclose(_ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    _ctx.set_x(0, 0);
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

pub fn thunk_fputs(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let str_ptr = ctx.get_x(0);
    let bytes = mem
        .read_string(str_ptr)
        .map_err(|e| format!("fputs error: {}", e))?;
    use std::io::Write;
    let _ = std::io::stdout().write_all(&bytes);
    let _ = std::io::stdout().flush();
    ctx.set_x(0, bytes.len() as u64);
    Ok(())
}

pub fn thunk_fwrite(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let ptr = ctx.get_x(0);
    let size = ctx.get_x(1);
    let nmemb = ctx.get_x(2);
    let total = (size * nmemb) as usize;
    if total > 0 {
        let bytes = mem.read(ptr, total).map_err(|e| format!("fwrite error: {}", e))?;
        use std::io::Write;
        let _ = std::io::stdout().write_all(&bytes);
        let _ = std::io::stdout().flush();
    }
    ctx.set_x(0, nmemb);
    Ok(())
}

pub fn thunk_putchar(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let c = ctx.get_x(0) as u8;
    use std::io::Write;
    let _ = std::io::stdout().write_all(&[c]);
    let _ = std::io::stdout().flush();
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
    unsafe { libc::fflush(std::ptr::null_mut()); }
    ctx.exited = true;
    ctx.exit_code = code;
    Ok(())
}

pub fn thunk_strcmp(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let s1_ptr = ctx.get_x(0);
    let s2_ptr = ctx.get_x(1);

    if s1_ptr == 0 && s2_ptr == 0 {
        ctx.set_x(0, 0);
        return Ok(());
    }
    if s1_ptr == 0 {
        ctx.set_x(0, (-1i64) as u64);
        return Ok(());
    }
    if s2_ptr == 0 {
        ctx.set_x(0, 1);
        return Ok(());
    }

    let s1 = mem.read_string(s1_ptr).unwrap_or_default();
    let s2 = mem.read_string(s2_ptr).unwrap_or_default();

    let res = match s1.cmp(&s2) {
        std::cmp::Ordering::Less => -1i64,
        std::cmp::Ordering::Equal => 0i64,
        std::cmp::Ordering::Greater => 1i64,
    };
    tracing::info!("[Thunk: strcmp] LR=0x{:x} s1={:?} s2={:?} -> ret {}", ctx.get_x(30), s1, s2, res);
    ctx.set_x(0, res as u64);
    Ok(())
}

pub fn thunk_memcmp(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let s1_ptr = ctx.get_x(0);
    let s2_ptr = ctx.get_x(1);
    let n = ctx.get_x(2) as usize;

    let b1 = mem.read(s1_ptr, n).map_err(|e| format!("memcmp s1: {}", e))?;
    let b2 = mem.read(s2_ptr, n).map_err(|e| format!("memcmp s2: {}", e))?;

    let res = match b1.cmp(&b2) {
        std::cmp::Ordering::Less => -1i64,
        std::cmp::Ordering::Equal => 0i64,
        std::cmp::Ordering::Greater => 1i64,
    };
    ctx.set_x(0, res as u64);
    Ok(())
}

pub fn thunk_abort(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.exited = true;
    ctx.exit_code = 134;
    Ok(())
}

pub fn thunk_strcpy(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dest = ctx.get_x(0);
    let src = ctx.get_x(1);
    let mut offset = 0;
    loop {
        let buf = mem.read(src + offset, 1).map_err(|e| format!("strcpy src error: {}", e))?;
        let b = buf[0];
        mem.write(dest + offset, &[b]).map_err(|e| format!("strcpy dest error: {}", e))?;
        if b == 0 {
            break;
        }
        offset += 1;
    }
    ctx.set_x(0, dest);
    Ok(())
}

pub fn thunk_strncpy(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dest = ctx.get_x(0);
    let src = ctx.get_x(1);
    let n = ctx.get_x(2) as usize;
    let mut null_seen = false;
    for i in 0..n {
        let b = if null_seen {
            0
        } else {
            let buf = mem.read(src + i as u64, 1).map_err(|e| format!("strncpy src error: {}", e))?;
            let byte = buf[0];
            if byte == 0 {
                null_seen = true;
            }
            byte
        };
        mem.write(dest + i as u64, &[b]).map_err(|e| format!("strncpy dest error: {}", e))?;
    }
    ctx.set_x(0, dest);
    Ok(())
}

static GETOPT_POS: Mutex<usize> = Mutex::new(1);

pub fn thunk_getopt(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let _argc = ctx.get_x(0);
    let argv_ptr = ctx.get_x(1);

    let mut optind = mem.read(0x7f010300, 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .unwrap_or(1);
    if optind == 0 {
        optind = 1;
    }

    if let Ok(arg_bytes) = mem.read(argv_ptr + (optind as u64) * 8, 8) {
        let arg_addr = u64::from_le_bytes(arg_bytes.try_into().unwrap());
        if arg_addr != 0 {
            if let Ok(str_bytes) = mem.read_string(arg_addr) {
                let s = String::from_utf8_lossy(&str_bytes);
                if s.starts_with('-') && s != "-" && s != "--" {
                    let mut pos_guard = GETOPT_POS.lock().unwrap();
                    let pos = *pos_guard;
                    if pos < s.len() {
                        let ch = s.as_bytes()[pos];
                        if pos + 1 < s.len() {
                            *pos_guard = pos + 1;
                        } else {
                            *pos_guard = 1;
                            optind += 1;
                            let _ = mem.write(0x7f010300, &optind.to_le_bytes());
                        }
                        ctx.set_x(0, ch as u64);
                        return Ok(());
                    }
                }
            }
        }
    }

    *GETOPT_POS.lock().unwrap() = 1;
    let _ = mem.write(0x7f010300, &optind.to_le_bytes());
    ctx.set_x(0, -1i64 as u64);
    Ok(())
}

pub fn thunk_getopt_long(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    thunk_getopt(ctx, mem)
}

pub fn thunk_stpcpy(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dest = ctx.get_x(0);
    let src = ctx.get_x(1);
    let mut offset = 0;
    loop {
        let buf = mem.read(src + offset, 1).map_err(|e| format!("stpcpy src error: {}", e))?;
        let b = buf[0];
        mem.write(dest + offset, &[b]).map_err(|e| format!("stpcpy dest error: {}", e))?;
        if b == 0 {
            break;
        }
        offset += 1;
    }
    ctx.set_x(0, dest + offset);
    Ok(())
}

pub fn thunk_stpncpy(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dest = ctx.get_x(0);
    let src = ctx.get_x(1);
    let n = ctx.get_x(2) as usize;
    let mut null_seen = false;
    let mut term_ptr = dest + n as u64;
    for i in 0..n {
        let b = if null_seen {
            0
        } else {
            let buf = mem.read(src + i as u64, 1).map_err(|e| format!("stpncpy src error: {}", e))?;
            let byte = buf[0];
            if byte == 0 {
                null_seen = true;
                term_ptr = dest + i as u64;
            }
            byte
        };
        mem.write(dest + i as u64, &[b]).map_err(|e| format!("stpncpy dest error: {}", e))?;
    }
    ctx.set_x(0, term_ptr);
    Ok(())
}

pub fn thunk_strcat(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dest = ctx.get_x(0);
    let src = ctx.get_x(1);
    let dest_len = mem.read_string(dest).map_err(|e| format!("strcat dest: {}", e))?.len() as u64;
    let mut offset = 0;
    loop {
        let buf = mem.read(src + offset, 1).map_err(|e| format!("strcat src error: {}", e))?;
        let b = buf[0];
        mem.write(dest + dest_len + offset, &[b]).map_err(|e| format!("strcat dest error: {}", e))?;
        if b == 0 {
            break;
        }
        offset += 1;
    }
    ctx.set_x(0, dest);
    Ok(())
}

pub fn thunk_strdup(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let src = ctx.get_x(0);
    let bytes = mem.read_string(src).map_err(|e| format!("strdup error: {}", e))?;
    let len = bytes.len() + 1;
    let page_size = 4096;
    let aligned_size = ((len + page_size - 1) / page_size) * page_size;
    let vaddr = mem.map_anonymous(0, aligned_size).map_err(|e| format!("strdup alloc error: {}", e))?;
    let mut buf = bytes;
    buf.push(0);
    mem.write(vaddr, &buf).map_err(|e| format!("strdup write error: {}", e))?;
    ctx.set_x(0, vaddr);
    Ok(())
}

#[allow(non_snake_case)]
pub fn thunk___errno_location(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0x7f010500);
    Ok(())
}

pub fn thunk_write(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let fd = ctx.get_x(0);
    let buf_addr = ctx.get_x(1);
    let count = ctx.get_x(2);
    let bytes = mem.read(buf_addr, count as usize).map_err(|e| format!("write read error: {}", e))?;
    if fd == 1 || fd == 2 {
        use std::io::Write;
        let _ = std::io::stdout().write_all(&bytes);
        let _ = std::io::stdout().flush();
    }
    ctx.set_x(0, count);
    Ok(())
}

pub fn thunk_writev(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let fd = ctx.get_x(0);
    let iov_ptr = ctx.get_x(1);
    let iovcnt = ctx.get_x(2) as usize;

    let mut total_written = 0u64;
    for i in 0..iovcnt {
        let cur_iov = iov_ptr + (i * 16) as u64;
        let base_bytes = mem.read(cur_iov, 8).map_err(|e| format!("writev iov_base read error: {}", e))?;
        let len_bytes = mem.read(cur_iov + 8, 8).map_err(|e| format!("writev iov_len read error: {}", e))?;
        let iov_base = u64::from_le_bytes(base_bytes.try_into().unwrap());
        let iov_len = u64::from_le_bytes(len_bytes.try_into().unwrap());

        if iov_len > 0 {
            let bytes = mem.read(iov_base, iov_len as usize).map_err(|e| format!("writev data read error: {}", e))?;
            if fd == 1 || fd == 2 {
                use std::io::Write;
                let _ = std::io::stdout().write_all(&bytes);
                let _ = std::io::stdout().flush();
            }
            total_written += iov_len;
        }
    }
    ctx.set_x(0, total_written);
    Ok(())
}

pub fn thunk_open(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let path_ptr = ctx.get_x(0);
    let flags = ctx.get_x(1) as i32;
    let mode = ctx.get_x(2) as u32;

    let path_bytes = mem.read_string(path_ptr).unwrap_or_default();
    let path = String::from_utf8_lossy(&path_bytes);

    let c_path = match std::ffi::CString::new(path.as_bytes()) {
        Ok(p) => p,
        Err(_) => {
            ctx.set_x(0, -1i64 as u64);
            return Ok(());
        }
    };

    let host_flags = maarch64_core::syscall::translate_open_flags(flags);
    let fd = unsafe { libc::open(c_path.as_ptr(), host_flags, mode) };
    if fd < 0 {
        let err = unsafe { *libc::__errno_location() };
        tracing::info!("[thunk_open] path={:?}, err={}", path, err);
        ctx.set_x(0, (-err as i64) as u64);
    } else {
        tracing::info!("[thunk_open] path={:?}, fd={}", path, fd);
        ctx.set_x(0, fd as u64);
    }
    Ok(())
}

pub fn thunk_openat(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let dirfd = ctx.get_x(0) as i32;
    let path_ptr = ctx.get_x(1);
    let flags = ctx.get_x(2) as i32;
    let mode = ctx.get_x(3) as u32;

    let path_bytes = mem.read_string(path_ptr).unwrap_or_default();
    let path = String::from_utf8_lossy(&path_bytes);

    let c_path = match std::ffi::CString::new(path.as_bytes()) {
        Ok(p) => p,
        Err(_) => {
            ctx.set_x(0, -1i64 as u64);
            return Ok(());
        }
    };

    let host_flags = maarch64_core::syscall::translate_open_flags(flags);
    let fd = unsafe { libc::openat(dirfd, c_path.as_ptr(), host_flags, mode) };
    if fd < 0 {
        let err = unsafe { *libc::__errno_location() };
        tracing::info!("[thunk_openat] dirfd={}, path={:?}, err={}", dirfd, path, err);
        ctx.set_x(0, (-err as i64) as u64);
    } else {
        tracing::info!("[thunk_openat] dirfd={}, path={:?}, fd={}", dirfd, path, fd);
        ctx.set_x(0, fd as u64);
    }
    Ok(())
}

pub fn thunk_read(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let fd = ctx.get_x(0) as i32;
    let buf_ptr = ctx.get_x(1);
    let count = ctx.get_x(2) as usize;

    let mut tmp_buf = vec![0u8; count];
    let current_offset = unsafe { libc::lseek(fd, 0, libc::SEEK_CUR) };
    let target_link = std::fs::read_link(format!("/proc/self/fd/{}", fd)).unwrap_or_default();
    let ret = unsafe { libc::read(fd, tmp_buf.as_mut_ptr() as *mut libc::c_void, count) };
    tracing::info!("[thunk_read] fd={} offset={} target={:?}, buf_ptr={:#x}, count={}, ret={}", fd, current_offset, target_link, buf_ptr, count, ret);
    if ret < 0 {
        let err = unsafe { *libc::__errno_location() };
        ctx.set_x(0, (-err as i64) as u64);
    } else {
        if ret > 0 {
            if let Err(e) = mem.write(buf_ptr, &tmp_buf[..ret as usize]) {
                tracing::error!("[thunk_read] mem.write FAILED at {:#x}: {:?}", buf_ptr, e);
            }
        }
        ctx.set_x(0, ret as u64);
    }
    Ok(())
}

pub fn thunk_close(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let fd = ctx.get_x(0) as i32;
    let ret = unsafe { libc::close(fd) };
    if ret < 0 {
        let err = unsafe { *libc::__errno_location() };
        ctx.set_x(0, (-err as i64) as u64);
    } else {
        ctx.set_x(0, 0);
    }
    Ok(())
}

pub fn thunk_sendfile(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let out_fd = ctx.get_x(0) as i32;
    let in_fd = ctx.get_x(1) as i32;
    let count = ctx.get_x(3) as usize;

    let mut buf = vec![0u8; 8192];
    let mut total_written: usize = 0;

    while total_written < count {
        let to_read = std::cmp::min(buf.len(), count - total_written);
        let nread = unsafe { libc::read(in_fd, buf.as_mut_ptr() as *mut libc::c_void, to_read) };
        if nread <= 0 {
            break;
        }
        let nwritten = unsafe { libc::write(out_fd, buf.as_ptr() as *const libc::c_void, nread as usize) };
        if nwritten <= 0 {
            break;
        }
        total_written += nwritten as usize;
    }

    ctx.set_x(0, total_written as u64);
    Ok(())
}

pub fn thunk_sysconf(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let name = ctx.get_x(0) as i32;
    let val = match name {
        29 | 30 => 4096, // _SC_PAGESIZE / _SC_PAGE_SIZE
        83 | 84 => 4,    // _SC_NPROCESSORS_CONF / _SC_NPROCESSORS_ONLN
        _ => unsafe { libc::sysconf(name) },
    };
    ctx.set_x(0, val as u64);
    Ok(())
}

pub fn thunk_pthread_attr_getstack(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let stackaddr_ptr = ctx.get_x(1);
    let stacksize_ptr = ctx.get_x(2);
    let default_stack_base: u64 = 0x7fffefe00000;
    let default_stack_size: u64 = 8 * 1024 * 1024; // 8MB

    if stackaddr_ptr != 0 {
        let _ = mem.write(stackaddr_ptr, &default_stack_base.to_le_bytes());
    }
    if stacksize_ptr != 0 {
        let _ = mem.write(stacksize_ptr, &default_stack_size.to_le_bytes());
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_pthread_getattr_np(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_pthread_self(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let tid = if ctx.tpidr_el0 != 0 { ctx.tpidr_el0 } else { 1 };
    ctx.set_x(0, tid);
    Ok(())
}

use std::os::unix::fs::{DirEntryExt, MetadataExt};
use std::sync::Mutex;

struct DirState {
    entries: Vec<(String, bool, u64)>,
    index: usize,
    buffer_addr: u64,
}

static DIR_HANDLES: Mutex<Option<HashMap<u64, DirState>>> = Mutex::new(None);
static NEXT_DIR_HANDLE: Mutex<u64> = Mutex::new(0x7f030000);

fn write_stat_struct(mem: &mut MemoryManager, buf_addr: u64, meta: &std::fs::Metadata) -> Result<(), String> {
    let mode = meta.mode();
    tracing::info!("[write_stat_struct] mode={:#o} ({:#x}) at buf_addr={:#x}", mode, mode, buf_addr);
    let zeros = [0u8; 128];
    let _ = mem.write(buf_addr, &zeros);

    let dev = meta.dev();
    let ino = meta.ino();
    let nlink = meta.nlink() as u32;
    let uid = meta.uid();
    let gid = meta.gid();
    let rdev = meta.rdev();
    let size = meta.size() as i64;
    let blksize = meta.blksize() as i32;
    let blocks = meta.blocks() as i64;
    let atime = meta.atime();
    let mtime = meta.mtime();
    let ctime = meta.ctime();

    let _ = mem.write(buf_addr + 0, &dev.to_le_bytes());
    let _ = mem.write(buf_addr + 8, &ino.to_le_bytes());
    let _ = mem.write(buf_addr + 16, &mode.to_le_bytes());
    let _ = mem.write(buf_addr + 20, &nlink.to_le_bytes());
    let _ = mem.write(buf_addr + 24, &uid.to_le_bytes());
    let _ = mem.write(buf_addr + 28, &gid.to_le_bytes());
    let _ = mem.write(buf_addr + 32, &rdev.to_le_bytes());
    let _ = mem.write(buf_addr + 48, &size.to_le_bytes());
    let _ = mem.write(buf_addr + 56, &blksize.to_le_bytes());
    let _ = mem.write(buf_addr + 64, &blocks.to_le_bytes());
    let _ = mem.write(buf_addr + 72, &atime.to_le_bytes());
    let _ = mem.write(buf_addr + 88, &mtime.to_le_bytes());
    let _ = mem.write(buf_addr + 104, &ctime.to_le_bytes());
    Ok(())
}

pub fn thunk_stat64(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    thunk_xstat(ctx, mem)
}

pub fn thunk_xstat(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let arg0 = ctx.get_x(0);
    let path_ptr;
    let buf_addr;
    if arg0 <= 3 {
        path_ptr = ctx.get_x(1);
        buf_addr = ctx.get_x(2);
    } else {
        path_ptr = ctx.get_x(0);
        buf_addr = ctx.get_x(1);
    }
    let path_bytes = mem.read_string(path_ptr).unwrap_or_default();
    let path_str = String::from_utf8_lossy(&path_bytes);
    let p = if path_str.is_empty() { "." } else { &path_str };
    if let Ok(meta) = std::fs::metadata(p) {
        let _ = write_stat_struct(mem, buf_addr, &meta);
        let mode_bytes = mem.read(buf_addr + 16, 4).unwrap_or_default();
        let read_mode = u32::from_le_bytes(mode_bytes.try_into().unwrap_or([0; 4]));
        tracing::info!("[stat DEBUG] path='{}', buf_addr={:#x}, wrote mode={:#o}, read_back={:#o}", p, buf_addr, meta.mode(), read_mode);
        ctx.set_x(0, 0);
    } else {
        ctx.set_x(0, 0xffffffffffffffff);
    }
    Ok(())
}

pub fn thunk_fstat64(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    thunk_fxstat(ctx, mem)
}

pub fn thunk_fxstat(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let arg0 = ctx.get_x(0);
    let (fd, buf_addr) = if arg0 <= 3 { (ctx.get_x(1) as i32, ctx.get_x(2)) } else { (ctx.get_x(0) as i32, ctx.get_x(1)) };
    
    use std::os::unix::io::FromRawFd;
    let file = unsafe { std::fs::File::from_raw_fd(fd) };
    let res = file.metadata();
    let _ = std::mem::forget(file);

    if let Ok(meta) = res {
        use std::os::unix::fs::MetadataExt;
        tracing::info!("[thunk_fxstat] arg0={} fd={} buf_addr={:#x} size={} mode={:#o}", arg0, fd, buf_addr, meta.size(), meta.mode());
        let _ = write_stat_struct(mem, buf_addr, &meta);
        ctx.set_x(0, 0);
    } else {
        tracing::info!("[thunk_fxstat] arg0={} fd={} err", arg0, fd);
        ctx.set_x(0, -1i64 as u64);
    }
    Ok(())
}

pub fn thunk_opendir(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let path_ptr = ctx.get_x(0);
    let path_bytes = mem.read_string(path_ptr).unwrap_or_default();
    let path_str = String::from_utf8_lossy(&path_bytes);
    let p = if path_str.is_empty() { "." } else { &path_str };

    if let Ok(read_dir) = std::fs::read_dir(p) {
        let mut entries = vec![
            (".".to_string(), true, 1u64),
            ("..".to_string(), true, 1u64),
        ];
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let ino = entry.ino();
            entries.push((name, is_dir, ino));
        }

        let mut handle_guard = NEXT_DIR_HANDLE.lock().unwrap();
        let handle = *handle_guard;
        *handle_guard += 0x1000;

        let buf_addr = mem.map_anonymous(0, 4096).unwrap_or(0);

        let mut map_guard = DIR_HANDLES.lock().unwrap();
        let map = map_guard.get_or_insert_with(HashMap::new);
        map.insert(handle, DirState {
            entries,
            index: 0,
            buffer_addr: buf_addr,
        });

        ctx.set_x(0, handle);
    } else {
        ctx.set_x(0, 0);
    }
    Ok(())
}

pub fn thunk_readdir(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let handle = ctx.get_x(0);
    let mut map_guard = DIR_HANDLES.lock().unwrap();
    if let Some(map) = map_guard.as_mut() {
        if let Some(state) = map.get_mut(&handle) {
            if state.index < state.entries.len() {
                let (name, is_dir, ino) = &state.entries[state.index];
                state.index += 1;

                let buf = state.buffer_addr;
                let _ = mem.write(buf + 0, &ino.to_le_bytes());
                let _ = mem.write(buf + 8, &(state.index as i64).to_le_bytes());
                let reclen = 276u16;
                let _ = mem.write(buf + 16, &reclen.to_le_bytes());
                let dtype = if *is_dir { 4u8 } else { 8u8 }; // DT_DIR=4, DT_REG=8
                let _ = mem.write(buf + 18, &[dtype]);

                let mut name_bytes = name.as_bytes().to_vec();
                name_bytes.push(0);
                let _ = mem.write(buf + 19, &name_bytes);

                ctx.set_x(0, buf);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_closedir(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let handle = ctx.get_x(0);
    let mut map_guard = DIR_HANDLES.lock().unwrap();
    if let Some(map) = map_guard.as_mut() {
        map.remove(&handle);
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_vasprintf(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let strp = ctx.get_x(0);
    let fmt_ptr = ctx.get_x(1);
    let ap_ptr = ctx.get_x(2);

    if strp == 0 || fmt_ptr == 0 {
        ctx.set_x(0, (-1i64) as u64);
        return Ok(());
    }

    let fmt_bytes = mem.read_string(fmt_ptr).unwrap_or_default();
    let fmt_str = String::from_utf8_lossy(&fmt_bytes);

    let mut out = String::new();
    let chars: Vec<char> = fmt_str.chars().collect();

    let read_u32 = |m: &MemoryManager, a: u64| -> u32 {
        m.read(a, 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).unwrap_or(0)
    };
    let read_u64 = |m: &MemoryManager, a: u64| -> u64 {
        m.read(a, 8).map(|b| u64::from_le_bytes(b.try_into().unwrap())).unwrap_or(0)
    };

    let mut reg_idx = 2;
    let mut gr_offs = if ap_ptr != 0 {
        read_u32(mem, ap_ptr + 24) as i32
    } else {
        0
    };
    let gr_top = if ap_ptr != 0 {
        read_u64(mem, ap_ptr + 8)
    } else {
        0
    };
    let mut stack_ptr = if ap_ptr != 0 {
        read_u64(mem, ap_ptr)
    } else {
        0
    };

    let mut get_next_arg = |reg_i: usize, mem: &MemoryManager, ctx: &CpuContext| -> u64 {
        if ap_ptr != 0 && gr_top != 0 {
            if gr_offs < 0 {
                let addr = (gr_top as i64 + gr_offs as i64) as u64;
                gr_offs += 8;
                read_u64(mem, addr)
            } else {
                let addr = stack_ptr;
                stack_ptr += 8;
                read_u64(mem, addr)
            }
        } else {
            ctx.get_x(reg_i)
        }
    };

    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            let spec = chars[i + 1];
            match spec {
                's' => {
                    let str_ptr = get_next_arg(reg_idx, mem, ctx);
                    reg_idx += 1;
                    if str_ptr != 0 {
                        if let Ok(s_bytes) = mem.read_string(str_ptr) {
                            out.push_str(&String::from_utf8_lossy(&s_bytes));
                        }
                    }
                    i += 2;
                    continue;
                }
                'u' | 'd' | 'i' => {
                    let val = get_next_arg(reg_idx, mem, ctx);
                    reg_idx += 1;
                    out.push_str(&val.to_string());
                    i += 2;
                    continue;
                }
                'x' => {
                    let val = get_next_arg(reg_idx, mem, ctx);
                    reg_idx += 1;
                    out.push_str(&format!("{:x}", val));
                    i += 2;
                    continue;
                }
                '%' => {
                    out.push('%');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(chars[i]);
        i += 1;
    }

    let buf_len = out.len() + 1;
    let page_size = 4096;
    let aligned_size = ((buf_len + page_size - 1) / page_size) * page_size;
    let vaddr = mem
        .map_anonymous(0, aligned_size)
        .map_err(|e| format!("vasprintf malloc error: {}", e))?;

    mem.write(vaddr, out.as_bytes())
        .map_err(|e| format!("vasprintf write error: {}", e))?;
    mem.write(vaddr + out.len() as u64, &[0u8])
        .map_err(|e| format!("vasprintf null byte write error: {}", e))?;

    let buf_ptr_bytes = vaddr.to_le_bytes();
    mem.write(strp, &buf_ptr_bytes)
        .map_err(|e| format!("vasprintf strp write error: {}", e))?;

    ctx.set_x(0, out.len() as u64);
    Ok(())
}

pub fn thunk_vsnprintf(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let buf_ptr = ctx.get_x(0);
    let max_len = ctx.get_x(1) as usize;
    let fmt_ptr = ctx.get_x(2);
    let ap_ptr = ctx.get_x(3);

    if fmt_ptr == 0 {
        ctx.set_x(0, 0);
        return Ok(());
    }

    let fmt_bytes = mem.read_string(fmt_ptr).unwrap_or_default();
    let fmt_str = String::from_utf8_lossy(&fmt_bytes);

    let read_u32 = |m: &MemoryManager, a: u64| -> u32 {
        m.read(a, 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).unwrap_or(0)
    };
    let read_u64 = |m: &MemoryManager, a: u64| -> u64 {
        m.read(a, 8).map(|b| u64::from_le_bytes(b.try_into().unwrap())).unwrap_or(0)
    };

    let mut reg_idx = 3;
    let mut gr_offs = if ap_ptr != 0 {
        read_u32(mem, ap_ptr + 24) as i32
    } else {
        0
    };
    let gr_top = if ap_ptr != 0 {
        read_u64(mem, ap_ptr + 8)
    } else {
        0
    };
    let mut stack_ptr = if ap_ptr != 0 {
        read_u64(mem, ap_ptr)
    } else {
        0
    };

    let mut get_next_arg = |reg_i: usize, mem: &MemoryManager, ctx: &CpuContext| -> u64 {
        if ap_ptr != 0 && gr_top != 0 {
            if gr_offs < 0 {
                let addr = (gr_top as i64 + gr_offs as i64) as u64;
                gr_offs += 8;
                read_u64(mem, addr)
            } else {
                let addr = stack_ptr;
                stack_ptr += 8;
                read_u64(mem, addr)
            }
        } else {
            ctx.get_x(reg_i)
        }
    };

    let mut out = String::new();
    let chars: Vec<char> = fmt_str.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            let spec = chars[i + 1];
            match spec {
                's' => {
                    let str_ptr = get_next_arg(reg_idx, mem, ctx);
                    reg_idx += 1;
                    if str_ptr != 0 {
                        if let Ok(s_bytes) = mem.read_string(str_ptr) {
                            out.push_str(&String::from_utf8_lossy(&s_bytes));
                        }
                    }
                    i += 2;
                    continue;
                }
                'u' | 'd' | 'i' => {
                    let val = get_next_arg(reg_idx, mem, ctx);
                    reg_idx += 1;
                    out.push_str(&val.to_string());
                    i += 2;
                    continue;
                }
                'x' => {
                    let val = get_next_arg(reg_idx, mem, ctx);
                    reg_idx += 1;
                    out.push_str(&format!("{:x}", val));
                    i += 2;
                    continue;
                }
                '%' => {
                    out.push('%');
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        out.push(chars[i]);
        i += 1;
    }

    if buf_ptr != 0 && max_len > 0 {
        let write_len = out.len().min(max_len - 1);
        let _ = mem.write(buf_ptr, &out.as_bytes()[..write_len]);
        let _ = mem.write(buf_ptr + write_len as u64, &[0u8]);
    }

    ctx.set_x(0, out.len() as u64);
    Ok(())
}

pub fn thunk_g_build_filename(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let mut parts = Vec::new();
    for i in 0..8 {
        let arg_ptr = ctx.get_x(i);
        if arg_ptr == 0 {
            break;
        }
        if let Ok(bytes) = mem.read_string(arg_ptr) {
            parts.push(String::from_utf8_lossy(&bytes).to_string());
        }
    }
    let joined = parts.join("/");
    let bytes = joined.as_bytes();
    let alloc_addr = mem.map_anonymous(0, ((bytes.len() + 1 + 4095) / 4096) * 4096).unwrap_or(0);
    if alloc_addr != 0 {
        let _ = mem.write(alloc_addr, bytes);
        let _ = mem.write(alloc_addr + bytes.len() as u64, &[0u8]);
        ctx.set_x(0, alloc_addr);
    } else {
        ctx.set_x(0, 0);
    }
    Ok(())
}

pub fn thunk_g_path_get_dirname(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let path_ptr = ctx.get_x(0);
    let path_str = if path_ptr != 0 {
        String::from_utf8_lossy(&mem.read_string(path_ptr).unwrap_or_default()).to_string()
    } else {
        ".".to_string()
    };
    let parent = std::path::Path::new(&path_str).parent().unwrap_or(std::path::Path::new(".")).to_string_lossy();
    let bytes = parent.as_bytes();
    let alloc_addr = mem.map_anonymous(0, 4096).unwrap_or(0);
    if alloc_addr != 0 {
        let _ = mem.write(alloc_addr, bytes);
        let _ = mem.write(alloc_addr + bytes.len() as u64, &[0u8]);
        ctx.set_x(0, alloc_addr);
    } else {
        ctx.set_x(0, 0);
    }
    Ok(())
}

pub fn thunk_g_get_user_config_dir(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let config_dir = format!("{}/.config", home);
    let bytes = config_dir.as_bytes();
    let alloc_addr = mem.map_anonymous(0, 4096).unwrap_or(0);
    if alloc_addr != 0 {
        let _ = mem.write(alloc_addr, bytes);
        let _ = mem.write(alloc_addr + bytes.len() as u64, &[0u8]);
        ctx.set_x(0, alloc_addr);
    } else {
        ctx.set_x(0, 0);
    }
    Ok(())
}

pub fn thunk_g_get_user_data_dir(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let data_dir = format!("{}/.local/share", home);
    let bytes = data_dir.as_bytes();
    let alloc_addr = mem.map_anonymous(0, 4096).unwrap_or(0);
    if alloc_addr != 0 {
        let _ = mem.write(alloc_addr, bytes);
        let _ = mem.write(alloc_addr + bytes.len() as u64, &[0u8]);
        ctx.set_x(0, alloc_addr);
    } else {
        ctx.set_x(0, 0);
    }
    Ok(())
}

pub fn thunk_g_file_test(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let path_ptr = ctx.get_x(0);
    if path_ptr != 0 {
        let path_str = String::from_utf8_lossy(&mem.read_string(path_ptr).unwrap_or_default()).to_string();
        let exists = std::path::Path::new(&path_str).exists();
        ctx.set_x(0, if exists { 1 } else { 0 });
    } else {
        ctx.set_x(0, 0);
    }
    Ok(())
}

fn write_cpp_string(mem: &mut MemoryManager, ret_ptr: u64, val: &str) -> Result<(), String> {
    if ret_ptr == 0 {
        return Ok(());
    }
    let page_base = ret_ptr & !0xfff;
    let _ = mem.map_anonymous(page_base, 4096);
    let bytes = val.as_bytes();
    let len = bytes.len();
    let sso_buf_addr = ret_ptr + 16;
    if len < 16 {
        let _ = mem.write(ret_ptr, &sso_buf_addr.to_le_bytes());
        let _ = mem.write(ret_ptr + 8, &(len as u64).to_le_bytes());
        let mut local_buf = [0u8; 16];
        local_buf[..len].copy_from_slice(bytes);
        let _ = mem.write(sso_buf_addr, &local_buf);
    } else {
        let heap_addr = mem.map_anonymous(0, ((len + 1 + 4095) / 4096) * 4096).unwrap_or(0);
        let _ = mem.write(heap_addr, bytes);
        let _ = mem.write(heap_addr + len as u64, &[0u8]);
        let _ = mem.write(ret_ptr, &heap_addr.to_le_bytes());
        let _ = mem.write(ret_ptr + 8, &(len as u64).to_le_bytes());
    }
    Ok(())
}

fn read_u64_helper(mem: &MemoryManager, addr: u64) -> Result<u64, String> {
    let bytes = mem.read(addr, 8).map_err(|e| e.to_string())?;
    Ok(u64::from_le_bytes(bytes.try_into().unwrap_or([0; 8])))
}

pub fn thunk_glibmm_path_get_dirname(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let ret_ptr = ctx.get_x(0);
    let path_arg_ptr = ctx.get_x(1);
    tracing::info!("[thunk_glibmm_path_get_dirname] ret_ptr={:#x}, path_arg_ptr={:#x}", ret_ptr, path_arg_ptr);
    let path_str = if path_arg_ptr != 0 {
        if let Ok(p_str_ptr) = read_u64_helper(mem, path_arg_ptr) {
            String::from_utf8_lossy(&mem.read_string(p_str_ptr).unwrap_or_default()).to_string()
        } else {
            ".".to_string()
        }
    } else {
        ".".to_string()
    };
    let parent = std::path::Path::new(&path_str).parent().unwrap_or(std::path::Path::new(".")).to_string_lossy();
    tracing::info!("[thunk_glibmm_path_get_dirname] path_str={:?} -> parent={:?}", path_str, parent);
    write_cpp_string(mem, ret_ptr, &parent)?;
    ctx.set_x(0, ret_ptr);
    Ok(())
}

pub fn thunk_glibmm_get_user_config_dir(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let ret_ptr = ctx.get_x(0);
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let config_dir = format!("{}/.config", home);
    write_cpp_string(mem, ret_ptr, &config_dir)?;
    ctx.set_x(0, ret_ptr);
    Ok(())
}

pub fn thunk_glibmm_build_filename(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let ret_ptr = ctx.get_x(0);
    let p1_ptr = ctx.get_x(1);
    let p2_ptr = ctx.get_x(2);
    let mut parts = Vec::new();
    for p_ptr in [p1_ptr, p2_ptr] {
        if p_ptr != 0 {
            if let Ok(str_ptr) = read_u64_helper(mem, p_ptr) {
                if let Ok(bytes) = mem.read_string(str_ptr) {
                    parts.push(String::from_utf8_lossy(&bytes).to_string());
                }
            }
        }
    }
    let joined = parts.join("/");
    write_cpp_string(mem, ret_ptr, &joined)?;
    ctx.set_x(0, ret_ptr);
    Ok(())
}

pub fn thunk_glibmm_getenv(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let ret_ptr = ctx.get_x(0);
    let var_name_ptr = ctx.get_x(1);
    let name = if var_name_ptr != 0 {
        if let Ok(str_ptr) = read_u64_helper(mem, var_name_ptr) {
            String::from_utf8_lossy(&mem.read_string(str_ptr).unwrap_or_default()).to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    let val = std::env::var(&name).unwrap_or_default();
    write_cpp_string(mem, ret_ptr, &val)?;
    ctx.set_x(0, ret_ptr);
    Ok(())
}

pub fn thunk_glibmm_file_test(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let path_arg_ptr = ctx.get_x(0);
    if path_arg_ptr != 0 {
        if let Ok(str_ptr) = read_u64_helper(mem, path_arg_ptr) {
            let path_str = String::from_utf8_lossy(&mem.read_string(str_ptr).unwrap_or_default()).to_string();
            let exists = std::path::Path::new(&path_str).exists();
            ctx.set_x(0, if exists { 1 } else { 0 });
            return Ok(());
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_glibmm_canonicalize_filename(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let ret_ptr = ctx.get_x(0);
    let path_arg_ptr = ctx.get_x(1);
    let path_str = if path_arg_ptr != 0 {
        if let Ok(str_ptr) = read_u64_helper(mem, path_arg_ptr) {
            String::from_utf8_lossy(&mem.read_string(str_ptr).unwrap_or_default()).to_string()
        } else {
            "/".to_string()
        }
    } else {
        "/".to_string()
    };
    write_cpp_string(mem, ret_ptr, &path_str)?;
    ctx.set_x(0, ret_ptr);
    Ok(())
}
