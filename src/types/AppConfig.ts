export interface AppConfig {
  apiBaseUrl: string;
  whisperModel: string;
  llmModels: string;
  defaultLlmModel: string;
  correctionPrompt: string;
  postRecordingDelayMs: number;
  isAutoCorrectionEnabled: boolean;
  microphoneDeviceNumber: number;
  theme: "dark" | "light";
  startMinimized: boolean;
}

export const DEFAULT_CONFIG: AppConfig = {
  apiBaseUrl: "http://127.0.0.1:13305/v1",
  whisperModel: "whisper-v3-turbo-FLM",
  llmModels: "qwen3.5-2b-FLM,qwen3.5-4b-FLM,gemma4-it-e2b-FLM",
  defaultLlmModel: "qwen3.5-2b-FLM",
  correctionPrompt:
    "以下の音声認識されたテキストの誤字脱字を修正し、自然な日本語にしてください。修正後のテキストのみを出力してください。",
  postRecordingDelayMs: 500,
  isAutoCorrectionEnabled: false,
  microphoneDeviceNumber: 0,
  theme: "dark",
  startMinimized: false,
};
