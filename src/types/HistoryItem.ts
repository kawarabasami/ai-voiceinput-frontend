export interface HistoryItem {
  id: string;
  timestamp: string; // ISO8601
  originalText: string;
  correctedText: string;
  audioPath?: string;
}
