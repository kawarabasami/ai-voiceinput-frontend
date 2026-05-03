use serde::Serialize;
use tauri::State;
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
