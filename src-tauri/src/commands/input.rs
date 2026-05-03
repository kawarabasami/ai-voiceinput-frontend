use arboard::Clipboard;
use enigo::{Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

#[tauri::command]
pub async fn input_text(text: String) -> Result<(), String> {
    let original_clipboard = {
        let mut clipboard =
            Clipboard::new().map_err(|e| format!("クリップボード初期化失敗: {}", e))?;
        clipboard.get_text().ok()
    };

    {
        let mut clipboard =
            Clipboard::new().map_err(|e| format!("クリップボード初期化失敗: {}", e))?;

        #[cfg(target_os = "windows")]
        {
            set_clipboard_exclude(&mut clipboard, &text)?;
        }

        #[cfg(not(target_os = "windows"))]
        clipboard
            .set_text(&text)
            .map_err(|e| format!("クリップボード設定失敗: {}", e))?;
    }

    thread::sleep(Duration::from_millis(50));

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|e| format!("Enigo初期化失敗: {}", e))?;

    let _ = enigo.key(Key::Control, enigo::Direction::Press);
    let _ = enigo.key(Key::Unicode('v'), enigo::Direction::Click);
    let _ = enigo.key(Key::Control, enigo::Direction::Release);

    thread::sleep(Duration::from_millis(100));

    if let Some(original) = original_clipboard {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(original);
        }
    }

    Ok(())
}

#[tauri::command]
pub fn copy_to_clipboard(text: String) -> Result<(), String> {
    let mut clipboard =
        Clipboard::new().map_err(|e| format!("クリップボード初期化失敗: {}", e))?;
    clipboard
        .set_text(text)
        .map_err(|e| format!("クリップボード設定失敗: {}", e))
}

#[cfg(target_os = "windows")]
fn set_clipboard_exclude(clipboard: &mut Clipboard, text: &str) -> Result<(), String> {
    use windows::Win32::System::DataExchange::{
        CloseClipboard, OpenClipboard,
    };

    clipboard
        .set_text(text)
        .map_err(|e| format!("クリップボード設定失敗: {}", e))?;

    // ExcludeClipboardContentFromMonitorProcessing は環境によって見つからない場合があるため
    // 現時点では Open/Close だけ行うか、機能を保留する
    unsafe {
        if OpenClipboard(None).is_ok() {
            // ここで将来的に除外処理を追加可能
            let _ = CloseClipboard();
        }
    }

    Ok(())
}
