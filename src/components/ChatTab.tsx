import React, { useState, useRef, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChatMessage } from "../types/ChatMessage";
import { AppConfig } from "../types/AppConfig";

interface ChatTabProps {
  messages: ChatMessage[];
  config: AppConfig;
  onAddMessage: (role: ChatMessage["role"], content: string) => void;
  onClearMessages: () => void;
  onStatusChange: (text: string, color: string) => void;
}

const ChatTab: React.FC<ChatTabProps> = ({
  messages,
  config,
  onAddMessage,
  onClearMessages,
  onStatusChange,
}) => {
  const [input, setInput] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [selectedModel, setSelectedModel] = useState(config.defaultLlmModel);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const models = useMemo(() => {
    return config.llmModels
      .split(",")
      .map((m) => m.trim())
      .filter(Boolean);
  }, [config.llmModels]);

  useEffect(() => {
    // 現在の選択がリストにない場合、あるいは未設定の場合、デフォルトまたは先頭のモデルに切り替える
    if (models.length > 0) {
      if (!selectedModel || !models.includes(selectedModel)) {
        setSelectedModel(config.defaultLlmModel || models[0]);
      }
    }
  }, [config.defaultLlmModel, models, selectedModel]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const sendMessage = async () => {
    const text = input.trim();
    const modelToUse = selectedModel || config.defaultLlmModel || models[0];

    if (!text || isLoading) return;
    if (!modelToUse) {
      onStatusChange("エラー: モデルが設定されていません", "#f44336");
      return;
    }

    setInput("");
    onAddMessage("user", text);
    setIsLoading(true);
    onStatusChange("回答待ち...", "#4a9eff");

    // 送信するメッセージ配列を構築（システムプロンプトは含めない）
    const apiMessages = [
      ...messages.map((m) => ({ role: m.role, content: m.content })),
      { role: "user", content: text },
    ];

    try {
      const response = await invoke<string>("chat_completion", {
        apiBaseUrl: config.apiBaseUrl,
        model: modelToUse,
        messages: apiMessages,
      });
      onAddMessage("assistant", response);
      onStatusChange("待機中 (Ctrl+Win で録音開始)", "#888");
    } catch (e) {
      console.error("[ChatTab] チャット送信失敗:", e);
      onStatusChange(`チャットエラー: ${e}`, "#f44336");
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && e.ctrlKey) {
      e.preventDefault();
      sendMessage();
    }
  };

  return (
    <div className="chat-tab">
      <div className="chat-toolbar">
        <label className="chat-model-label">モデル:</label>
        <select
          id="chat-model-select"
          className="select"
          value={selectedModel}
          onChange={(e) => setSelectedModel(e.target.value)}
        >
          {models.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
        <button className="btn btn-secondary btn-sm" onClick={onClearMessages}>
          🗑 クリア
        </button>
      </div>

      <div className="chat-messages">
        {messages.length === 0 && (
          <div className="empty-state">
            <div className="empty-icon">💬</div>
            <p>AIとチャットを開始してください</p>
          </div>
        )}
        {messages.map((msg, idx) => (
          <div key={idx} className={`chat-bubble ${msg.role}`}>
            <div className="chat-role">{msg.role === "user" ? "あなた" : "AI"}</div>
            <div className="chat-content">{msg.content}</div>
            <div className="chat-time">
              {new Date(msg.timestamp).toLocaleTimeString("ja-JP")}
            </div>
          </div>
        ))}
        {isLoading && (
          <div className="chat-bubble assistant">
            <div className="chat-role">AI</div>
            <div className="chat-content typing-indicator">
              <span></span>
              <span></span>
              <span></span>
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      <div className="chat-input-area">
        <textarea
          id="chat-input"
          className="chat-textarea"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="メッセージを入力 (Ctrl+Enter で送信)"
          rows={3}
          disabled={isLoading}
        />
        <button
          id="chat-send-btn"
          className="btn btn-primary"
          onClick={sendMessage}
          disabled={isLoading || !input.trim()}
        >
          送信
        </button>
      </div>
    </div>
  );
};

export default ChatTab;
