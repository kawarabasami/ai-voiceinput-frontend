use reqwest::multipart;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: String,
}

fn validate_api_base_url(api_base_url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(api_base_url)
        .map_err(|e| format!("API Base URLが不正です: {}", e))?;
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        _ => Err("API Base URLはhttp/httpsのみ許可されています".to_string()),
    }
}

fn resolve_recording_path(app_handle: &AppHandle, file_path: &str) -> Result<PathBuf, String> {
    let app_data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let recordings_dir = app_data_dir.join("recordings");
    let recordings_dir = std::fs::canonicalize(&recordings_dir)
        .map_err(|e| format!("録音ディレクトリ取得失敗: {}", e))?;

    let requested = PathBuf::from(file_path);
    let resolved = if requested.is_absolute() {
        requested
    } else {
        recordings_dir.join(requested)
    };
    let resolved = std::fs::canonicalize(&resolved)
        .map_err(|e| format!("音声ファイル読み込み失敗: {}", e))?;

    if !resolved.starts_with(&recordings_dir) {
        return Err("録音ディレクトリ外のファイルは読み込めません".to_string());
    }

    Ok(resolved)
}

#[tauri::command]
pub async fn transcribe_audio(
    app_handle: AppHandle,
    api_base_url: String,
    model: String,
    file_path: String,
    language_fixed: bool,
    initial_prompt_enabled: bool,
    initial_prompt: String,
) -> Result<String, String> {
    validate_api_base_url(&api_base_url)?;
    let safe_path = resolve_recording_path(&app_handle, &file_path)?;
    let path = Path::new(&safe_path);
    if !path.exists() {
        return Err(format!("音声ファイルが見つかりません: {}", file_path));
    }

    let file_bytes =
        std::fs::read(path).map_err(|e| format!("音声ファイル読み込み失敗: {}", e))?;
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio.wav")
        .to_string();

    let part = multipart::Part::bytes(file_bytes)
        .file_name(file_name)
        .mime_str("audio/wav")
        .map_err(|e| format!("MIMEタイプ設定失敗: {}", e))?;

    let mut form = multipart::Form::new()
        .part("file", part)
        .text("model", model);

    if language_fixed {
        form = form.text("language", "ja");
    }

    if initial_prompt_enabled && !initial_prompt.is_empty() {
        form = form.text("prompt", initial_prompt);
    }

    let url = format!("{}/audio/transcriptions", api_base_url.trim_end_matches('/'));

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Whisper APIリクエスト失敗: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_text = response.text().await.unwrap_or_else(|_| "不明なエラー".to_string());
        return Err(format!("Whisper APIエラー ({}): {}", status, err_text));
    }

    let result: TranscriptionResponse = response
        .json()
        .await
        .map_err(|e| format!("レスポンスのパース失敗: {}", e))?;

    Ok(result.text.trim().to_string())
}

#[tauri::command]
pub async fn correct_text(
    api_base_url: String,
    model: String,
    text: String,
    prompt: String,
) -> Result<String, String> {
    validate_api_base_url(&api_base_url)?;
    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: prompt,
        },
        ChatMessage {
            role: "user".to_string(),
            content: text,
        },
    ];

    chat_completion_inner(api_base_url, model, messages).await
}

#[tauri::command]
pub async fn chat_completion(
    api_base_url: String,
    model: String,
    messages: Vec<ChatMessage>,
) -> Result<String, String> {
    validate_api_base_url(&api_base_url)?;
    chat_completion_inner(api_base_url, model, messages).await
}

async fn chat_completion_inner(
    api_base_url: String,
    model: String,
    messages: Vec<ChatMessage>,
) -> Result<String, String> {
    let url = format!("{}/chat/completions", api_base_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": model,
        "messages": messages,
    });

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("APIリクエスト失敗: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let err_text = response.text().await.unwrap_or_else(|_| "不明なエラー".to_string());
        return Err(format!("APIエラー ({}): {}", status, err_text));
    }

    let result: ChatCompletionResponse = response
        .json()
        .await
        .map_err(|e| format!("レスポンスのパース失敗: {}", e))?;

    result
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content.trim().to_string())
        .ok_or_else(|| "レスポンスが空でした".to_string())
}
