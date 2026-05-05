use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use std::sync::{Arc, Mutex};

use crate::audio_recorder::{self, AudioRecorder};

#[derive(Serialize)]
pub struct MicDevice {
    pub index: usize,
    pub name: String,
}

#[tauri::command]
pub fn get_microphone_devices() -> Result<Vec<MicDevice>, String> {
    let devices = audio_recorder::get_input_devices()?;
    Ok(devices
        .into_iter()
        .map(|(index, name)| MicDevice { index, name })
        .collect())
}

#[tauri::command]
pub fn start_recording(
    device_number: usize,
    recorder: State<Arc<Mutex<AudioRecorder>>>,
) -> Result<(), String> {
    audio_recorder::start_recording(recorder.inner().clone(), device_number)
}

#[tauri::command]
pub fn stop_recording(recorder: State<Arc<Mutex<AudioRecorder>>>) -> Result<String, String> {
    audio_recorder::stop_recording(recorder.inner().clone())
}

#[tauri::command]
pub fn get_recording_audio(app_handle: AppHandle, path: String) -> Result<Vec<u8>, String> {
    let app_data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let recordings_dir = app_data_dir.join("recordings");
    let recordings_dir = std::fs::canonicalize(&recordings_dir)
        .map_err(|e| format!("録音ディレクトリ取得失敗: {}", e))?;

    let requested_path = std::path::PathBuf::from(path);
    let resolved_path = if requested_path.is_absolute() {
        requested_path
    } else {
        recordings_dir.join(requested_path)
    };
    let resolved_path = std::fs::canonicalize(&resolved_path)
        .map_err(|e| format!("ファイル読み込み失敗: {}", e))?;

    if !resolved_path.starts_with(&recordings_dir) {
        return Err("不正なアクセスです".to_string());
    }

    std::fs::read(resolved_path).map_err(|e| format!("ファイル読み込み失敗: {}", e))
}
