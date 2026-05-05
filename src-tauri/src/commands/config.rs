use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

/// アプリ設定データモデル（フロントエンドと共通）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default = "default_api_base_url")]
    pub api_base_url: String,
    #[serde(default = "default_whisper_model")]
    pub whisper_model: String,
    #[serde(default = "default_llm_models")]
    pub llm_models: String,
    #[serde(default = "default_llm_model")]
    pub default_llm_model: String,
    #[serde(default = "default_correction_prompt")]
    pub correction_prompt: String,
    #[serde(default = "default_post_recording_delay_ms")]
    pub post_recording_delay_ms: u64,
    #[serde(default)]
    pub is_auto_correction_enabled: bool,
    #[serde(default)]
    pub microphone_device_number: usize,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default)]
    pub whisper_language_fixed: bool,
    #[serde(default)]
    pub whisper_initial_prompt_enabled: bool,
    #[serde(default)]
    pub whisper_initial_prompt: String,
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_api_base_url() -> String {
    "http://127.0.0.1:13305/v1".to_string()
}
fn default_whisper_model() -> String {
    "whisper-v3-turbo-FLM".to_string()
}
fn default_llm_models() -> String {
    "qwen2.5-7b-instruct".to_string()
}
fn default_llm_model() -> String {
    "qwen2.5-7b-instruct".to_string()
}
fn default_correction_prompt() -> String {
    "以下の音声認識されたテキストの誤字脱字を修正し、自然な日本語にしてください。修正後のテキストのみを出力してください。".to_string()
}
fn default_post_recording_delay_ms() -> u64 {
    500
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_base_url: default_api_base_url(),
            whisper_model: default_whisper_model(),
            llm_models: default_llm_models(),
            default_llm_model: default_llm_model(),
            correction_prompt: default_correction_prompt(),
            post_recording_delay_ms: default_post_recording_delay_ms(),
            is_auto_correction_enabled: false,
            microphone_device_number: 0,
            theme: default_theme(),
            start_minimized: false,
            whisper_language_fixed: false,
            whisper_initial_prompt_enabled: false,
            whisper_initial_prompt: "".to_string(),
        }
    }
}

const CONFIG_KEY: &str = "config";

#[tauri::command]
pub fn load_config<R: Runtime>(app: AppHandle<R>) -> Result<AppConfig, String> {
    use tauri_plugin_store::StoreExt;
    let store = app
        .store("config.json")
        .map_err(|e| format!("ストア開錠失敗: {}", e))?;

    let config = store
        .get(CONFIG_KEY)
        .and_then(|v| serde_json::from_value::<AppConfig>(v).ok())
        .unwrap_or_default();

    Ok(config)
}

#[tauri::command]
pub fn save_config<R: Runtime>(app: AppHandle<R>, config: AppConfig) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    let store = app
        .store("config.json")
        .map_err(|e| format!("ストア開錠失敗: {}", e))?;

    let value =
        serde_json::to_value(&config).map_err(|e| format!("シリアライズ失敗: {}", e))?;
    store.set(CONFIG_KEY, value);
    store.save().map_err(|e| format!("ストア保存失敗: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn show_main_window<R: Runtime>(app: AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.unminimize();
    }
}

#[tauri::command]
pub fn hide_main_window<R: Runtime>(app: AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}
