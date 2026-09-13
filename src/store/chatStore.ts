import { create } from "zustand";
import type { ChatMessage } from "@/lib/types";
import { api } from "@/lib/tauri";

interface ChatState {
  messages: ChatMessage[];
  streamingText: string;
  isStreaming: boolean;
  currentSessionId: string | null;

  loadMessages: (sessionId: string) => Promise<void>;
  addUserMessage: (content: string) => void;
  appendStreamText: (delta: string) => void;
  startStream: () => void;
  finishStream: (message: ChatMessage) => void;
  failStream: () => void;
  clearMessages: () => void;
  sendMessage: (sessionId: string, content: string) => Promise<void>;
}

export const useChatStore = create<ChatState>((set, get) => ({
  messages: [],
  streamingText: "",
  isStreaming: false,
  currentSessionId: null,

  loadMessages: async (sessionId: string) => {
    try {
      const msgs = await api.getMessages(sessionId);
      set({ messages: msgs, currentSessionId: sessionId, streamingText: "", isStreaming: false });
    } catch (e) {
      console.error("[chatStore] Failed to load messages", e);
      set({ messages: [], currentSessionId: sessionId });
    }
  },

  addUserMessage: (content: string) => {
    const sessionId = get().currentSessionId;
    if (!sessionId) return;
    const msg: ChatMessage = {
      id: `temp-${Date.now()}`,
      session_id: sessionId,
      role: "user",
      content,
      question_ids: [],
      created_at: new Date().toISOString(),
    };
    set((s) => ({ messages: [...s.messages, msg] }));
  },

  appendStreamText: (delta: string) => {
    set((s) => ({ streamingText: s.streamingText + delta }));
  },

  startStream: () => {
    set({ streamingText: "", isStreaming: true });
  },

  finishStream: (message: ChatMessage) => {
    set((s) => {
      if (s.messages.some((m) => m.id === message.id)) {
        return { streamingText: "", isStreaming: false };
      }
      return {
        messages: [...s.messages, message],
        streamingText: "",
        isStreaming: false,
      };
    });
  },

  failStream: () => {
    set({ streamingText: "", isStreaming: false });
  },

  clearMessages: () => {
    set({ messages: [], streamingText: "", isStreaming: false, currentSessionId: null });
  },

  sendMessage: async (sessionId: string, content: string) => {
    get().addUserMessage(content);
    get().startStream();
    try {
      await api.sendMessage(sessionId, content);
    } catch (e) {
      get().failStream();
      throw e;
    }
  },
}));
