use reqwest::multipart;
use serde::{Deserialize, Serialize};
use std::path::Path;

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

#[tauri::command]
pub async fn transcribe_audio(
    api_base_url: String,
    model: String,
    file_path: String,
    language_fixed: bool,
    initial_prompt_enabled: bool,
    initial_prompt: String,
) -> Result<String, String> {
    let path = Path::new(&file_path);
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
