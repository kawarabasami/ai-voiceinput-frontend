export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  timestamp: string; // ISO8601
}
