export type QuestionType = "choice" | "multi" | "open";
export type QuestionCategory = "intent" | "choice" | "open" | "tradeoff" | "dependency";
export type QuestionStatus = "generating" | "ready" | "answered" | "skipped" | "stale";
export type SessionStatus = "active" | "completed";
export type BatchStatus = "generating" | "done" | "failed";
export type OutlineStatus = "none" | "generating" | "draft" | "confirmed";
export type OutlineNodeStatus = "pending" | "covered" | "excluded";

export interface OutlineNode {
  id: string;
  session_id: string;
  title: string;
  description?: string;
  status: OutlineNodeStatus;
  display_order: number;
  created_at: string;
}

export interface OutlineInfo {
  status: OutlineStatus;
  nodes: OutlineNode[];
}

export interface QuestionOption {
  label: string;
  description?: string;
}

export type AnswerValue =
  | { kind: "choice"; option: string }
  | { kind: "multi"; options: string[] }
  | { kind: "open"; text: string };

export interface Question {
  id: string;
  session_id: string;
  batch_id: string;
  q_type: QuestionType;
  category: QuestionCategory;
  question: string;
  context?: string;
  options: QuestionOption[];
  recommended_option?: string;
  rationale?: string;
  depends_on: string[];
  status: QuestionStatus;
  answer?: AnswerValue;
  answer_version: number;
  display_order: number;
  stale_triggered_by?: string;
  message_id?: string;
  outline_node_id?: string;
}

export type PipelineStage =
  | "none"
  | "interviewing"
  | "spec_draft"
  | "tickets_draft"
  | "developing";

export type TicketStatus = "pending" | "in_progress" | "done" | "rejected";

export interface Ticket {
  id: string;
  session_id: string;
  title: string;
  description?: string;
  status: TicketStatus;
  depends_on: string[];
  branches: string[];
  display_order: number;
  created_at: string;
}

export interface PipelineInfo {
  stage: PipelineStage;
  spec?: string | null;
  tickets: Ticket[];
  running?: string[];
  /** Active development round; null between rounds (after archive). */
  round?: Round | null;
}

// --- Development rounds (开发循环) ---

export type RoundStatus = "active" | "archived";

export interface Round {
  id: string;
  session_id: string;
  number: number;
  title: string;
  goal?: string | null;
  status: RoundStatus;
  pipeline_stage: PipelineStage;
  spec?: string | null;
  summary?: string | null;
  created_at: string;
  archived_at?: string | null;
}

export interface RoundArchive {
  round: Round;
  nodes: OutlineNode[];
  questions: Question[];
  tickets: Ticket[];
  messages: ChatMessage[];
  decisions: DecisionEntry[];
}

export interface Session {
  id: string;
  title: string;
  initial_context?: string;
  role: string;
  status: SessionStatus;
  summary?: string;
  prototype_version: number;
  outline_status: OutlineStatus;
  pipeline_stage: PipelineStage;
  spec?: string | null;
  current_round_id?: string | null;
  created_at: string;
  updated_at: string;
}

export interface PrototypeFile {
  path: string;
  content: string;
}

export interface PrototypeSnapshot {
  files: PrototypeFile[];
  changelog?: string | null;
}

export interface PrototypeVersion {
  id: string;
  session_id: string;
  version: number;
  snapshot_json: string;
  feedback?: string;
  created_at: string;
}

export interface PrototypeGenResult {
  version: number;
  preview_url: string;
  changelog?: string | null;
  file_count: number;
  commit?: string | null;
}

export interface ChatMessage {
  id: string;
  session_id: string;
  role: "user" | "assistant";
  content: string;
  question_ids: string[];
  created_at: string;
}

export interface Settings {
  base_url: string;
  api_key: string;
  model_name: string;
  temperature: number;
  batch_size: number;
  max_concurrent_batches: number;
  debounce_seconds: number;
  questions_per_screen: number;
  agent_tool: string;
  agent_model: string;
  agent_effort: string;
  agent_auto_approve: boolean;
  prototype_auto_paused: boolean;
}

export interface AgentToolStatus {
  id: string;
  available: boolean;
  path?: string | null;
}

export interface DecisionEntry {
  question_id: string;
  question: string;
  answer: string;
  rationale?: string;
  category: QuestionCategory;
}

// --- Iteration blueprint / timeline ---

export type GraphNodeKind = "start" | "agent" | "loop" | "note" | "end";

export interface GraphPos {
  x: number;
  y: number;
}

export interface GraphNode {
  id: string;
  kind: string;
  label: string;
  position: GraphPos;
  instruction: string;
  focus?: string | null;
  count?: number | null;
  status?: string | null;
  last_error?: string | null;
  last_commit?: string | null;
  completed_rounds?: number | null;
}

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
}

export interface IterationGraph {
  id: string;
  session_id: string;
  name: string;
  nodes: GraphNode[];
  edges: GraphEdge[];
  entry_id?: string | null;
  created_at: string;
  updated_at: string;
}

export interface TimelineCommit {
  sha: string;
  short_sha: string;
  subject: string;
  body?: string | null;
  parent_shas: string[];
  branch_tips: string[];
  author_time: string;
  version?: number | null;
  is_head: boolean;
}

export interface GitStatus {
  available: boolean;
  initialized: boolean;
  branch?: string | null;
  dirty: boolean;
  head?: string | null;
  /** Absolute prototype workspace path (git root). */
  path: string;
  has_files: boolean;
}

export interface GraphProgressPayload {
  session_id: string;
  status: string;
  step_index: number;
  step_total: number;
  node_id?: string | null;
  label?: string | null;
  message?: string | null;
  commit?: string | null;
}

export interface GraphNodeStatusPayload {
  session_id: string;
  node_id: string;
  status: string;
  round?: number | null;
  total_rounds?: number | null;
  commit?: string | null;
  error?: string | null;
}
