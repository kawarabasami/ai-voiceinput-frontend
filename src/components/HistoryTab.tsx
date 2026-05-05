import React from "react";
import { invoke } from "@tauri-apps/api/core";
import { HistoryItem } from "../types/HistoryItem";
import { AppConfig } from "../types/AppConfig";

interface HistoryTabProps {
  history: HistoryItem[];
  config: AppConfig;
  onCorrectionUpdate: (id: string, correctedText: string) => void;
  onStatusChange: (text: string, color: string) => void;
}

const HistoryTab: React.FC<HistoryTabProps> = ({
  history,
  config,
  onCorrectionUpdate,
  onStatusChange,
}) => {
  const handleCopy = async (item: HistoryItem) => {
    const text = item.correctedText || item.originalText;
    try {
      await invoke("copy_to_clipboard", { text });
    } catch (e) {
      console.error("[HistoryTab] コピー失敗:", e);
    }
  };

  const handleCorrect = async (item: HistoryItem) => {
    onStatusChange("校正中...", "#4a9eff");
    try {
      const correctedText = await invoke<string>("correct_text", {
        apiBaseUrl: config.apiBaseUrl,
        model: config.defaultLlmModel,
        text: item.originalText,
        prompt: config.correctionPrompt,
      });
      onCorrectionUpdate(item.id, correctedText);
      onStatusChange("校正完了", "#4caf50");
    } catch (e) {
      console.error("[HistoryTab] 校正失敗:", e);
      onStatusChange(`校正エラー: ${e}`, "#f44336");
    }
    setTimeout(() => onStatusChange("待機中 (Ctrl+Win で録音開始)", "#888"), 3000);
  };

  if (history.length === 0) {
    return (
      <div className="history-tab empty-state">
        <div className="empty-icon">🎤</div>
        <p>Ctrl+Win キーを押して録音を開始してください</p>
      </div>
    );
  }

  return (
    <div className="history-tab">
      {history.map((item) => (
        <div key={item.id} className="history-item">
          <div className="history-item-meta">
            <span className="history-timestamp">
              {new Date(item.timestamp).toLocaleTimeString("ja-JP")}
            </span>
          </div>
          <div className="history-text-block">
            <div className="history-label">認識テキスト</div>
            <div className="history-original">{item.originalText}</div>
            {item.correctedText && (
              <>
                <div className="history-label corrected-label">校正後</div>
                <div className="history-corrected">{item.correctedText}</div>
              </>
            )}
          </div>
          <div className="history-actions">
            {item.recordingPath && (
              <button
                className="btn btn-secondary btn-sm"
                onClick={async () => {
                  try {
                    onStatusChange("音声を読み込み中...", "#4a9eff");
                    const bytes = await invoke<number[]>("get_recording_audio", {
                      path: item.recordingPath!,
                    });
                    const blob = new Blob([new Uint8Array(bytes)], { type: "audio/wav" });
                    const url = URL.createObjectURL(blob);
                    const audio = new Audio(url);
                    await audio.play();
                    onStatusChange("待機中 (Ctrl+Win で録音開始)", "#888");
                  } catch (e) {
                    console.error("再生失敗:", e);
                    onStatusChange(
                      "音声ファイルが見つかりません（削除された可能性があります）",
                      "#f44336"
                    );
                    setTimeout(
                      () => onStatusChange("待機中 (Ctrl+Win で録音開始)", "#888"),
                      3000
                    );
                  }
                }}
                title="録音を再生する"
              >
                ▶ 再生
              </button>
            )}
            <button
              className="btn btn-secondary btn-sm"
              onClick={() => handleCopy(item)}
              title="クリップボードにコピー"
            >
              📋 再コピー
            </button>
            <button
              className="btn btn-primary btn-sm"
              onClick={() => handleCorrect(item)}
              title="LLMで校正する"
            >
              ✨ 校正する
            </button>
          </div>
        </div>
      ))}
    </div>
  );
};

export default HistoryTab;
