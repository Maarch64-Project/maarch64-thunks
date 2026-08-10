#![allow(non_snake_case)]

use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use std::collections::HashMap;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::OnceLock;

pub struct GtkRegistry {
    loaded_libraries: HashMap<String, libloading::Library>,
}

impl GtkRegistry {
    pub fn new() -> Self {
        let mut loaded_libraries = HashMap::new();
        let libs_to_try = [
            ("libgtk-3.so.0", "libgtk-3.so"),
            ("libgtk-3.so", "libgtk-3.so"),
        ];

        for (name, alt_name) in libs_to_try {
            if let Ok(lib) = unsafe { libloading::Library::new(name) } {
                tracing::info!("[Maarch64 GTK3 Passthrough] Successfully loaded host GTK3 library: {}", name);
                loaded_libraries.insert(name.to_string(), lib);
                break;
            } else if let Ok(lib) = unsafe { libloading::Library::new(alt_name) } {
                tracing::info!("[Maarch64 GTK3 Passthrough] Successfully loaded host GTK3 library: {}", alt_name);
                loaded_libraries.insert(name.to_string(), lib);
                break;
            }
        }

        Self { loaded_libraries }
    }

    pub fn get_library(&self) -> Option<&libloading::Library> {
        self.loaded_libraries.values().next()
    }
}

static GTK_REGISTRY: OnceLock<GtkRegistry> = OnceLock::new();
pub fn get_gtk_registry() -> &'static GtkRegistry {
    GTK_REGISTRY.get_or_init(GtkRegistry::new)
}

pub fn thunk_gtk_init(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let argc_ptr = ctx.get_x(0) as *mut c_int;
    let argv_ptr = ctx.get_x(1) as *mut *mut *mut c_char;

    tracing::info!("[Maarch64 GTK3 Passthrough] gtk_init()");

    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type InitFn = unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char);
            if let Ok(init) = gtk_lib.get::<InitFn>(b"gtk_init\0") {
                let mut dummy_argc: c_int = 0;
                let mut dummy_argv: *mut *mut c_char = std::ptr::null_mut();
                init(
                    if argc_ptr.is_null() { &mut dummy_argc } else { argc_ptr },
                    if argv_ptr.is_null() { &mut dummy_argv } else { argv_ptr },
                );
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_init_check(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let argc_ptr = ctx.get_x(0) as *mut c_int;
    let argv_ptr = ctx.get_x(1) as *mut *mut *mut c_char;

    tracing::info!("[Maarch64 GTK3 Passthrough] gtk_init_check()");

    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type InitCheckFn = unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int;
            if let Ok(init_check) = gtk_lib.get::<InitCheckFn>(b"gtk_init_check\0") {
                let mut dummy_argc: c_int = 0;
                let mut dummy_argv: *mut *mut c_char = std::ptr::null_mut();
                let res = init_check(
                    if argc_ptr.is_null() { &mut dummy_argc } else { argc_ptr },
                    if argv_ptr.is_null() { &mut dummy_argv } else { argv_ptr },
                );
                ctx.set_x(0, res as i64 as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 1);
    Ok(())
}

pub fn thunk_gtk_window_new(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let window_type = ctx.get_x(0) as c_int;
    tracing::info!("[Maarch64 GTK3 Passthrough] gtk_window_new(type={})", window_type);

    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type WindowNewFn = unsafe extern "C" fn(c_int) -> *mut c_void;
            if let Ok(window_new) = gtk_lib.get::<WindowNewFn>(b"gtk_window_new\0") {
                let window_ptr = window_new(window_type);
                ctx.set_x(0, window_ptr as u64);
                return Ok(());
            }
        }
    }

    ctx.set_x(0, 0x6666_0000);
    Ok(())
}

pub fn thunk_gtk_window_set_title(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let window_ptr = ctx.get_x(0) as *mut c_void;
    let title_ptr = ctx.get_x(1);

    let title_str = if title_ptr != 0 {
        mem.read_string(title_ptr)
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_else(|_| "Maarch64 GTK Window".to_string())
    } else {
        "Maarch64 GTK Window".to_string()
    };

    tracing::info!("[Maarch64 GTK3 Passthrough] gtk_window_set_title(window={:?}, title={:?})", window_ptr, title_str);

    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type SetTitleFn = unsafe extern "C" fn(*mut c_void, *const c_char);
            if let Ok(set_title) = gtk_lib.get::<SetTitleFn>(b"gtk_window_set_title\0") {
                let c_title = std::ffi::CString::new(title_str).unwrap();
                set_title(window_ptr, c_title.as_ptr());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_window_set_default_size(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let window_ptr = ctx.get_x(0) as *mut c_void;
    let width = ctx.get_x(1) as c_int;
    let height = ctx.get_x(2) as c_int;

    tracing::info!("[Maarch64 GTK3 Passthrough] gtk_window_set_default_size(window={:?}, w={}, h={})", window_ptr, width, height);

    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type SetSizeFn = unsafe extern "C" fn(*mut c_void, c_int, c_int);
            if let Ok(set_size) = gtk_lib.get::<SetSizeFn>(b"gtk_window_set_default_size\0") {
                set_size(window_ptr, width, height);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_widget_show(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let widget_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type WidgetShowFn = unsafe extern "C" fn(*mut c_void);
            if let Ok(widget_show) = gtk_lib.get::<WidgetShowFn>(b"gtk_widget_show\0") {
                widget_show(widget_ptr);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_widget_show_all(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let widget_ptr = ctx.get_x(0) as *mut c_void;
    tracing::info!("[Maarch64 GTK3 Passthrough] gtk_widget_show_all(widget={:?})", widget_ptr);

    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type WidgetShowAllFn = unsafe extern "C" fn(*mut c_void);
            if let Ok(widget_show_all) = gtk_lib.get::<WidgetShowAllFn>(b"gtk_widget_show_all\0") {
                widget_show_all(widget_ptr);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_widget_destroy(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let widget_ptr = ctx.get_x(0) as *mut c_void;
    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type WidgetDestroyFn = unsafe extern "C" fn(*mut c_void);
            if let Ok(widget_destroy) = gtk_lib.get::<WidgetDestroyFn>(b"gtk_widget_destroy\0") {
                widget_destroy(widget_ptr);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_main(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[Maarch64 GTK3 Passthrough] gtk_main()");

    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type MainFn = unsafe extern "C" fn();
            if let Ok(main_fn) = gtk_lib.get::<MainFn>(b"gtk_main\0") {
                main_fn();
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_main_quit(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[Maarch64 GTK3 Passthrough] gtk_main_quit()");

    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type MainQuitFn = unsafe extern "C" fn();
            if let Ok(main_quit) = gtk_lib.get::<MainQuitFn>(b"gtk_main_quit\0") {
                main_quit();
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_gtk_button_new_with_label(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let label_ptr = ctx.get_x(0);
    let label_str = if label_ptr != 0 {
        mem.read_string(label_ptr)
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_else(|_| "Button".to_string())
    } else {
        "Button".to_string()
    };

    tracing::info!("[Maarch64 GTK3 Passthrough] gtk_button_new_with_label(label={:?})", label_str);

    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type ButtonNewFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
            if let Ok(button_new) = gtk_lib.get::<ButtonNewFn>(b"gtk_button_new_with_label\0") {
                let c_label = std::ffi::CString::new(label_str).unwrap();
                let btn_ptr = button_new(c_label.as_ptr());
                ctx.set_x(0, btn_ptr as u64);
                return Ok(());
            }
        }
    }

    ctx.set_x(0, 0x6666_0008);
    Ok(())
}

pub fn thunk_gtk_label_new(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let text_ptr = ctx.get_x(0);
    let text_str = if text_ptr != 0 {
        mem.read_string(text_ptr)
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_else(|_| "".to_string())
    } else {
        "".to_string()
    };

    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type LabelNewFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
            if let Ok(label_new) = gtk_lib.get::<LabelNewFn>(b"gtk_label_new\0") {
                let c_text = std::ffi::CString::new(text_str).unwrap();
                let lbl_ptr = label_new(c_text.as_ptr());
                ctx.set_x(0, lbl_ptr as u64);
                return Ok(());
            }
        }
    }

    ctx.set_x(0, 0x6666_0010);
    Ok(())
}

pub fn thunk_gtk_container_add(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let container_ptr = ctx.get_x(0) as *mut c_void;
    let widget_ptr = ctx.get_x(1) as *mut c_void;

    if let Some(gtk_lib) = get_gtk_registry().get_library() {
        unsafe {
            type ContainerAddFn = unsafe extern "C" fn(*mut c_void, *mut c_void);
            if let Ok(container_add) = gtk_lib.get::<ContainerAddFn>(b"gtk_container_add\0") {
                container_add(container_ptr, widget_ptr);
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn register_gtk_thunks(map: &mut HashMap<String, super::ThunkFn>) {
    map.insert("gtk_init".to_string(), thunk_gtk_init);
    map.insert("gtk_init_check".to_string(), thunk_gtk_init_check);
    map.insert("gtk_window_new".to_string(), thunk_gtk_window_new);
    map.insert("gtk_window_set_title".to_string(), thunk_gtk_window_set_title);
    map.insert("gtk_window_set_default_size".to_string(), thunk_gtk_window_set_default_size);
    map.insert("gtk_widget_show".to_string(), thunk_gtk_widget_show);
    map.insert("gtk_widget_show_all".to_string(), thunk_gtk_widget_show_all);
    map.insert("gtk_widget_destroy".to_string(), thunk_gtk_widget_destroy);
    map.insert("gtk_main".to_string(), thunk_gtk_main);
    map.insert("gtk_main_quit".to_string(), thunk_gtk_main_quit);
    map.insert("gtk_button_new_with_label".to_string(), thunk_gtk_button_new_with_label);
    map.insert("gtk_label_new".to_string(), thunk_gtk_label_new);
    map.insert("gtk_container_add".to_string(), thunk_gtk_container_add);
}
