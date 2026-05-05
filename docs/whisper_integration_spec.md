# Whisper連携およびVAD(音声区間検出)実装仕様案

本ドキュメントは、Tauri + Rustバックエンド環境において、ローカルWhisperサーバーへの音声送信機能およびVAD（音声区間検出）による自動停止機能、フォーマット最適化、設定機能の追加に関する実装案をまとめたものです。

---

## 1. Rust側バックエンド実装案

### 1.1 VAD (音声区間検出) と自動停止ロジック
- **`webrtc-vad` の利用**: 
  `webrtc-vad` クレートを使用して、マイクから取得した音声データの無音判定を行います。
- **フレーム分割**: 
  `webrtc-vad` は 10ms, 20ms, 30ms のフレーム（16kHzの場合、それぞれ160, 320, 480サンプル）のみを受け付けます。`cpal`のコールバックで取得するデータ長は一定ではないため、**リングバッファ**等を用いてデータを蓄積し、固定長(例: 30ms)ごとにVAD判定にかけます。
- **自動停止の仕組み**:
  VADが「無音」と判定したフレーム数をカウントします。30msフレームの場合、1.5秒の無音は50フレーム連続になります。有音が検出されたらカウンターをリセットし、カウンターが50に達した時点で、`mpsc`チャンネルなどを通じて録音停止シグナルを送信します。

### 1.2 音声フォーマットの最適化 (16000Hz, Mono)
- **ストリーム設定の固定化**:
  可能であれば `cpal` の `StreamConfig` で サンプリングレート `16000Hz`、チャンネル数 `1` (Mono) を指定してデバイスをオープンします。
- **ソフトウェア・リサンプリング**:
  マイクデバイスが `16000Hz / Mono` をサポートしていない場合（例: 48kHz / Stereo固定など）、取得したデータをRust側で変換する必要があります。
  - **Mono化**: 左右のチャンネルの平均を取って1chにダウンミックスします。
  - **リサンプリング**: `rubato` クレート等を用いるか、単純な間引き処理（48kHz→16kHzなら3サンプルに1つ取る）を行います。
- **WAV保存**:
  `hound` クレートを用いて、変換後の16-bit PCMデータをWAVファイル（またはメモリ上のバッファ）として生成します。

### 1.3 WhisperサーバーへのHTTPリクエスト
- **`reqwest` を使用した Multipart 送信**:
  作成したWAVデータ（ファイルパスまたはメモリバッファ）を `multipart/form-data` としてローカルWhisperサーバーへPOSTリクエストします。
- **オプションの適用**:
  フロントエンドから受け取った設定（言語、プロンプト）に応じて、Multipartのリクエストボディに `language` と `prompt` パラメータを動的に追加します。

### 1.4 設定状態の管理とコマンド
- **状態管理**:
  `tauri::State` 内に `std::sync::RwLock` または `Mutex` でラップした設定構造体を保持し、リクエスト時に参照できるようにします。
- **設定受け取りコマンド**:
  ```rust
  #[derive(Clone, serde::Deserialize)]
  pub struct WhisperConfig {
      pub language_fixed: bool,
      pub initial_prompt_enabled: bool,
      pub initial_prompt_text: String,
  }

  #[tauri::command]
  pub fn update_whisper_config(config: WhisperConfig, state: tauri::State<ConfigState>) {
      // 状態を更新
  }
  ```

---

## 2. Tauriフロントエンド側実装案

### 2.1 設定UIの構築 (React/Vue等)
- 設定タブやモーダル（例: `SettingsTab.tsx`）内に以下のUIを配置します。
  - **言語固定設定**: `language: "ja"` を固定するかどうかの Toggle Switch (または Checkbox)。
  - **初期プロンプト設定**:
    - 有効/無効の Toggle Switch。
    - 有効な場合のみ入力可能になる Textarea。
- **状態の永続化**:
  ローカルの `localStorage` または `tauri-plugin-store` を用いて、アプリ再起動後も設定を保持するようにします。

### 2.2 状態管理とRustへの通知
- フロントエンド側で設定が変更されるたびに、または設定画面を閉じる際に `invoke` を使用してRust側に設定を同期します。
  ```typescript
  import { invoke } from '@tauri-apps/api/core'; // Tauri v2 の場合

  async function saveConfig(config) {
    // 状態の保存とRust側への通知
    await invoke('update_whisper_config', { config });
  }
  ```
- 起動時にも保存されている設定を読み込み、Rustへ初期設定として送信するフローを `useEffect` などに実装します。

---

## 3. 依存関係 (`Cargo.toml` に追加するクレート)

```toml
[dependencies]
# VAD（音声区間検出）
webrtc-vad = "0.4.0"
# 必要に応じて、バッファリング用にringbuf等
ringbuf = "0.3"

# WAV生成
hound = "3.5"

# HTTPリクエスト (非同期処理・JSON・Multipart対応)
reqwest = { version = "0.12", features = ["json", "multipart"] }
tokio = { version = "1", features = ["full"] }

# データのシリアライズ
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# (オプション) リサンプリングが必要な場合
# rubato = "0.14"
```

---

## 4. 実装における注意点やアドバイス

1. **Cコンパイラの準備**
   `webrtc-vad` は内部でC言語のコードに依存しているため、Windows環境でビルドするには MSVC (Visual Studio の C++ ビルドツール) などの C コンパイラがインストールされている必要があります。

2. **VADの入力制約に注意**
   VADに渡すデータは必ず **16kHz, 16-bit Mono, 10/20/30ms単位** である必要があります。`cpal` でそのまま取得した可変長のデータチャンクを直接渡すとエラーになります。必ずリングバッファ等でバッファリングし、固定サイズのフレームに切り出してからVADに渡す設計にしてください。

3. **ブロッキング処理の回避**
   `cpal` のオーディオコールバック関数内はリアルタイム処理が求められるため、ファイルI/Oや重い処理（HTTPリクエストなど）を直接書くと音飛びの原因になります。コールバック内ではVAD判定とバッファリングのみを行い、録音完了後のWAVエンコードやHTTP送信は `tokio::spawn` などの別スレッド（非同期タスク）にオフロードするようにしてください。

4. **Whisper側のパラメータ仕様の確認**
   OpenAI API互換サーバー（LM Studio等）によっては、`prompt` や `language` パラメータの解釈が完全には一致しない場合があります。実際に送信する際は、対象となるローカルサーバーのAPIドキュメント（または実際の動作）を確認しつつ、`multipart` の各パートの名前が仕様通りになっているかテストすることをお勧めします。
