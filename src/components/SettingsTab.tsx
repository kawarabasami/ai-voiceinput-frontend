import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppConfig } from "../types/AppConfig";

interface MicDevice {
  index: number;
  name: string;
}

interface SettingsTabProps {
  config: AppConfig;
  onSave: (config: AppConfig) => Promise<void>;
}

const SettingsTab: React.FC<SettingsTabProps> = ({ config, onSave }) => {
  const [form, setForm] = useState<AppConfig>(config);
  const [micDevices, setMicDevices] = useState<MicDevice[]>([]);
  const [saveStatus, setSaveStatus] = useState<"" | "saved" | "error">("");

  useEffect(() => {
    setForm(config);
  }, [config]);

  useEffect(() => {
    invoke<MicDevice[]>("get_microphone_devices")
      .then(setMicDevices)
      .catch((e) => console.error("[SettingsTab] マイクデバイス取得失敗:", e));
  }, []);

  // LLMモデル変更時にdefaultLlmModelも更新
  const handleLlmModelsChange = (value: string) => {
    const models = value.split(",").map(m => m.trim()).filter(m => m !== "");
    const firstModel = models[0] || "";
    
    setForm((prev) => {
      // 現在の選択が新しいリストに含まれているか確認
      const isCurrentValid = models.includes(prev.defaultLlmModel);
      return {
        ...prev,
        llmModels: value,
        defaultLlmModel: isCurrentValid ? prev.defaultLlmModel : firstModel,
      };
    });
  };

  const handleSave = async () => {
    try {
      await onSave(form);
      setSaveStatus("saved");
    } catch (e) {
      console.error("[SettingsTab] 保存失敗:", e);
      setSaveStatus("error");
    }
    setTimeout(() => setSaveStatus(""), 3000);
  };

  const models = form.llmModels
    .split(",")
    .map((m) => m.trim())
    .filter(Boolean);

  return (
    <div className="settings-tab">
      <div className="settings-group">
        <label className="settings-label" htmlFor="api-base-url">
          API Base URL
        </label>
        <input
          id="api-base-url"
          className="input"
          type="text"
          value={form.apiBaseUrl}
          onChange={(e) => setForm({ ...form, apiBaseUrl: e.target.value })}
        />
      </div>

      <div className="settings-group">
        <label className="settings-label" htmlFor="mic-device">
          マイクデバイス
        </label>
        <select
          id="mic-device"
          className="select"
          value={form.microphoneDeviceNumber}
          onChange={(e) =>
            setForm({ ...form, microphoneDeviceNumber: Number(e.target.value) })
          }
        >
          {micDevices.map((d) => (
            <option key={d.index} value={d.index}>
              {d.name}
            </option>
          ))}
          {micDevices.length === 0 && (
            <option value={0}>デバイス 0（デフォルト）</option>
          )}
        </select>
      </div>

      <div className="settings-group">
        <label className="settings-label" htmlFor="whisper-model">
          Whisperモデル名
        </label>
        <input
          id="whisper-model"
          className="input"
          type="text"
          value={form.whisperModel}
          onChange={(e) => setForm({ ...form, whisperModel: e.target.value })}
        />
      </div>

      <div className="settings-group">
        <label className="settings-label" htmlFor="llm-models">
          LLMモデル一覧（カンマ区切り）
        </label>
        <input
          id="llm-models"
          className="input"
          type="text"
          value={form.llmModels}
          onChange={(e) => handleLlmModelsChange(e.target.value)}
        />
      </div>

      <div className="settings-group">
        <label className="settings-label" htmlFor="default-llm">
          デフォルトLLM（校正用）
        </label>
        <select
          id="default-llm"
          className="select"
          value={form.defaultLlmModel}
          onChange={(e) => setForm({ ...form, defaultLlmModel: e.target.value })}
        >
          {models.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </div>

      <div className="settings-group">
        <label className="settings-label" htmlFor="correction-prompt">
          校正プロンプト
        </label>
        <textarea
          id="correction-prompt"
          className="textarea"
          value={form.correctionPrompt}
          onChange={(e) => setForm({ ...form, correctionPrompt: e.target.value })}
          rows={4}
        />
      </div>

      <div className="settings-group">
        <label className="settings-label" htmlFor="post-delay">
          録音終了後の待機時間（ミリ秒）
        </label>
        <input
          id="post-delay"
          className="input input-number"
          type="number"
          min={0}
          max={5000}
          value={form.postRecordingDelayMs}
          onChange={(e) =>
            setForm({ ...form, postRecordingDelayMs: Number(e.target.value) })
          }
        />
      </div>

      <div className="settings-group settings-checkbox">
        <label className="checkbox-label" htmlFor="auto-correction">
          <input
            id="auto-correction"
            type="checkbox"
            checked={form.isAutoCorrectionEnabled}
            onChange={(e) =>
              setForm({ ...form, isAutoCorrectionEnabled: e.target.checked })
            }
          />
          音声認識後に自動校正を行う
        </label>
      </div>

      <div className="settings-group settings-checkbox">
        <label className="checkbox-label" htmlFor="start-minimized">
          <input
            id="start-minimized"
            type="checkbox"
            checked={form.startMinimized}
            onChange={(e) =>
              setForm({ ...form, startMinimized: e.target.checked })
            }
          />
          起動時に最小化（タスクトレイに格納）
        </label>
      </div>

      <div className="settings-group">
        <label className="settings-label">表示テーマ</label>
        <div className="theme-toggle">
          <label className={`theme-option ${form.theme === "dark" ? "active" : ""}`}>
            <input
              type="radio"
              name="theme"
              value="dark"
              checked={form.theme === "dark"}
              onChange={() => setForm({ ...form, theme: "dark" })}
            />
            🌙 ダーク
          </label>
          <label className={`theme-option ${form.theme === "light" ? "active" : ""}`}>
            <input
              type="radio"
              name="theme"
              value="light"
              checked={form.theme === "light"}
              onChange={() => setForm({ ...form, theme: "light" })}
            />
            ☀️ ライト
          </label>
        </div>
      </div>

      <div className="settings-actions">
        <button id="save-settings-btn" className="btn btn-primary" onClick={handleSave}>
          設定を保存
        </button>
        {saveStatus === "saved" && (
          <span className="save-status saved">✓ 保存しました</span>
        )}
        {saveStatus === "error" && (
          <span className="save-status error">✗ 保存に失敗しました</span>
        )}
      </div>
    </div>
  );
};

export default SettingsTab;
