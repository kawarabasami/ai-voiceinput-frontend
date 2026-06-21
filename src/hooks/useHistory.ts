import { useState, useCallback } from "react";
import { HistoryItem } from "../types/HistoryItem";

let idCounter = 0;

export function useHistory() {
  const [history, setHistory] = useState<HistoryItem[]>([]);

  const addItem = useCallback((originalText: string, audioPath?: string): HistoryItem => {
    const item: HistoryItem = {
      id: `${Date.now()}-${++idCounter}`,
      timestamp: new Date().toISOString(),
      originalText,
      correctedText: "",
      audioPath,
    };

    setHistory((prev) =>
      [item, ...prev].map((historyItem, index) =>
        index < 3 ? historyItem : { ...historyItem, audioPath: undefined }
      )
    );
    return item;
  }, []);

  const updateCorrectedText = useCallback((id: string, correctedText: string) => {
    setHistory((prev) =>
      prev.map((item) => (item.id === id ? { ...item, correctedText } : item))
    );
  }, []);

  return { history, addItem, updateCorrectedText };
}
