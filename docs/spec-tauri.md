# VoiceInputApp (Tauri版) 要件定義および設計書

> **対象バージョン**: Tauri 2.x + React + TypeScript  
> **対応OS**: Windows 10/11（主要ターゲット）、将来的にmacOS/Linux対応可能  
> **元となる実装**: WPF版 VoiceInputApp の機能を全て移植する

---

## 1. 概要

ローカルAIサーバー（Lemonade等、OpenAI互換APIサーバー）と連携し、OS上のあらゆる場所で音声入力を可能にする軽量なWindows向け常駐アプリのTauri版。  
WPF版の全機能を踏襲しつつ、クロスプラットフォーム対応・モダンなWebフロントエンド・Tauriのプラグインエコシステムを活用する。

---

## 2. アプリの要件

### 2.1 基本機能

- **グローバルショートカット**: `Ctrl + Win` キーを同時に押している間だけマイク録音を行い、離すと録音終了および音声認識を開始する。
  - 録音: Rustの`cpal`クレートまたはTauriプラグイン (`tauri-plugin-microphone` 等) を使用。
  - キー監視: `tauri-plugin-global-shortcut` または Rust側 `global-hotkey` クレートでグローバルキーイベントを監視。
  - **ホールド操作**: キーを押している間だけ録音し、離したら自動的に録音停止・文字起こし開始する「Push-to-Talk」方式。
  
- **自動文字入力**: 音声認識されたテキストを現在アクティブなウィンドウに入力する。
  - `tauri-plugin-clipboard-manager` または Rust側の `arboard` クレートでクリップボード操作。
  - `enigo` クレートで `Ctrl+V` キーストロークをシミュレートし、文字化けを防ぎつつ入力。
  - クリップボード入力後、元のクリップボード内容を復元する。
  - Windowsクリップボード履歴に追加しないよう `CF_CLIPBOARD_EXCLUDE` 相当の処理を行う（Windows APIの `ExcludeClipboardContentFromMonitorProcessing` を `windows-rs` 経由で設定）。

### 2.2 AI連携 (OpenAI互換 API)

- **音声認識 (Whisper)**
  - エンドポイント: 設定済みのAPIベースURLの `/audio/transcriptions`
  - デフォルトベースURL: `http://127.0.0.1:13305/v1`
  - 送信データ: 録音した音声ファイル（WAV、16kHz/モノラル）をMultipart/form-dataで送信
  - 処理: 音声をテキスト化する。
  - Rust側で `reqwest` クレートを使用してHTTPリクエストを行う。

- **文章校正 (LLM)**
  - エンドポイント: 設定済みのAPIベースURLの `/chat/completions`
  - 処理: 履歴から選択したテキストに対して文章校正を行う。
  - デフォルトプロンプト: 「以下の音声認識されたテキストの誤字脱字を修正し、自然な日本語にしてください。修正後のテキストのみを出力してください」
  - 自動校正モード（設定で有効化）と手動校正モード（履歴の「校正する」ボタン）をサポート。

- **チャット (LLM)**
  - エンドポイント: 設定済みのAPIベースURLの `/chat/completions`
  - 処理: ユーザーとAIの会話履歴を保持しながらLLMと対話する。
  - チャット履歴はアプリ内メモリで管理（セッション終了で消去）。

### 2.3 UI画面構成

アプリは **メインウィンドウ** と **オーバーレイウィンドウ** の2ウィンドウ構成とする。

#### 2.3.1 メインウィンドウ（タブ構成）

**共通エリア（タブ外）**:
- ステータス表示: 現在の状態（待機中・録音中・文字起こし中・入力完了・エラー等）をテキストと色で視覚的に表示。

**タブ1: 音声入力・履歴**
- 過去の文字起こし結果と校正結果を一覧表示（タイムスタンプ・元テキスト・校正後テキスト）。
- 各履歴アイテムのアクション:
  - 「再コピー」ボタン: クリップボードにコピー
  - 「校正する」ボタン: LLMで校正を実行し結果を履歴に反映

**タブ2: チャット**
- LLMとのテキストチャット機能。
- モデル選択ComboBox（設定済みモデルリストから選択）。
- メッセージ入力エリア（`Ctrl+Enter` で送信）。
- 送信中プログレスインジケーター。
- 「チャットをクリア」ボタン。
- ユーザーとAIのメッセージを色分けして表示。

**タブ3: 設定**
- API Base URL
- 使用するマイクデバイス（利用可能なデバイス一覧から選択）
- Whisperモデル名
- LLMモデル一覧（カンマ区切りで複数登録）
- デフォルトLLM（音声校正用）
- 校正プロンプト（テキストエリアで編集可能）
- 録音終了後の待機時間（ミリ秒）
- 自動校正の有効/無効（チェックボックス）
- 「設定を保存」ボタン

#### 2.3.2 オーバーレイウィンドウ

- 録音中・文字起こし中・入力完了などの状態を画面下部中央にポップアップ表示する小型ウィンドウ。
- 常時前面表示（Always on top）、クリックスルー（マウスイベントを下のウィンドウに透過）。
- 画面下部中央に固定配置（タスクバーの上）。
- Tauri の `WebviewWindowBuilder` で独立したウィンドウとして実装し、フロントエンドからイベント経由で制御。

#### 2.3.3 タスクトレイ常駐

- アプリはタスクトレイに常駐し、バックグラウンドで動作する。
- **アイコン**: `tauri-plugin-tray` でシステムトレイアイコンを設定。
- **メインウィンドウのクローズ動作**: ×ボタンで閉じてもアプリは終了せず、トレイに常駐する。
- **右クリックメニュー**:
  - 「設定画面を表示」: メインウィンドウを表示・フォーカス
  - 「終了」: アプリケーションを完全終了

---

## 3. アーキテクチャ設計

### 3.1 技術スタック

| 層 | 技術 | 説明 |
|---|---|---|
| フロントエンド | React + TypeScript + Vite | UIコンポーネント |
| スタイリング | CSS Modules / Vanilla CSS | コンポーネント単位のスコープ付きCSS |
| バックグラウンド | Rust (Tauri 2.x) | システム操作・ネイティブAPI |
| プロセス間通信 | Tauri Commands / Events | Rust ↔ JavaScript の双方向通信 |
| 設定永続化 | `tauri-plugin-store` | JSONファイル (`config.json`) |

### 3.2 使用するRustクレート

| クレート | 用途 |
|---|---|
| `tauri` | フレームワーク本体 |
| `tauri-plugin-global-shortcut` | グローバルショートカット登録 |
| `tauri-plugin-tray` | システムトレイアイコン・メニュー |
| `tauri-plugin-store` | 設定の永続化 |
| `tauri-plugin-clipboard-manager` | クリップボード操作 |
| `cpal` | マイク音声録音 |
| `hound` | WAVファイルの書き込み |
| `reqwest` | HTTP通信（Whisper/LLM API呼び出し） |
| `enigo` | キーストロークシミュレート（Ctrl+V） |
| `serde` / `serde_json` | JSON シリアライズ |
| `tokio` | 非同期ランタイム |
| `windows` または `windows-rs` | Windows固有API（クリップボード履歴除外等） |

### 3.3 プロジェクト構成

```
VoiceInputApp-Tauri/
├── src/                          # フロントエンド（React/TS）
│   ├── components/
│   │   ├── StatusBar.tsx         # ステータス表示（タブ外共通）
│   │   ├── HistoryTab.tsx        # 音声入力・履歴タブ
│   │   ├── ChatTab.tsx           # チャットタブ
│   │   ├── SettingsTab.tsx       # 設定タブ
│   │   └── OverlayStatus.tsx     # オーバーレイウィンドウのUI
│   ├── hooks/
│   │   ├── useConfig.ts          # 設定の読み書き
│   │   ├── useHistory.ts         # 履歴管理
│   │   └── useChatMessages.ts    # チャットメッセージ管理
│   ├── types/
│   │   ├── AppConfig.ts          # 設定データ型
│   │   ├── HistoryItem.ts        # 履歴アイテム型
│   │   └── ChatMessage.ts        # チャットメッセージ型
│   ├── App.tsx                   # メインアプリコンポーネント（タブ制御）
│   ├── overlay.tsx               # オーバーレイウィンドウのエントリポイント
│   └── main.tsx                  # メインウィンドウのエントリポイント
│
└── src-tauri/                    # バックエンド（Rust）
    ├── src/
    │   ├── main.rs               # エントリポイント・Tauriアプリ初期化
    │   ├── commands/
    │   │   ├── mod.rs
    │   │   ├── audio.rs          # 録音開始・停止コマンド
    │   │   ├── ai_client.rs      # Whisper/LLM API呼び出しコマンド
    │   │   ├── input.rs          # クリップボード/キー入力コマンド
    │   │   └── config.rs         # 設定読み書きコマンド
    │   ├── shortcut.rs           # グローバルショートカット管理
    │   ├── tray.rs               # システムトレイ管理
    │   └── audio_recorder.rs     # マイク録音ロジック（cpal）
    ├── Cargo.toml
    └── tauri.conf.json
```

### 3.4 Tauriコマンド定義

フロントエンドからバックエンドを呼び出すコマンド（`#[tauri::command]`）:

| コマンド名 | 説明 | 引数 | 戻り値 |
|---|---|---|---|
| `start_recording` | マイク録音開始 | `device_number: i32` | `Result<(), String>` |
| `stop_recording` | 録音停止・WAVファイルパス返却 | なし | `Result<String, String>` |
| `transcribe_audio` | Whisper API呼び出し | `api_base_url, model, file_path` | `Result<String, String>` |
| `correct_text` | LLM校正API呼び出し | `api_base_url, model, text, prompt` | `Result<String, String>` |
| `chat_completion` | LLMチャットAPI呼び出し | `api_base_url, model, messages` | `Result<String, String>` |
| `input_text` | テキストをアクティブウィンドウに入力 | `text: String` | `Result<(), String>` |
| `copy_to_clipboard` | クリップボードにコピー | `text: String` | `Result<(), String>` |
| `get_microphone_devices` | 利用可能なマイクデバイス一覧取得 | なし | `Result<Vec<MicDevice>, String>` |
| `load_config` | 設定ファイル読み込み | なし | `Result<AppConfig, String>` |
| `save_config` | 設定ファイル保存 | `config: AppConfig` | `Result<(), String>` |
| `show_main_window` | メインウィンドウを表示・フォーカス | なし | `()` |
| `hide_main_window` | メインウィンドウを非表示 | なし | `()` |

### 3.5 Tauriイベント定義

バックエンドからフロントエンドへのイベント（`app_handle.emit()`）:

| イベント名 | 発火タイミング | ペイロード |
|---|---|---|
| `shortcut-down` | Ctrl+Winが押された | なし |
| `shortcut-up` | Ctrl+Winが離された | なし |
| `status-changed` | ステータス変化時 | `{ text: string, color: string }` |
| `recording-started` | 録音開始 | なし |
| `recording-stopped` | 録音停止 | `{ file_path: string }` |
| `transcription-completed` | 文字起こし完了 | `{ text: string }` |
| `correction-completed` | 校正完了 | `{ original: string, corrected: string }` |
| `overlay-show` | オーバーレイ表示 | `{ text: string, color: string }` |
| `overlay-hide` | オーバーレイ非表示 | なし |

### 3.6 データモデル

#### AppConfig (TypeScript/Rust共通)

```typescript
interface AppConfig {
  apiBaseUrl: string;               // デフォルト: "http://127.0.0.1:13305/v1"
  whisperModel: string;             // デフォルト: "whisper-v3-turbo-FLM"
  llmModels: string;                // カンマ区切り、デフォルト: "qwen2.5-7b-instruct"
  defaultLlmModel: string;          // デフォルト: "qwen2.5-7b-instruct"
  correctionPrompt: string;         // 校正プロンプト
  postRecordingDelayMs: number;     // デフォルト: 500
  isAutoCorrectionEnabled: boolean; // デフォルト: false
  microphoneDeviceNumber: number;   // デフォルト: 0
}
```

#### HistoryItem (TypeScript)

```typescript
interface HistoryItem {
  id: string;            // UUID
  timestamp: string;     // ISO8601
  originalText: string;
  correctedText: string; // 空文字の場合は未校正
}
```

#### ChatMessage (TypeScript)

```typescript
interface ChatMessage {
  role: "User" | "AI";
  content: string;
  timestamp: string;
}
```

### 3.7 設定の永続化

- `tauri-plugin-store` を使用し、アプリデータディレクトリ（`AppData/Roaming/VoiceInputApp/config.json`）に保存。
- フォーマットはJSON。

### 3.8 音声録音仕様

- フォーマット: WAV、16kHz、モノラル（Whisper最適）
- 録音中はインメモリバッファ（`Vec<f32>`）に蓄積し、録音停止時にWAVファイルとして書き出す。
- 一時ファイルの保存先: OSの一時ディレクトリ（`std::env::temp_dir()`）の `voice_input_temp.wav`

### 3.9 グローバルショートカットの制約と代替案

> **注意**: Tauri の `tauri-plugin-global-shortcut` は `Ctrl+Win` のようなWindowsキーとの組み合わせを
> サポートしない場合がある。その場合は Rust側で `global-hotkey` クレートを直接使用するか、
> Windows API（`SetWindowsHookEx` 相当）を `windows-rs` 経由で実装する。

---

## 4. WPF版との主な相違点

| 項目 | WPF版 | Tauri版 |
|---|---|---|
| フロントエンド | XAML/WPF | React + TypeScript |
| バックエンド | .NET 8 / C# | Rust |
| キーボードフック | `SetWindowsHookEx` (P/Invoke) | `tauri-plugin-global-shortcut` or `global-hotkey` |
| マイク録音 | `NAudio` | `cpal` + `hound` |
| クリップボード | `System.Windows.Clipboard` | `arboard` / `tauri-plugin-clipboard-manager` |
| キーストロークシミュレート | `System.Windows.Forms.SendKeys` | `enigo` |
| タスクトレイ | `System.Windows.Forms.NotifyIcon` | `tauri-plugin-tray` |
| 設定保存 | JSON (`config.json`) in exe dir | `tauri-plugin-store` (AppData) |
| HTTP通信 | `System.Net.Http.HttpClient` | `reqwest` |
| クロスプラットフォーム | Windows専用 | Windows/macOS/Linux対応 |

---

## 5. 将来の拡張（Nice to Have）

- **macOS / Linux対応**: `cpal` と `enigo` はクロスプラットフォームだが、クリップボード履歴除外等はOS固有処理が必要。
- **ショートカットキーのカスタマイズ**: 設定UIでショートカットを変更可能にする。
- **音声ファイルのボリューム検知**: 無音時間が一定以上続いたら自動停止する機能。
- **複数プロファイル**: APIサーバーやモデルの設定をプロファイルとして保存・切り替え。
- **ストリーミングレスポンス**: チャット機能でLLMのストリーミング出力に対応（`EventSource` / Server-Sent Events）。
