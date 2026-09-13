import { create } from "zustand";
import type { Session, Question, Settings, OutlineNode, OutlineStatus, PipelineStage, Ticket, Round } from "@/lib/types";

interface SessionStore {
  sessions: Session[];
  currentSessionId: string | null;
  questions: Question[];
  settings: Settings | null;
  isLoading: boolean;
  isGenerating: boolean;
  isPrototypeGenerating: boolean;
  outlineStatus: OutlineStatus;
  outlineNodes: OutlineNode[];
  pipelineStage: PipelineStage;
  spec: string | null;
  tickets: Ticket[];
  rounds: Round[];
  currentRound: Round | null;
  runningTickets: string[];

  setSessions: (sessions: Session[]) => void;
  setCurrentSession: (id: string | null) => void;
  addSession: (session: Session) => void;
  removeSession: (id: string) => void;
  setQuestions: (questions: Question[]) => void;
  addQuestion: (question: Question) => void;
  updateQuestion: (id: string, updates: Partial<Question>) => void;
  markStale: (questionIds: string[], triggeredBy?: string) => void;
  setSettings: (settings: Settings) => void;
  setLoading: (loading: boolean) => void;
  setGenerating: (generating: boolean) => void;
  setPrototypeGenerating: (generating: boolean) => void;
  setOutline: (status: OutlineStatus, nodes: OutlineNode[]) => void;
  setPipeline: (
    stage: PipelineStage,
    spec: string | null,
    tickets: Ticket[],
    round?: Round | null,
    running?: string[]
  ) => void;
  setRounds: (rounds: Round[], currentRoundId?: string | null) => void;
  bumpSessionPrototypeVersion: (sessionId: string, version: number) => void;
}

export const useSessionStore = create<SessionStore>((set) => ({
  sessions: [],
  currentSessionId: null,
  questions: [],
  settings: null,
  isLoading: false,
  isGenerating: false,
  isPrototypeGenerating: false,
  outlineStatus: "none",
  outlineNodes: [],
  pipelineStage: "none",
  spec: null,
  tickets: [],
  rounds: [],
  currentRound: null,
  runningTickets: [],

  setSessions: (sessions) => set({ sessions }),

  setCurrentSession: (id) =>
    set({ currentSessionId: id, questions: [], outlineStatus: "none", outlineNodes: [], pipelineStage: "none", spec: null, tickets: [], rounds: [], currentRound: null, runningTickets: [] }),

  addSession: (session) =>
    set((state) => ({ sessions: [session, ...state.sessions] })),

  removeSession: (id) =>
    set((state) => ({
      sessions: state.sessions.filter((s) => s.id !== id),
      currentSessionId: state.currentSessionId === id ? null : state.currentSessionId,
    })),

  setQuestions: (questions) =>
    set({
      questions: [...questions].sort((a, b) => a.display_order - b.display_order),
    }),

  addQuestion: (question) =>
    set((state) => {
      const filtered = state.questions.filter((q) => q.id !== question.id);
      const next = [...filtered, question];
      next.sort((a, b) => a.display_order - b.display_order);
      return { questions: next };
    }),

  updateQuestion: (id, updates) =>
    set((state) => ({
      questions: state.questions.map((q) =>
        q.id === id ? { ...q, ...updates } : q
      ),
    })),

  markStale: (questionIds, triggeredBy) =>
    set((state) => ({
      questions: state.questions.map((q) =>
        questionIds.includes(q.id) ? { ...q, status: "stale", stale_triggered_by: triggeredBy } : q
      ),
    })),

  setSettings: (settings) => set({ settings }),

  setLoading: (isLoading) => set({ isLoading }),

  setGenerating: (isGenerating) => set({ isGenerating }),

  setPrototypeGenerating: (isPrototypeGenerating) => set({ isPrototypeGenerating }),

  setOutline: (outlineStatus, outlineNodes) => set({ outlineStatus, outlineNodes }),

  setPipeline: (pipelineStage, spec, tickets, round, running) =>
    set((state) => ({
      pipelineStage,
      spec,
      tickets,
      currentRound: round === undefined ? state.currentRound : round,
      runningTickets: running ?? state.runningTickets,
    })),

  setRounds: (rounds, currentRoundId) =>
    set((state) => {
      const current =
        rounds.find((r) => r.id === currentRoundId) ??
        rounds.find((r) => r.status === "active") ??
        null;
      return { rounds, currentRound: current ?? state.currentRound };
    }),

  bumpSessionPrototypeVersion: (sessionId, version) =>
    set((state) => ({
      sessions: state.sessions.map((s) =>
        s.id === sessionId ? { ...s, prototype_version: Math.max(s.prototype_version, version) } : s
      ),
    })),
}));
