/// グローバルショートカット管理（Windows専用: SetWindowsHookEx）
use tauri::AppHandle;

#[cfg(target_os = "windows")]
mod windows_hook {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use tauri::{AppHandle, Emitter};
    use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LWIN, VK_RCONTROL, VK_RWIN,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    static SHORTCUT_ACTIVE: AtomicBool = AtomicBool::new(false);

    // HHOOK や生ポインタを Mutex で扱えるようにするためのラッパー
    struct SendWrapper<T>(T);
    unsafe impl<T> Send for SendWrapper<T> {}
    unsafe impl<T> Sync for SendWrapper<T> {}

    static HOOK_HANDLE: Mutex<Option<SendWrapper<HHOOK>>> = Mutex::new(None);
    static APP_HANDLE_PTR: Mutex<Option<SendWrapper<*const AppHandle>>> = Mutex::new(None);

    unsafe extern "system" fn keyboard_proc(
        n_code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if n_code >= 0 {
            let kb = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
            let vk = VIRTUAL_KEY(kb.vkCode as u16);

            let msg = w_param.0 as u32;
            let is_keydown = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let is_keyup = msg == WM_KEYUP || msg == WM_SYSKEYUP;

            if is_keydown || is_keyup {
                // GetAsyncKeyState で現在の状態を取得
                let mut ctrl_pressed = is_key_pressed(VK_CONTROL)
                    || is_key_pressed(VK_LCONTROL)
                    || is_key_pressed(VK_RCONTROL);

                let mut win_pressed = is_key_pressed(VK_LWIN) || is_key_pressed(VK_RWIN);

                // 現在のイベント対象が Ctrl/Win の場合は状態を上書き（GetAsyncKeyStateとの同期ズレ対策）
                if is_keydown {
                    if is_ctrl_key(vk) { ctrl_pressed = true; }
                    if is_win_key(vk) { win_pressed = true; }
                } else if is_keyup {
                    if is_ctrl_key(vk) { ctrl_pressed = false; }
                    if is_win_key(vk) { win_pressed = false; }
                }

                if ctrl_pressed && win_pressed {
                    if !SHORTCUT_ACTIVE.load(Ordering::SeqCst) {
                        SHORTCUT_ACTIVE.store(true, Ordering::SeqCst);
                        println!("[Shortcut] Shortcut Down (Ctrl+Win)");
                        if let Ok(guard) = APP_HANDLE_PTR.lock() {
                            if let Some(wrapper) = &*guard {
                                let handle = &*wrapper.0;
                                let _ = handle.emit("shortcut-down", ());
                            }
                        }
                    }
                } else {
                    if SHORTCUT_ACTIVE.load(Ordering::SeqCst) {
                        SHORTCUT_ACTIVE.store(false, Ordering::SeqCst);
                        println!("[Shortcut] Shortcut Up");
                        if let Ok(guard) = APP_HANDLE_PTR.lock() {
                            if let Some(wrapper) = &*guard {
                                let handle = &*wrapper.0;
                                let _ = handle.emit("shortcut-up", ());
                            }
                        }
                    }
                }
            }
        }

        let h_hook = if let Ok(guard) = HOOK_HANDLE.lock() {
            guard.as_ref().map(|w| w.0)
        } else {
            None
        };

        CallNextHookEx(
            h_hook,
            n_code,
            w_param,
            l_param,
        )
    }

    fn is_ctrl_key(vk: VIRTUAL_KEY) -> bool {
        vk == VK_CONTROL || vk == VK_LCONTROL || vk == VK_RCONTROL
    }

    fn is_win_key(vk: VIRTUAL_KEY) -> bool {
        vk == VK_LWIN || vk == VK_RWIN
    }

    unsafe fn is_key_pressed(vk: VIRTUAL_KEY) -> bool {
        (GetAsyncKeyState(vk.0 as i32) & 0x8000u16 as i16) != 0
    }

    pub fn install_hook(app: &AppHandle) {
        unsafe {
            if let Ok(mut guard) = APP_HANDLE_PTR.lock() {
                *guard = Some(SendWrapper(app as *const AppHandle));
            }
            
            use windows::Win32::System::LibraryLoader::GetModuleHandleW;
            let hmod = GetModuleHandleW(None).unwrap_or_default();
            let hinstance = HINSTANCE(hmod.0);
            
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), Some(hinstance), 0)
                .expect("キーボードフックの設定に失敗しました");
            
            if let Ok(mut guard) = HOOK_HANDLE.lock() {
                *guard = Some(SendWrapper(hook));
            }
            println!("[Shortcut] Keyboard hook installed (hmod: {:?})", hmod);
        }
    }

    pub fn uninstall_hook() {
        unsafe {
            let mut h_hook = None;
            if let Ok(mut guard) = HOOK_HANDLE.lock() {
                h_hook = guard.take().map(|w| w.0);
            }
            
            if let Some(hook) = h_hook {
                let _ = UnhookWindowsHookEx(hook);
                println!("[Shortcut] Keyboard hook uninstalled");
            }
            
            if let Ok(mut guard) = APP_HANDLE_PTR.lock() {
                *guard = None;
            }
        }
    }
}

pub fn setup_global_shortcut(app: &AppHandle) {
    #[cfg(target_os = "windows")]
    {
        windows_hook::install_hook(app);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
    }
}

pub fn teardown_global_shortcut() {
    #[cfg(target_os = "windows")]
    {
        windows_hook::uninstall_hook();
    }
}
