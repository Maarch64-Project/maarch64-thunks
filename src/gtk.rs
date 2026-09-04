#![allow(non_snake_case)]

use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use std::collections::HashMap;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::OnceLock;

pub struct GtkRegistry {
    loaded_libraries: Vec<libloading::Library>,
}

impl GtkRegistry {
    pub fn new() -> Self {
        let mut loaded_libraries = Vec::new();
        let libs_to_try = [
            "libgtk-3.so.0", "libgtk-3.so",
            "libgdk-3.so.0", "libgdk-3.so",
            "libglib-2.0.so.0", "libglib-2.0.so",
            "libgobject-2.0.so.0", "libgobject-2.0.so",
            "libcairo.so.2", "libcairo.so",
            "libpango-1.0.so.0", "libpango-1.0.so",
            "libpangocairo-1.0.so.0", "libpangocairo-1.0.so",
        ];

        for name in libs_to_try {
            if let Ok(lib) = unsafe { libloading::Library::new(name) } {
                tracing::info!("[Maarch64 Desktop Passthrough] Loaded host library: {}", name);
                loaded_libraries.push(lib);
            }
        }

        Self { loaded_libraries }
    }

    pub fn get_symbol<T>(&self, symbol: &[u8]) -> Option<libloading::Symbol<T>> {
        for lib in &self.loaded_libraries {
            if let Ok(sym) = unsafe { lib.get::<T>(symbol) } {
                return Some(sym);
            }
        }
        None
    }
}

static GTK_REGISTRY: OnceLock<GtkRegistry> = OnceLock::new();
pub fn get_gtk_registry() -> &'static GtkRegistry {
    GTK_REGISTRY.get_or_init(GtkRegistry::new)
}

// -----------------------------------------------------------------------------
// GTK3 Window & Lifecycle Thunks
// -----------------------------------------------------------------------------

pub fn thunk_gtk_init(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let argc_ptr = ctx.get_x(0) as *mut c_int;
    let argv_ptr = ctx.get_x(1) as *mut *mut *mut c_char;
    tracing::info!("[GTK3 Passthrough] gtk_init()");

    if let Some(init) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char)>(b"gtk_init\0") {
        let mut dummy_argc: c_int = 0;
        let mut dummy_argv: *mut *mut c_char = std::ptr::null_mut();
        unsafe {
            init(
                if argc_ptr.is_null() { &mut dummy_argc } else { argc_ptr },
                if argv_ptr.is_null() { &mut dummy_argv } else { argv_ptr },
            );
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_init_check(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let argc_ptr = ctx.get_x(0) as *mut c_int;
    let argv_ptr = ctx.get_x(1) as *mut *mut *mut c_char;
    tracing::info!("[GTK3 Passthrough] gtk_init_check()");

    if let Some(init_check) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int>(b"gtk_init_check\0") {
        let mut dummy_argc: c_int = 0;
        let mut dummy_argv: *mut *mut c_char = std::ptr::null_mut();
        let res = unsafe {
            init_check(
                if argc_ptr.is_null() { &mut dummy_argc } else { argc_ptr },
                if argv_ptr.is_null() { &mut dummy_argv } else { argv_ptr },
            )
        };
        ctx.set_x(0, res as i64 as u64);
        return Ok(());
    }
    ctx.set_x(0, 1);
    Ok(())
}

pub fn thunk_gtk_window_new(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let window_type = ctx.get_x(0) as c_int;
    tracing::info!("[GTK3 Passthrough] gtk_window_new(type={})", window_type);

    if let Some(window_new) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(c_int) -> *mut c_void>(b"gtk_window_new\0") {
        let ptr = unsafe { window_new(window_type) };
        ctx.set_x(0, ptr as u64);
        return Ok(());
    }
    ctx.set_x(0, 0x6666_0000);
    Ok(())
}

pub fn thunk_gtk_window_set_title(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let window_ptr = ctx.get_x(0) as *mut c_void;
    let title_ptr = ctx.get_x(1);
    let title_str = if title_ptr != 0 {
        mem.read_string(title_ptr).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_else(|_| "Antigravity IDE".to_string())
    } else {
        "Antigravity IDE".to_string()
    };
    tracing::info!("[GTK3 Passthrough] gtk_window_set_title(title={:?})", title_str);

    if let Some(set_title) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void, *const c_char)>(b"gtk_window_set_title\0") {
        if let Ok(c_title) = std::ffi::CString::new(title_str) {
            unsafe { set_title(window_ptr, c_title.as_ptr()) };
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_window_set_default_size(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let window_ptr = ctx.get_x(0) as *mut c_void;
    let width = ctx.get_x(1) as c_int;
    let height = ctx.get_x(2) as c_int;
    tracing::info!("[GTK3 Passthrough] gtk_window_set_default_size(w={}, h={})", width, height);

    if let Some(set_size) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void, c_int, c_int)>(b"gtk_window_set_default_size\0") {
        unsafe { set_size(window_ptr, width, height) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_window_present(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let window_ptr = ctx.get_x(0) as *mut c_void;
    tracing::info!("[GTK3 Passthrough] gtk_window_present(window={:?})", window_ptr);
    if let Some(present) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void)>(b"gtk_window_present\0") {
        unsafe { present(window_ptr) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_widget_show(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let widget_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(widget_show) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void)>(b"gtk_widget_show\0") {
        unsafe { widget_show(widget_ptr) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_widget_show_all(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let widget_ptr = ctx.get_x(0) as *mut c_void;
    tracing::info!("[GTK3 Passthrough] gtk_widget_show_all(widget={:?})", widget_ptr);
    if let Some(widget_show_all) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void)>(b"gtk_widget_show_all\0") {
        unsafe { widget_show_all(widget_ptr) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_widget_destroy(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let widget_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(widget_destroy) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void)>(b"gtk_widget_destroy\0") {
        unsafe { widget_destroy(widget_ptr) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_main(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[GTK3 Passthrough] gtk_main()");
    if let Some(main_fn) = get_gtk_registry().get_symbol::<unsafe extern "C" fn()>(b"gtk_main\0") {
        unsafe { main_fn() };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_main_quit(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[GTK3 Passthrough] gtk_main_quit()");
    if let Some(main_quit) = get_gtk_registry().get_symbol::<unsafe extern "C" fn()>(b"gtk_main_quit\0") {
        unsafe { main_quit() };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_main_iteration(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    if let Some(iteration) = get_gtk_registry().get_symbol::<unsafe extern "C" fn() -> c_int>(b"gtk_main_iteration\0") {
        let res = unsafe { iteration() };
        ctx.set_x(0, res as u64);
        return Ok(());
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_main_iteration_do(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let blocking = ctx.get_x(0) as c_int;
    if let Some(iteration_do) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(c_int) -> c_int>(b"gtk_main_iteration_do\0") {
        let res = unsafe { iteration_do(blocking) };
        ctx.set_x(0, res as u64);
        return Ok(());
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_events_pending(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    if let Some(pending) = get_gtk_registry().get_symbol::<unsafe extern "C" fn() -> c_int>(b"gtk_events_pending\0") {
        let res = unsafe { pending() };
        ctx.set_x(0, res as u64);
        return Ok(());
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_file_chooser_set_show_hidden(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let chooser = ctx.get_x(0) as *mut c_void;
    let show_hidden = ctx.get_x(1) as c_int;
    if let Some(set_show) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void, c_int)>(b"gtk_file_chooser_set_show_hidden\0") {
        unsafe { set_show(chooser, show_hidden) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_print_settings_set_page_ranges(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_button_new_with_label(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let label_ptr = ctx.get_x(0);
    let label_str = if label_ptr != 0 {
        mem.read_string(label_ptr).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_else(|_| "Button".to_string())
    } else {
        "Button".to_string()
    };
    if let Some(button_new) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*const c_char) -> *mut c_void>(b"gtk_button_new_with_label\0") {
        if let Ok(c_label) = std::ffi::CString::new(label_str) {
            let ptr = unsafe { button_new(c_label.as_ptr()) };
            ctx.set_x(0, ptr as u64);
            return Ok(());
        }
    }
    ctx.set_x(0, 0x6666_0008);
    Ok(())
}

pub fn thunk_gtk_label_new(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let text_ptr = ctx.get_x(0);
    let text_str = if text_ptr != 0 {
        mem.read_string(text_ptr).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_else(|_| "".to_string())
    } else {
        "".to_string()
    };
    if let Some(label_new) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*const c_char) -> *mut c_void>(b"gtk_label_new\0") {
        if let Ok(c_text) = std::ffi::CString::new(text_str) {
            let ptr = unsafe { label_new(c_text.as_ptr()) };
            ctx.set_x(0, ptr as u64);
            return Ok(());
        }
    }
    ctx.set_x(0, 0x6666_0010);
    Ok(())
}

pub fn thunk_gtk_container_add(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let container_ptr = ctx.get_x(0) as *mut c_void;
    let widget_ptr = ctx.get_x(1) as *mut c_void;
    if let Some(container_add) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void, *mut c_void)>(b"gtk_container_add\0") {
        unsafe { container_add(container_ptr, widget_ptr) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

// -----------------------------------------------------------------------------
// GDK Display & Screen Thunks
// -----------------------------------------------------------------------------

pub fn thunk_gdk_display_get_default(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    if let Some(get_default) = get_gtk_registry().get_symbol::<unsafe extern "C" fn() -> *mut c_void>(b"gdk_display_get_default\0") {
        let ptr = unsafe { get_default() };
        ctx.set_x(0, ptr as u64);
        return Ok(());
    }
    ctx.set_x(0, 0x7777_0001);
    Ok(())
}

pub fn thunk_gdk_x11_display_get_xdisplay(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let display_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(get_xdisplay) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void) -> *mut c_void>(b"gdk_x11_display_get_xdisplay\0") {
        let ptr = unsafe { get_xdisplay(display_ptr) };
        ctx.set_x(0, ptr as u64);
        return Ok(());
    }
    ctx.set_x(0, 0x7777_0002);
    Ok(())
}

pub fn thunk_gdk_screen_get_default(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    if let Some(get_screen) = get_gtk_registry().get_symbol::<unsafe extern "C" fn() -> *mut c_void>(b"gdk_screen_get_default\0") {
        let ptr = unsafe { get_screen() };
        ctx.set_x(0, ptr as u64);
        return Ok(());
    }
    ctx.set_x(0, 0x7777_0003);
    Ok(())
}

// -----------------------------------------------------------------------------
// GLib Main Loop, SList, KeyFile Thunks
// -----------------------------------------------------------------------------

pub fn thunk_g_main_context_default(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    if let Some(get_default) = get_gtk_registry().get_symbol::<unsafe extern "C" fn() -> *mut c_void>(b"g_main_context_default\0") {
        let ptr = unsafe { get_default() };
        ctx.set_x(0, ptr as u64);
        return Ok(());
    }
    ctx.set_x(0, 0x8888_0001);
    Ok(())
}

pub fn thunk_g_main_context_iteration(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let context = ctx.get_x(0) as *mut c_void;
    let may_block = ctx.get_x(1) as c_int;
    if let Some(iter) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void, c_int) -> c_int>(b"g_main_context_iteration\0") {
        let res = unsafe { iter(context, may_block) };
        ctx.set_x(0, res as u64);
        return Ok(());
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_g_main_context_pending(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let context = ctx.get_x(0) as *mut c_void;
    if let Some(pending) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void) -> c_int>(b"g_main_context_pending\0") {
        let res = unsafe { pending(context) };
        ctx.set_x(0, res as u64);
        return Ok(());
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_g_main_loop_new(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let context = ctx.get_x(0) as *mut c_void;
    let is_running = ctx.get_x(1) as c_int;
    if let Some(new_loop) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_void>(b"g_main_loop_new\0") {
        let ptr = unsafe { new_loop(context, is_running) };
        ctx.set_x(0, ptr as u64);
        return Ok(());
    }
    ctx.set_x(0, 0x8888_0002);
    Ok(())
}

pub fn thunk_g_main_loop_run(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let loop_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(run) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void)>(b"g_main_loop_run\0") {
        unsafe { run(loop_ptr) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_g_main_loop_quit(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let loop_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(quit) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void)>(b"g_main_loop_quit\0") {
        unsafe { quit(loop_ptr) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_g_main_loop_unref(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let loop_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(unref) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void)>(b"g_main_loop_unref\0") {
        unsafe { unref(loop_ptr) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_g_slist_index(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let list = ctx.get_x(0) as *mut c_void;
    let data = ctx.get_x(1) as *mut c_void;
    if let Some(idx_fn) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int>(b"g_slist_index\0") {
        let res = unsafe { idx_fn(list, data) };
        ctx.set_x(0, res as i64 as u64);
        return Ok(());
    }
    ctx.set_x(0, (-1i64) as u64);
    Ok(())
}

pub fn thunk_g_slist_free(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let list = ctx.get_x(0) as *mut c_void;
    if let Some(free_fn) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void)>(b"g_slist_free\0") {
        unsafe { free_fn(list) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_g_list_free(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let list = ctx.get_x(0) as *mut c_void;
    if let Some(free_fn) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void)>(b"g_list_free\0") {
        unsafe { free_fn(list) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_g_file_get_type(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    if let Some(get_type) = get_gtk_registry().get_symbol::<unsafe extern "C" fn() -> usize>(b"g_file_get_type\0") {
        let res = unsafe { get_type() };
        ctx.set_x(0, res as u64);
        return Ok(());
    }
    ctx.set_x(0, 0x8888_0003);
    Ok(())
}

pub fn thunk_g_key_file_new(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    if let Some(new_fn) = get_gtk_registry().get_symbol::<unsafe extern "C" fn() -> *mut c_void>(b"g_key_file_new\0") {
        let ptr = unsafe { new_fn() };
        ctx.set_x(0, ptr as u64);
        return Ok(());
    }
    ctx.set_x(0, 0x8888_0004);
    Ok(())
}

pub fn thunk_g_key_file_load_from_data(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 1); // 1 = TRUE
    Ok(())
}

pub fn thunk_g_key_file_to_data(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let len_out = ctx.get_x(2);
    if len_out != 0 {
        let _ = mem.write(len_out, &0usize.to_le_bytes());
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_g_key_file_free(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let kf = ctx.get_x(0) as *mut c_void;
    if let Some(free_fn) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void)>(b"g_key_file_free\0") {
        unsafe { free_fn(kf) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_g_utf8_pointer_to_offset(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let str_ptr = ctx.get_x(0);
    let pos_ptr = ctx.get_x(1);
    let offset = pos_ptr.saturating_sub(str_ptr);
    ctx.set_x(0, offset);
    Ok(())
}

pub fn thunk_g_utf8_offset_to_pointer(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let str_ptr = ctx.get_x(0);
    let offset = ctx.get_x(1);
    ctx.set_x(0, str_ptr + offset);
    Ok(())
}

// -----------------------------------------------------------------------------
// Cairo & Pango Thunks
// -----------------------------------------------------------------------------

pub fn thunk_cairo_scale(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_cairo_pattern_destroy(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let pat = ctx.get_x(0) as *mut c_void;
    if let Some(destroy_fn) = get_gtk_registry().get_symbol::<unsafe extern "C" fn(*mut c_void)>(b"cairo_pattern_destroy\0") {
        unsafe { destroy_fn(pat) };
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_pango_attr_list_get_iterator(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0x9999_0001);
    Ok(())
}

pub fn thunk_pango_attr_iterator_range(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let start_ptr = ctx.get_x(1);
    let end_ptr = ctx.get_x(2);
    if start_ptr != 0 {
        let _ = mem.write(start_ptr, &0i32.to_le_bytes());
    }
    if end_ptr != 0 {
        let _ = mem.write(end_ptr, &i32::MAX.to_le_bytes());
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_pango_attr_iterator_get(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_pango_attr_iterator_next(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0); // 0 = FALSE (no more attributes)
    Ok(())
}

pub fn thunk_pango_attr_iterator_destroy(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_pango_attr_list_unref(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    ctx.set_x(0, 0);
    Ok(())
}

// -----------------------------------------------------------------------------
// Register All GTK / GDK / GLib / Cairo / Pango Thunks
// -----------------------------------------------------------------------------

pub fn register_gtk_thunks(map: &mut HashMap<String, super::ThunkFn>) {
    map.insert("gtk_init".to_string(), thunk_gtk_init);
    map.insert("gtk_init_check".to_string(), thunk_gtk_init_check);
    map.insert("gtk_window_new".to_string(), thunk_gtk_window_new);
    map.insert("gtk_window_set_title".to_string(), thunk_gtk_window_set_title);
    map.insert("gtk_window_set_default_size".to_string(), thunk_gtk_window_set_default_size);
    map.insert("gtk_window_present".to_string(), thunk_gtk_window_present);
    map.insert("gtk_widget_show".to_string(), thunk_gtk_widget_show);
    map.insert("gtk_widget_show_all".to_string(), thunk_gtk_widget_show_all);
    map.insert("gtk_widget_destroy".to_string(), thunk_gtk_widget_destroy);
    map.insert("gtk_main".to_string(), thunk_gtk_main);
    map.insert("gtk_main_quit".to_string(), thunk_gtk_main_quit);
    map.insert("gtk_main_iteration".to_string(), thunk_gtk_main_iteration);
    map.insert("gtk_main_iteration_do".to_string(), thunk_gtk_main_iteration_do);
    map.insert("gtk_events_pending".to_string(), thunk_gtk_events_pending);
    map.insert("gtk_file_chooser_set_show_hidden".to_string(), thunk_gtk_file_chooser_set_show_hidden);
    map.insert("gtk_print_settings_set_page_ranges".to_string(), thunk_gtk_print_settings_set_page_ranges);
    map.insert("gtk_button_new_with_label".to_string(), thunk_gtk_button_new_with_label);
    map.insert("gtk_label_new".to_string(), thunk_gtk_label_new);
    map.insert("gtk_container_add".to_string(), thunk_gtk_container_add);

    map.insert("gdk_display_get_default".to_string(), thunk_gdk_display_get_default);
    map.insert("gdk_x11_display_get_xdisplay".to_string(), thunk_gdk_x11_display_get_xdisplay);
    map.insert("gdk_screen_get_default".to_string(), thunk_gdk_screen_get_default);

    map.insert("g_main_context_default".to_string(), thunk_g_main_context_default);
    map.insert("g_main_context_iteration".to_string(), thunk_g_main_context_iteration);
    map.insert("g_main_context_pending".to_string(), thunk_g_main_context_pending);
    map.insert("g_main_loop_new".to_string(), thunk_g_main_loop_new);
    map.insert("g_main_loop_run".to_string(), thunk_g_main_loop_run);
    map.insert("g_main_loop_quit".to_string(), thunk_g_main_loop_quit);
    map.insert("g_main_loop_unref".to_string(), thunk_g_main_loop_unref);
    map.insert("g_slist_index".to_string(), thunk_g_slist_index);
    map.insert("g_slist_free".to_string(), thunk_g_slist_free);
    map.insert("g_list_free".to_string(), thunk_g_list_free);
    map.insert("g_file_get_type".to_string(), thunk_g_file_get_type);
    map.insert("g_key_file_new".to_string(), thunk_g_key_file_new);
    map.insert("g_key_file_load_from_data".to_string(), thunk_g_key_file_load_from_data);
    map.insert("g_key_file_to_data".to_string(), thunk_g_key_file_to_data);
    map.insert("g_key_file_free".to_string(), thunk_g_key_file_free);
    map.insert("g_utf8_pointer_to_offset".to_string(), thunk_g_utf8_pointer_to_offset);
    map.insert("g_utf8_offset_to_pointer".to_string(), thunk_g_utf8_offset_to_pointer);

    map.insert("cairo_scale".to_string(), thunk_cairo_scale);
    map.insert("cairo_pattern_destroy".to_string(), thunk_cairo_pattern_destroy);

    map.insert("pango_attr_list_get_iterator".to_string(), thunk_pango_attr_list_get_iterator);
    map.insert("pango_attr_iterator_range".to_string(), thunk_pango_attr_iterator_range);
    map.insert("pango_attr_iterator_get".to_string(), thunk_pango_attr_iterator_get);
    map.insert("pango_attr_iterator_next".to_string(), thunk_pango_attr_iterator_next);
    map.insert("pango_attr_iterator_destroy".to_string(), thunk_pango_attr_iterator_destroy);
    map.insert("pango_attr_list_unref".to_string(), thunk_pango_attr_list_unref);
}

