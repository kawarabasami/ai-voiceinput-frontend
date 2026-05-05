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
        .manage(recorder_state.clone())
        .invoke_handler(tauri::generate_handler![
            // audio
            audio::get_microphone_devices,
            audio::start_recording,
            audio::stop_recording,
            audio::get_recording_audio,
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
        .setup(move |app| {
            // システムトレイ設定
            tray::setup_tray(app.handle())?;

            if let Some(state) = app.handle().try_state::<Arc<Mutex<AudioRecorder>>>() {
                if let Ok(mut s) = state.inner().lock() {
                    s.app_handle = Some(app.handle().clone());
                }
            }


            // 設定の読み込み
            let config = config::load_config(app.handle().clone()).unwrap_or_default();

            // メインウィンドウの表示制御 (start_minimized が false の場合のみ表示)
            if !config.start_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
            }

            // グローバルショートカット設定（Ctrl+Win）
            shortcut::setup_global_shortcut(app.handle());

            // オーディオストリームのプリウォーム（起動時にデバイスを開いておく）
            let recorder_state_prewarm = recorder_state.clone();
            let mic_device_num = config.microphone_device_number;
            tauri::async_runtime::spawn(async move {
                if let Err(e) = audio_recorder::ensure_stream(recorder_state_prewarm, mic_device_num) {
                    eprintln!("[Setup] オーディオプリウォーム失敗: {}", e);
                } else {
                    println!("[Setup] オーディオプリウォーム完了");
                }
            });

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
                        let y = screen_size.height as i32 - win_size.height as i32 - 100; // タスクバーより上に配置
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
