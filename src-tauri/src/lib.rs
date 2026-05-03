mod audio_recorder;
mod commands;
mod shortcut;
mod tray;

use audio_recorder::AudioRecorder;
use commands::{ai_client, audio, config, input};
use std::sync::{Arc, Mutex};
use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let recorder_state = Arc::new(Mutex::new(AudioRecorder::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(recorder_state)
        .invoke_handler(tauri::generate_handler![
            // audio
            audio::get_microphone_devices,
            audio::start_recording,
            audio::stop_recording,
            // ai_client
            ai_client::transcribe_audio,
            ai_client::correct_text,
            ai_client::chat_completion,
            // input
            input::input_text,
            input::copy_to_clipboard,
            // config
            config::load_config,
            config::save_config,
            config::show_main_window,
            config::hide_main_window,
        ])
        .setup(|app| {
            // システムトレイ設定
            tray::setup_tray(app.handle())?;

            // グローバルショートカット設定（Ctrl+Win）
            shortcut::setup_global_shortcut(app.handle());

            // オーバーレイウィンドウの位置を画面下部中央に設定
            if let Some(overlay) = app.get_webview_window("overlay") {
                if let Ok(monitor) = overlay.primary_monitor() {
                    if let Some(monitor) = monitor {
                        let screen_size = monitor.size();
                        let win_size = overlay.outer_size().unwrap_or(tauri::PhysicalSize {
                            width: 300,
                            height: 60,
                        });
                        let x = (screen_size.width as i32 - win_size.width as i32) / 2;
                        let y = screen_size.height as i32 - win_size.height as i32 - 60; // タスクバー上
                        let _ = overlay.set_position(tauri::PhysicalPosition { x, y });
                    }
                }
                // クリックスルー設定
                let _ = overlay.set_ignore_cursor_events(true);
                // ウィンドウを表示（中身が空なら透明で見えない）
                let _ = overlay.show();
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("Tauriアプリケーションの構築に失敗しました")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                shortcut::teardown_global_shortcut();
            }
        });
}
