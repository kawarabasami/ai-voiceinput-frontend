import React, { useEffect, useRef, useState } from "react";
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
  const currentAudioRef = useRef<HTMLAudioElement | null>(null);
  const currentAudioUrlRef = useRef<string | null>(null);
  const activeAudioIdRef = useRef<string | null>(null);
  const [playingAudioId, setPlayingAudioId] = useState<string | null>(null);

  const cleanupCurrentAudio = () => {
    currentAudioRef.current?.pause();
    currentAudioRef.current = null;
    if (currentAudioUrlRef.current) {
      URL.revokeObjectURL(currentAudioUrlRef.current);
      currentAudioUrlRef.current = null;
    }
    activeAudioIdRef.current = null;
    setPlayingAudioId(null);
  };

  useEffect(() => {
    return () => {
      currentAudioRef.current?.pause();
      currentAudioRef.current = null;
      if (currentAudioUrlRef.current) {
        URL.revokeObjectURL(currentAudioUrlRef.current);
        currentAudioUrlRef.current = null;
      }
      activeAudioIdRef.current = null;
    };
  }, []);

  const handleCopy = async (item: HistoryItem) => {
    const text = item.correctedText || item.originalText;
    try {
      await invoke("copy_to_clipboard", { text });
    } catch (e) {
      console.error("[HistoryTab] コピー失敗:", e);
    }
  };

  const handlePlayAudio = async (item: HistoryItem) => {
    if (!item.audioPath) return;

    try {
      if (activeAudioIdRef.current === item.id) {
        cleanupCurrentAudio();
        return;
      }

      cleanupCurrentAudio();
      activeAudioIdRef.current = item.id;
      setPlayingAudioId(item.id);

      const audioBytes = await invoke<number[]>("get_recording_audio", {
        path: item.audioPath,
      });

      if (activeAudioIdRef.current !== item.id) {
        return;
      }

      const audioUrl = URL.createObjectURL(
        new Blob([new Uint8Array(audioBytes)], { type: "audio/wav" })
      );
      const audio = new Audio(audioUrl);
      currentAudioRef.current = audio;
      currentAudioUrlRef.current = audioUrl;
      audio.addEventListener(
        "ended",
        () => {
          cleanupCurrentAudio();
        },
        { once: true }
      );
      audio.addEventListener("error", cleanupCurrentAudio, { once: true });
      await audio.play();
    } catch (e) {
      cleanupCurrentAudio();
      console.error("[HistoryTab] 音声再生失敗:", e);
      onStatusChange(`音声再生エラー: ${e}`, "#f44336");
      setTimeout(() => onStatusChange("待機中 (Ctrl+Win で録音開始)", "#888"), 3000);
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
            {item.audioPath && (
              <button
                className={`btn btn-secondary btn-sm audio-play-btn ${
                  playingAudioId === item.id ? "is-playing" : ""
                }`}
                onClick={() => handlePlayAudio(item)}
                title="文字起こしに使った音声を再生"
                aria-label={
                  playingAudioId === item.id
                    ? "音声の再生を停止"
                    : "文字起こしに使った音声を再生"
                }
              >
                <span className="play-icon" aria-hidden="true" />
                <span>{playingAudioId === item.id ? "再生中" : "再生"}</span>
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
