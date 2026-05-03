export interface HistoryItem {
  id: string;
  timestamp: string; // ISO8601
  originalText: string;
  correctedText: string; // 空文字の場合は未校正
}
