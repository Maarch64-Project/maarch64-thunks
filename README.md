# 🔌 maarch64-thunks

`maarch64-thunks` provides the Zero-Cost Library Passthrough (Thunking) subsystem for Maarch64. Instead of emulating CPU instructions inside guest shared libraries, Maarch64 intercepts library symbol calls and bridges them directly to native host x86_64 drivers and system frameworks.

---

## 🌟 Supported Subsystems & Bridges

```
thunks/src/
├── lib.rs         # ThunkManager registry & dynamic symbol resolution
├── gpu.rs         # GPU / Display: EGL, GLX, OpenGL ES (60 FPS hardware rendering)
├── audio.rs       # Audio: ALSA (libasound) & PulseAudio (libpulse)
├── vlc.rs         # Multimedia: LibVLC (libvlc.so.5 / libvlccore) passthrough
├── darwin.rs      # macOS: Objective-C Runtime (objc_msgSend), CoreFoundation, GCD
├── metal.rs       # macOS: Apple Metal Framework (MTLCreateSystemDefaultDevice)
├── android.rs     # Android: NDK APIs (ANativeWindow, AAssetManager, Bionic bridges)
└── gtk.rs         # GUI: GTK+ 3.0 windowing and event management
```

---

## 🏗️ Thunking Mechanism

```
Target ARM64 App                   Maarch64 ThunkManager               Host x86_64 System
+--------------------+            +-----------------------+            +---------------------+
| Calls glDrawArrays | ---------> | Lookup thunk table    | ---------> | Call native libGL.so|
| or objc_msgSend    |            | Translate pointers/ABI|            | Directly on GPU     |
+--------------------+            +-----------------------+            +---------------------+
                                             |
                                             v
                                  (Return result to X0)
```

1. When dynamic libraries are loaded, `ElfLoader` or `MachOLoader` registers dynamic symbol trampolines into `ThunkManager`.
2. When the CPU execution hits a registered trampoline address, `ThunkManager` executes the host Rust bridge function.
3. The bridge function reads arguments directly from `CpuContext` (`X0`..`X7`), invokes the host dynamic library, and places the return value back into `X0` with zero CPU emulation overhead.

---

## 📋 Key Symbol Implementations

| Category | Symbols Implemented |
| :--- | :--- |
| **GPU / OpenGL ES** | `eglGetDisplay`, `eglInitialize`, `eglCreateWindowSurface`, `eglMakeCurrent`, `eglSwapBuffers`, `glViewport`, `glClear`, `glDrawArrays`, `glCompileShader`, `glLinkProgram` |
| **Audio** | `snd_pcm_open`, `snd_pcm_hw_params`, `snd_pcm_writei`, `pa_simple_new`, `pa_simple_write`, `pa_simple_drain` |
| **Media Player** | `libvlc_new`, `libvlc_media_new_path`, `libvlc_media_player_new_from_media`, `libvlc_media_player_play`, `libvlc_media_player_stop` |
| **macOS / Darwin** | `objc_msgSend`, `objc_msgSend_stret`, `objc_msgSendSuper2`, `objc_getClass`, `sel_registerName`, `objc_autoreleasePoolPush`, `dispatch_async`, `CFRelease` |
| **macOS Metal** | `MTLCreateSystemDefaultDevice` |
| **Android NDK** | `ANativeWindow_fromSurface`, `ANativeWindow_getWidth`, `ANativeWindow_getHeight`, `AAssetManager_open` |

---

## 🧪 Testing

```bash
# Run thunk unit tests
cargo test -p maarch64-thunks
```
