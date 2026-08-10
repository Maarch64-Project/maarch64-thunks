#![allow(non_snake_case)]

use maarch64_core::{cpu::CpuContext, memory::MemoryManager};
use std::collections::HashMap;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::OnceLock;

pub struct QtRegistry {
    loaded_libraries: HashMap<String, libloading::Library>,
}

impl QtRegistry {
    pub fn new() -> Self {
        let mut loaded_libraries = HashMap::new();
        let libs_to_try = [
            ("libQt5Widgets.so.5", "libQt5Widgets.so"),
            ("libQt6Widgets.so.6", "libQt6Widgets.so"),
            ("libQt5Core.so.5", "libQt5Core.so"),
        ];

        for (name, alt_name) in libs_to_try {
            if let Ok(lib) = unsafe { libloading::Library::new(name) } {
                tracing::info!("[Maarch64 Qt Passthrough] Successfully loaded host Qt library: {}", name);
                loaded_libraries.insert(name.to_string(), lib);
            } else if let Ok(lib) = unsafe { libloading::Library::new(alt_name) } {
                tracing::info!("[Maarch64 Qt Passthrough] Successfully loaded host Qt library: {}", alt_name);
                loaded_libraries.insert(name.to_string(), lib);
            }
        }

        Self { loaded_libraries }
    }

    pub fn get_library(&self, name: &str) -> Option<&libloading::Library> {
        self.loaded_libraries.get(name)
            .or_else(|| self.loaded_libraries.values().next())
    }
}

static QT_REGISTRY: OnceLock<QtRegistry> = OnceLock::new();
pub fn get_qt_registry() -> &'static QtRegistry {
    QT_REGISTRY.get_or_init(QtRegistry::new)
}

pub fn thunk_QApplication_create(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let argc = ctx.get_x(0) as c_int;
    let argv_ptr = ctx.get_x(1) as *mut *mut c_char;

    tracing::info!("[Maarch64 Qt Passthrough] QApplication_create(argc={})", argc);

    if let Some(qt_lib) = get_qt_registry().get_library("libQt5Widgets.so.5") {
        unsafe {
            // Mangled _ZN12QApplicationC1ERiPPci
            type AppCtorFn = unsafe extern "C" fn(*mut c_void, *mut c_int, *mut *mut c_char, c_int);
            if let Ok(ctor) = qt_lib.get::<AppCtorFn>(b"_ZN12QApplicationC1ERiPPci\0") {
                if std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok() {
                    let app_buf = Box::into_raw(Box::new([0u8; 512])) as *mut c_void;
                    let mut dummy_argc: c_int = argc;
                    let mut dummy_argv: *mut c_char = std::ptr::null_mut();
                    ctor(
                        app_buf,
                        &mut dummy_argc,
                        if argv_ptr.is_null() { &mut dummy_argv } else { argv_ptr },
                        0x050f00, // Qt Version 5.15
                    );
                    ctx.set_x(0, app_buf as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0x7777_0000);
    Ok(())
}

pub fn thunk_QApplication_exec(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[Maarch64 Qt Passthrough] QApplication_exec()");

    if let Some(qt_lib) = get_qt_registry().get_library("libQt5Widgets.so.5") {
        unsafe {
            // Mangled _ZN12QApplication4execEv
            type ExecFn = unsafe extern "C" fn() -> c_int;
            if let Ok(exec) = qt_lib.get::<ExecFn>(b"_ZN12QApplication4execEv\0") {
                let res = exec();
                ctx.set_x(0, res as i64 as u64);
                return Ok(());
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_QApplication_quit(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    tracing::info!("[Maarch64 Qt Passthrough] QApplication_quit()");

    if let Some(qt_lib) = get_qt_registry().get_library("libQt5Widgets.so.5") {
        unsafe {
            // Mangled _ZN7QApplication4quitEv
            type QuitFn = unsafe extern "C" fn();
            if let Ok(quit) = qt_lib.get::<QuitFn>(b"_ZN7QApplication4quitEv\0") {
                quit();
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_QWidget_create(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let parent_ptr = ctx.get_x(0) as *mut c_void;
    let flags = ctx.get_x(1) as u32;

    tracing::info!("[Maarch64 Qt Passthrough] QWidget_create(parent={:?}, flags={:#x})", parent_ptr, flags);

    if let Some(qt_lib) = get_qt_registry().get_library("libQt5Widgets.so.5") {
        unsafe {
            // Mangled _ZN7QWidgetC1EPS_6QFlagsIN2Qt10WindowTypeEE
            type WidgetCtorFn = unsafe extern "C" fn(*mut c_void, *mut c_void, u32);
            if let Ok(ctor) = qt_lib.get::<WidgetCtorFn>(b"_ZN7QWidgetC1EPS_6QFlagsIN2Qt10WindowTypeEE\0") {
                if std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok() {
                    let widget_buf = Box::into_raw(Box::new([0u8; 512])) as *mut c_void;
                    ctor(widget_buf, parent_ptr, flags);
                    ctx.set_x(0, widget_buf as u64);
                    return Ok(());
                }
            }
        }
    }

    ctx.set_x(0, 0x7777_0008);
    Ok(())
}

pub fn thunk_QWidget_show(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let widget_ptr = ctx.get_x(0) as *mut c_void;
    tracing::info!("[Maarch64 Qt Passthrough] QWidget_show(widget={:?})", widget_ptr);

    if !widget_ptr.is_null() && (widget_ptr as u64) > 0x1_0000_0000 {
        if let Some(qt_lib) = get_qt_registry().get_library("libQt5Widgets.so.5") {
            unsafe {
                // Mangled _ZN7QWidget4showEv
                type ShowFn = unsafe extern "C" fn(*mut c_void);
                if let Ok(show) = qt_lib.get::<ShowFn>(b"_ZN7QWidget4showEv\0") {
                    show(widget_ptr);
                }
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_QWidget_hide(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let widget_ptr = ctx.get_x(0) as *mut c_void;
    if !widget_ptr.is_null() && (widget_ptr as u64) > 0x1_0000_0000 {
        if let Some(qt_lib) = get_qt_registry().get_library("libQt5Widgets.so.5") {
            unsafe {
                // Mangled _ZN7QWidget4hideEv
                type HideFn = unsafe extern "C" fn(*mut c_void);
                if let Ok(hide) = qt_lib.get::<HideFn>(b"_ZN7QWidget4hideEv\0") {
                    hide(widget_ptr);
                }
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_QWidget_setWindowTitle(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let widget_ptr = ctx.get_x(0) as *mut c_void;
    let title_ptr = ctx.get_x(1);

    let title_str = if title_ptr != 0 {
        mem.read_string(title_ptr)
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_else(|_| "Maarch64 Qt Window".to_string())
    } else {
        "Maarch64 Qt Window".to_string()
    };

    tracing::info!("[Maarch64 Qt Passthrough] QWidget_setWindowTitle(widget={:?}, title={:?})", widget_ptr, title_str);

    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_QWidget_resize(ctx: &mut CpuContext, _mem: &mut MemoryManager) -> Result<(), String> {
    let widget_ptr = ctx.get_x(0) as *mut c_void;
    let w = ctx.get_x(1) as c_int;
    let h = ctx.get_x(2) as c_int;

    tracing::info!("[Maarch64 Qt Passthrough] QWidget_resize(widget={:?}, w={}, h={})", widget_ptr, w, h);

    if !widget_ptr.is_null() && (widget_ptr as u64) > 0x1_0000_0000 {
        if let Some(qt_lib) = get_qt_registry().get_library("libQt5Widgets.so.5") {
            unsafe {
                // Mangled _ZN7QWidget6resizeEii
                type ResizeFn = unsafe extern "C" fn(*mut c_void, c_int, c_int);
                if let Ok(resize) = qt_lib.get::<ResizeFn>(b"_ZN7QWidget6resizeEii\0") {
                    resize(widget_ptr, w, h);
                }
            }
        }
    }
    ctx.set_x(0, 0);
    Ok(())
}

pub fn thunk_QPushButton_create(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let text_ptr = ctx.get_x(0);
    let parent_ptr = ctx.get_x(1) as *mut c_void;

    let text_str = if text_ptr != 0 {
        mem.read_string(text_ptr)
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_else(|_| "Button".to_string())
    } else {
        "Button".to_string()
    };

    tracing::info!("[Maarch64 Qt Passthrough] QPushButton_create(text={:?}, parent={:?})", text_str, parent_ptr);

    ctx.set_x(0, 0x7777_0010);
    Ok(())
}

pub fn thunk_QLabel_create(ctx: &mut CpuContext, mem: &mut MemoryManager) -> Result<(), String> {
    let text_ptr = ctx.get_x(0);
    let parent_ptr = ctx.get_x(1) as *mut c_void;

    let text_str = if text_ptr != 0 {
        mem.read_string(text_ptr)
            .map(|b| String::from_utf8_lossy(&b).to_string())
            .unwrap_or_else(|_| "".to_string())
    } else {
        "".to_string()
    };

    tracing::info!("[Maarch64 Qt Passthrough] QLabel_create(text={:?}, parent={:?})", text_str, parent_ptr);

    ctx.set_x(0, 0x7777_0018);
    Ok(())
}

pub fn register_qt_thunks(map: &mut HashMap<String, super::ThunkFn>) {
    map.insert("QApplication_create".to_string(), thunk_QApplication_create);
    map.insert("QApplication_exec".to_string(), thunk_QApplication_exec);
    map.insert("QApplication_quit".to_string(), thunk_QApplication_quit);
    map.insert("QWidget_create".to_string(), thunk_QWidget_create);
    map.insert("QWidget_show".to_string(), thunk_QWidget_show);
    map.insert("QWidget_hide".to_string(), thunk_QWidget_hide);
    map.insert("QWidget_setWindowTitle".to_string(), thunk_QWidget_setWindowTitle);
    map.insert("QWidget_resize".to_string(), thunk_QWidget_resize);
    map.insert("QPushButton_create".to_string(), thunk_QPushButton_create);
    map.insert("QLabel_create".to_string(), thunk_QLabel_create);

    // C++ Mangled Symbol Registrations
    map.insert("_ZN12QApplicationC1ERiPPci".to_string(), thunk_QApplication_create);
    map.insert("_ZN12QApplication4execEv".to_string(), thunk_QApplication_exec);
    map.insert("_ZN7QApplication4quitEv".to_string(), thunk_QApplication_quit);
    map.insert("_ZN7QWidget4showEv".to_string(), thunk_QWidget_show);
    map.insert("_ZN7QWidget4hideEv".to_string(), thunk_QWidget_hide);
    map.insert("_ZN7QWidget6resizeEii".to_string(), thunk_QWidget_resize);
}
