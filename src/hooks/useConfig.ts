import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppConfig, DEFAULT_CONFIG } from "../types/AppConfig";

export function useConfig() {
  const [config, setConfig] = useState<AppConfig>(DEFAULT_CONFIG);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    invoke<AppConfig>("load_config")
      .then((cfg) => setConfig({ ...DEFAULT_CONFIG, ...cfg }))
      .catch((e) => console.error("[useConfig] 設定読み込み失敗:", e))
      .finally(() => setLoading(false));
  }, []);

  const saveConfig = useCallback(async (newConfig: AppConfig) => {
    await invoke("save_config", { config: newConfig });
    setConfig(newConfig);
  }, []);

  return { config, setConfig, saveConfig, loading };
}
