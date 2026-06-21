import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { emit } from "@tauri-apps/api/event";
import { useConfig } from "./hooks/useConfig";
import { useHistory } from "./hooks/useHistory";
import { useChatMessages } from "./hooks/useChatMessages";
import StatusBar from "./components/StatusBar";
import HistoryTab from "./components/HistoryTab";
import ChatTab from "./components/ChatTab";
import SettingsTab from "./components/SettingsTab";
import "./App.css";

type Tab = "history" | "chat" | "settings";

function App() {
  const [activeTab, setActiveTab] = useState<Tab>("history");
  const [statusText, setStatusText] = useState("待機中 (Ctrl+Win で録音開始)");
  const [statusColor, setStatusColor] = useState("#888");

  const { config, saveConfig, loading } = useConfig();
  const { history, addItem, updateCorrectedText } = useHistory();
  const { messages, addMessage, clearMessages } = useChatMessages();

  // 最新の設定を保持するための Ref
  const configRef = useRef(config);
  useEffect(() => {
    configRef.current = config;
    // テーマの反映
    document.documentElement.setAttribute("data-theme", config.theme);
    // オーバーレイにも通知
    emit("theme-changed", config.theme).catch(() => {});
  }, [config]);

  // 処理中の重複を防ぐ ref
  const isProcessingRef = useRef(false);

  const setStatus = (text: string, color: string) => {
    setStatusText(text);
    setStatusColor(color);
    
    // 「待機中」を含む場合はオーバーレイを非表示にする
    if (text.includes("待機中")) {
      emit("overlay-hide").catch(() => {});
    } else {
      // それ以外はオーバーレイを表示
      emit("overlay-show", { text, color }).catch(() => {});
    }
  };

  const resetStatus = () => {
    setStatusText("待機中 (Ctrl+Win で録音開始)");
    setStatusColor("#888");
    emit("overlay-hide").catch(() => {});
  };

  useEffect(() => {
    // shortcut-down: 録音開始
    const unlistenDown = listen("shortcut-down", async () => {
      console.log("[App] shortcut-down received");
      if (isProcessingRef.current) return;
      
      const currentConfig = configRef.current;
      try {
        await invoke("start_recording", {
          deviceNumber: currentConfig.microphoneDeviceNumber,
        });
        setStatus("録音中...", "#f44336");
      } catch (e) {
        console.error("[App] 録音開始失敗:", e);
        setStatus(`録音開始失敗: ${e}`, "#f44336");
        setTimeout(resetStatus, 3000);
      }
    });

    // shortcut-up: 録音停止 → 文字起こし → (自動校正) → 入力
    const unlistenUp = listen("shortcut-up", async () => {
      const currentConfig = configRef.current;
      console.log("[App] shortcut-up received, auto-correct enabled:", currentConfig.isAutoCorrectionEnabled);
      
      if (isProcessingRef.current) return;
      isProcessingRef.current = true;

      try {
        // 録音終了後の待機
        if (currentConfig.postRecordingDelayMs > 0) {
          await new Promise((resolve) =>
            setTimeout(resolve, currentConfig.postRecordingDelayMs)
          );
        }

        let wavPath: string;
        try {
          wavPath = await invoke<string>("stop_recording");
        } catch (e) {
          console.error("[App] 録音停止失敗:", e);
          resetStatus();
          return;
        }

        setStatus("文字起こし中...", "#4a9eff");

        let transcribedText: string;
        try {
          transcribedText = await invoke<string>("transcribe_audio", {
            apiBaseUrl: currentConfig.apiBaseUrl,
            model: currentConfig.whisperModel,
            filePath: wavPath,
          });
        } catch (e) {
          console.error("[App] 文字起こし失敗:", e);
          setStatus(`エラー: ${e}`, "#f44336");
          setTimeout(resetStatus, 3000);
          return;
        }

        if (!transcribedText?.trim()) {
          setStatus("音声が認識されませんでした", "#ff9800");
          setTimeout(resetStatus, 3000);
          return;
        }

        console.log("[App] transcribed:", transcribedText);

        // 履歴に追加
        const historyItem = addItem(transcribedText, wavPath);
        let textToInput = transcribedText;

        // 自動校正
        if (currentConfig.isAutoCorrectionEnabled) {
          console.log("[App] starting auto-correction...");
          setStatus("校正中...", "#4a9eff");
          try {
            const corrected = await invoke<string>("correct_text", {
              apiBaseUrl: currentConfig.apiBaseUrl,
              model: currentConfig.defaultLlmModel,
              text: transcribedText,
              prompt: currentConfig.correctionPrompt,
            });
            console.log("[App] corrected:", corrected);
            if (corrected?.trim()) {
              textToInput = corrected;
              updateCorrectedText(historyItem.id, corrected);
            }
          } catch (e) {
            console.error("[App] 自動校正失敗:", e);
          }
        }

        // アクティブウィンドウに入力
        setStatus("入力中...", "#4a9eff");
        await invoke("input_text", { text: textToInput });

        setStatus("入力完了", "#4caf50");
        setTimeout(resetStatus, 3000);
      } finally {
        isProcessingRef.current = false;
      }
    });

    return () => {
      unlistenDown.then((fn) => fn());
      unlistenUp.then((fn) => fn());
    };
  }, []); // 空の依存配列で一度だけ登録

  if (loading) {
    return (
      <div className="app-loading">
        <div className="loading-spinner" />
      </div>
    );
  }

  return (
    <div className="app">
      <div className="app-header">
        <h1 className="app-title">🎤 VoiceInputApp</h1>
        <StatusBar text={statusText} color={statusColor} />
      </div>

      <nav className="tab-bar">
        <button
          id="tab-history"
          className={`tab-btn ${activeTab === "history" ? "active" : ""}`}
          onClick={() => setActiveTab("history")}
        >
          📝 履歴
        </button>
        <button
          id="tab-chat"
          className={`tab-btn ${activeTab === "chat" ? "active" : ""}`}
          onClick={() => setActiveTab("chat")}
        >
          💬 チャット
        </button>
        <button
          id="tab-settings"
          className={`tab-btn ${activeTab === "settings" ? "active" : ""}`}
          onClick={() => setActiveTab("settings")}
        >
          ⚙️ 設定
        </button>
      </nav>

      <main className="tab-content">
        {activeTab === "history" && (
          <HistoryTab
            history={history}
            config={config}
            onCorrectionUpdate={updateCorrectedText}
            onStatusChange={setStatus}
          />
        )}
        {activeTab === "chat" && (
          <ChatTab
            messages={messages}
            config={config}
            onAddMessage={addMessage}
            onClearMessages={clearMessages}
            onStatusChange={setStatus}
          />
        )}
        {activeTab === "settings" && (
          <SettingsTab config={config} onSave={saveConfig} />
        )}
      </main>
    </div>
  );
}

export default App;
