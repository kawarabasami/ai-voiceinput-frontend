import { useState, useCallback } from "react";
import { ChatMessage } from "../types/ChatMessage";

export function useChatMessages() {
  const [messages, setMessages] = useState<ChatMessage[]>([]);

  const addMessage = useCallback((role: ChatMessage["role"], content: string) => {
    const msg: ChatMessage = {
      role,
      content,
      timestamp: new Date().toISOString(),
    };
    setMessages((prev) => [...prev, msg]);
    return msg;
  }, []);

  const clearMessages = useCallback(() => {
    setMessages([]);
  }, []);

  return { messages, addMessage, clearMessages };
}
