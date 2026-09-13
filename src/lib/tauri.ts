import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Session,
  Question,
  Settings,
  DecisionEntry,
  AnswerValue,
  PrototypeVersion,
  PrototypeGenResult,
  PrototypeFile,
  ChatMessage,
  AgentToolStatus,
  IterationGraph,
  TimelineCommit,
  GitStatus,
  OutlineInfo,
  OutlineNode,
  PipelineInfo,
  Ticket,
  Round,
  RoundArchive,
} from "./types";

export const api = {
  createSession: (title: string, role: string, initialContext?: string) =>
    invoke<Session>("create_session", { title, role, initialContext }),
  deleteSession: (id: string) => invoke<void>("delete_session", { id }),
  getSessions: () => invoke<Session[]>("get_sessions"),
  getSession: (id: string) => invoke<Session | null>("get_session", { id }),
  getQuestions: (sessionId: string) => invoke<Question[]>("get_questions", { sessionId }),
  answerQuestion: (sessionId: string, questionId: string, answer: AnswerValue) =>
    invoke<void>("answer_question", { sessionId, questionId, answer }),
  skipQuestion: (sessionId: string, questionId: string) =>
    invoke<void>("skip_question", { sessionId, questionId }),
  regenerateStale: (sessionId: string, questionIds: string[]) =>
    invoke<void>("regenerate_stale", { sessionId, questionIds }),
  dismissStale: (questionId: string) => invoke<void>("dismiss_stale", { questionId }),
  finishSession: (sessionId: string) =>
    invoke<[DecisionEntry[], string]>("finish_session", { sessionId }),
  getOutline: (sessionId: string) => invoke<OutlineInfo>("get_outline", { sessionId }),
  generateOutline: (sessionId: string) =>
    invoke<void>("generate_outline", { sessionId }),
  saveOutline: (sessionId: string, nodes: OutlineNode[]) =>
    invoke<void>("save_outline", { sessionId, nodes }),
  confirmOutline: (sessionId: string) =>
    invoke<void>("confirm_outline", { sessionId }),
  dismissOutline: (sessionId: string) =>
    invoke<void>("dismiss_outline", { sessionId }),
  requestBatch: (sessionId: string) =>
    invoke<void>("request_batch", { sessionId }),
  exportSession: (sessionId: string, format: "md" | "json") =>
    invoke<string>("export_session", { sessionId, format }),
  generatePrototype: (sessionId: string, feedback?: string) =>
    invoke<PrototypeGenResult>("generate_prototype", { sessionId, feedback: feedback ?? null }),
  getPrototypeVersions: (sessionId: string) =>
    invoke<PrototypeVersion[]>("get_prototype_versions", { sessionId }),
  getPrototypePreviewUrl: (sessionId: string) =>
    invoke<string>("get_prototype_preview_url", { sessionId }),
  getPrototypeFiles: (sessionId: string) =>
    invoke<PrototypeFile[]>("get_prototype_files", { sessionId }),
  readPrototypeFile: (sessionId: string, path: string) =>
    invoke<string>("read_prototype_file", { sessionId, path }),
  sendMessage: (sessionId: string, content: string) =>
    invoke<void>("send_message", { sessionId, content }),
  getMessages: (sessionId: string) => invoke<ChatMessage[]>("get_messages", { sessionId }),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  listAgentTools: () => invoke<AgentToolStatus[]>("list_agent_tools"),
  getActiveSession: () =>
    invoke<{
      session_id: string | null;
      title?: string | null;
      role?: string | null;
      status?: string | null;
      prototype_version?: number | null;
      workspace_path?: string | null;
      prototype_path?: string | null;
      has_prototype: boolean;
    }>("get_active_session"),
  setActiveSession: (sessionId: string | null) =>
    invoke<{ session_id: string | null; title?: string | null }>("set_active_session", {
      sessionId,
    }),
  getMcpInfo: () => invoke<Record<string, unknown>>("get_mcp_info"),
  getTimeline: (sessionId: string) => invoke<GitStatus>("get_timeline", { sessionId }),
  initTimeline: (sessionId: string) => invoke<GitStatus>("init_timeline", { sessionId }),
  getPrototypeDirPath: (sessionId: string) =>
    invoke<string>("get_prototype_dir_path", { sessionId }),
  openPrototypeDir: (sessionId: string) => invoke<string>("open_prototype_dir", { sessionId }),
  listTimelineCommits: (sessionId: string, limit?: number) =>
    invoke<TimelineCommit[]>("list_timeline_commits", { sessionId, limit: limit ?? null }),
  checkoutTimeline: (sessionId: string, refName: string) =>
    invoke<void>("checkout_timeline", { sessionId, refName }),
  forkTimeline: (sessionId: string, branchName: string, fromSha?: string) =>
    invoke<string>("fork_timeline", {
      sessionId,
      branchName,
      fromSha: fromSha ?? null,
    }),
  getIterationGraph: (sessionId: string) =>
    invoke<IterationGraph>("get_iteration_graph", { sessionId }),
  saveIterationGraph: (sessionId: string, graph: IterationGraph) =>
    invoke<void>("save_iteration_graph", { sessionId, graph }),
  runIterationGraph: (sessionId: string) =>
    invoke<IterationGraph>("run_iteration_graph", { sessionId }),
  cancelIterationGraph: (sessionId: string) =>
    invoke<void>("cancel_iteration_graph", { sessionId }),
  // --- pipeline: spec → tickets → branch map ---
  getPipeline: (sessionId: string) => invoke<PipelineInfo>("get_pipeline", { sessionId }),
  generateSpec: (sessionId: string) => invoke<void>("generate_spec", { sessionId }),
  saveSpec: (sessionId: string, spec: string) =>
    invoke<void>("save_spec", { sessionId, spec }),
  confirmSpec: (sessionId: string) => invoke<void>("confirm_spec", { sessionId }),
  generateTickets: (sessionId: string) =>
    invoke<void>("generate_tickets", { sessionId }),
  saveTickets: (sessionId: string, tickets: Ticket[]) =>
    invoke<void>("save_tickets", { sessionId, tickets }),
  confirmTickets: (sessionId: string) =>
    invoke<void>("confirm_tickets", { sessionId }),
  setTicketStatus: (ticketId: string, status: string) =>
    invoke<void>("set_ticket_status", { ticketId, status }),
  runTicket: (
    sessionId: string,
    ticketId: string,
    opts?: { rounds?: number; branchName?: string; fromSha?: string; feedback?: string }
  ) =>
    invoke<string>("run_ticket", {
      sessionId,
      ticketId,
      rounds: opts?.rounds ?? null,
      branchName: opts?.branchName ?? null,
      fromSha: opts?.fromSha ?? null,
      feedback: opts?.feedback ?? null,
    }),
  // --- development rounds ---
  startRound: (sessionId: string, title: string, goal?: string) =>
    invoke<Round>("start_round", { sessionId, title, goal: goal ?? null }),
  archiveRound: (sessionId: string, force?: boolean) =>
    invoke<Round>("archive_round", { sessionId, force: force ?? null }),
  listRounds: (sessionId: string) => invoke<Round[]>("list_rounds", { sessionId }),
  getRoundArchive: (roundId: string) =>
    invoke<RoundArchive>("get_round_archive", { roundId }),
};

export function onNewQuestion(cb: (q: Question) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string; question: Question }>("new_question", (e) => {
    cb(e.payload.question);
  });
}

export function onStaleMarked(
  cb: (session_id: string, question_ids: string[], triggered_by_text: string) => void
): Promise<UnlistenFn> {
  return listen<{
    session_id: string;
    question_ids: string[];
    triggered_by_question_id: string;
    triggered_by_question_text: string;
  }>("stale_marked", (e) => {
    cb(e.payload.session_id, e.payload.question_ids, e.payload.triggered_by_question_text);
  });
}

export function onSessionComplete(
  cb: (session_id: string, suggestion: string) => void
): Promise<UnlistenFn> {
  return listen<{ session_id: string; suggestion: string }>("session_complete", (e) => {
    cb(e.payload.session_id, e.payload.suggestion);
  });
}

export function onError(
  cb: (session_id: string, message: string, kind: string) => void
): Promise<UnlistenFn> {
  return listen<{ session_id: string; message: string; kind: string }>("error", (e) => {
    cb(e.payload.session_id, e.payload.message, e.payload.kind);
  });
}

export function onBatchStatus(
  cb: (session_id: string, status: string, remaining: number) => void
): Promise<UnlistenFn> {
  return listen<{ session_id: string; status: string; remaining: number }>("batch_status", (e) => {
    cb(e.payload.session_id, e.payload.status, e.payload.remaining);
  });
}

export function onInterviewMayComplete(cb: (session_id: string) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string }>("interview_may_complete", (e) => {
    cb(e.payload.session_id);
  });
}

export function onOutlineUpdated(
  cb: (payload: OutlineInfo & { session_id: string }) => void
): Promise<UnlistenFn> {
  return listen<OutlineInfo & { session_id: string }>("outline_updated", (e) => {
    cb(e.payload);
  });
}

export function onChatStream(
  cb: (session_id: string, delta: string) => void
): Promise<UnlistenFn> {
  return listen<{ session_id: string; delta: string }>("chat_stream", (e) => {
    cb(e.payload.session_id, e.payload.delta);
  });
}

export function onChatMessageDone(cb: (message: ChatMessage) => void): Promise<UnlistenFn> {
  return listen<{ session_id: string; message: ChatMessage }>("chat_message_done", (e) => {
    cb(e.payload.message);
  });
}

export function onPrototypeStatus(
  cb: (session_id: string, status: string, message?: string) => void
): Promise<UnlistenFn> {
  return listen<{ session_id: string; status: string; message?: string | null }>(
    "prototype_status",
    (e) => {
      cb(e.payload.session_id, e.payload.status, e.payload.message ?? undefined);
    }
  );
}

export function onPrototypeUpdated(
  cb: (payload: {
    session_id: string;
    version: number;
    preview_url: string;
    changelog?: string | null;
    file_count: number;
  }) => void
): Promise<UnlistenFn> {
  return listen<{
    session_id: string;
    version: number;
    preview_url: string;
    changelog?: string | null;
    file_count: number;
  }>("prototype_updated", (e) => {
    cb(e.payload);
  });
}

export function onAgentOutput(
  cb: (session_id: string, stream: string, text: string) => void
): Promise<UnlistenFn> {
  return listen<{ session_id: string; stream: string; text: string }>("agent_output", (e) => {
    cb(e.payload.session_id, e.payload.stream, e.payload.text);
  });
}

export function onGraphProgress(
  cb: (payload: {
    session_id: string;
    status: string;
    step_index: number;
    step_total: number;
    node_id?: string | null;
    label?: string | null;
    message?: string | null;
    commit?: string | null;
  }) => void
): Promise<UnlistenFn> {
  return listen<{
    session_id: string;
    status: string;
    step_index: number;
    step_total: number;
    node_id?: string | null;
    label?: string | null;
    message?: string | null;
    commit?: string | null;
  }>("graph_progress", (e) => cb(e.payload));
}

export function onGraphNodeStatus(
  cb: (payload: {
    session_id: string;
    node_id: string;
    status: string;
    round?: number | null;
    total_rounds?: number | null;
    commit?: string | null;
    error?: string | null;
  }) => void
): Promise<UnlistenFn> {
  return listen<{
    session_id: string;
    node_id: string;
    status: string;
    round?: number | null;
    total_rounds?: number | null;
    commit?: string | null;
    error?: string | null;
  }>("graph_node_status", (e) => cb(e.payload));
}

export function onTimelineUpdated(
  cb: (payload: {
    session_id: string;
    branch?: string | null;
    head?: string | null;
    commit_count: number;
  }) => void
): Promise<UnlistenFn> {
  return listen<{
    session_id: string;
    branch?: string | null;
    head?: string | null;
    commit_count: number;
  }>("timeline_updated", (e) => cb(e.payload));
}

export function onPipelineUpdated(
  cb: (payload: PipelineInfo & { session_id: string }) => void
): Promise<UnlistenFn> {
  return listen<PipelineInfo & { session_id: string }>("pipeline_updated", (e) => {
    cb(e.payload);
  });
}

export function onRoundStarted(
  cb: (payload: { session_id: string; round: Round }) => void
): Promise<UnlistenFn> {
  return listen<{ session_id: string; round: Round }>("round_started", (e) =>
    cb(e.payload)
  );
}

export function onRoundArchived(
  cb: (payload: { session_id: string; round: Round }) => void
): Promise<UnlistenFn> {
  return listen<{ session_id: string; round: Round }>("round_archived", (e) =>
    cb(e.payload)
  );
}

export function onSessionCreated(
  cb: (session: Session) => void
): Promise<UnlistenFn> {
  return listen<Session>("session_created", (e) => cb(e.payload));
}

export function onSessionDeleted(
  cb: (session_id: string) => void
): Promise<UnlistenFn> {
  return listen<{ session_id: string }>("session_deleted", (e) => cb(e.payload.session_id));
}

export function onActiveSessionChanged(
  cb: (payload: { session_id: string | null; title?: string | null }) => void
): Promise<UnlistenFn> {
  return listen<{ session_id: string | null; title?: string | null }>(
    "active_session_changed",
    (e) => cb(e.payload)
  );
}
