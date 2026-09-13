import { useState } from "react";
import {
  ListChecks,
  ChevronDown,
  ChevronRight,
  Check,
  RotateCcw,
  Trash2,
  Plus,
  FileText,
  GitBranch,
  Sparkles,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { useSessionStore } from "@/store/sessionStore";
import { api } from "@/lib/tauri";
import { toast } from "sonner";
import type { PipelineStage, Ticket } from "@/lib/types";

const STAGES: { key: PipelineStage; label: string }[] = [
  { key: "interviewing", label: "访谈" },
  { key: "spec_draft", label: "Spec" },
  { key: "tickets_draft", label: "Tickets" },
  { key: "developing", label: "开发" },
];

const TICKET_STATUS_LABEL: Record<string, string> = {
  pending: "待开发",
  in_progress: "进行中",
  done: "已完成",
  rejected: "已废弃",
};

/** Staged pipeline: interview → spec → tickets → branch-map development. */
export function PipelinePanel() {
  const { pipelineStage, spec, tickets, currentSessionId } = useSessionStore();
  const [expanded, setExpanded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [specDraft, setSpecDraft] = useState<string | null>(null);
  const [newTitle, setNewTitle] = useState("");

  if (pipelineStage === "none") return null; // legacy free-mode session

  const stageIdx = STAGES.findIndex((s) => s.key === pipelineStage);
  const isSpecDraft = pipelineStage === "spec_draft";
  const isTicketsDraft = pipelineStage === "tickets_draft";
  const isDeveloping = pipelineStage === "developing";

  const run = async (fn: () => Promise<unknown>, okMsg?: string) => {
    setBusy(true);
    try {
      await fn();
      if (okMsg) toast.success(okMsg);
    } catch (e) {
      console.error(e);
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };

  const saveTickets = (next: Ticket[]) =>
    currentSessionId &&
    run(() => api.saveTickets(currentSessionId, next));

  const patchTicket = (id: string, patch: Partial<Ticket>) =>
    saveTickets(tickets.map((t) => (t.id === id ? { ...t, ...patch } : t)));

  const moveTicket = (id: string, dir: -1 | 1) => {
    const idx = tickets.findIndex((t) => t.id === id);
    const j = idx + dir;
    if (idx < 0 || j < 0 || j >= tickets.length) return;
    const next = [...tickets];
    [next[idx], next[j]] = [next[j], next[idx]];
    saveTickets(next.map((t, i) => ({ ...t, display_order: i })));
  };

  const addTicket = () => {
    const title = newTitle.trim();
    if (!title) return;
    saveTickets([
      ...tickets,
      {
        id: `t${Date.now().toString(36)}`,
        session_id: currentSessionId!,
        title,
        status: "pending",
        depends_on: [],
        branches: [],
        display_order: tickets.length,
        created_at: new Date().toISOString(),
      },
    ]);
    setNewTitle("");
  };

  return (
    <Card className="border-primary/20">
      <CardContent className="p-3">
        {/* Stage stepper */}
        <button
          className="flex w-full items-center justify-between"
          onClick={() => setExpanded((v) => !v)}
        >
          <div className="flex items-center gap-2 text-sm font-medium">
            <ListChecks className="h-4 w-4 text-primary" />
            开发流水线
            <div className="flex items-center gap-1">
              {STAGES.map((s, i) => (
                <span key={s.key} className="flex items-center gap-1">
                  <Badge
                    variant={i <= stageIdx ? "default" : "outline"}
                    className="text-[10px] px-1.5"
                  >
                    {s.label}
                  </Badge>
                  {i < STAGES.length - 1 && (
                    <ChevronRight className="h-3 w-3 text-muted-foreground" />
                  )}
                </span>
              ))}
            </div>
          </div>
          {expanded ? (
            <ChevronDown className="h-4 w-4 text-muted-foreground" />
          ) : (
            <ChevronRight className="h-4 w-4 text-muted-foreground" />
          )}
        </button>

        {!expanded ? null : (
          <div className="mt-3 space-y-3 border-t pt-3">
            {/* interviewing → generate spec */}
            {pipelineStage === "interviewing" && (
              <div className="flex items-center justify-between gap-2">
                <p className="text-xs text-muted-foreground">
                  访谈进行中。覆盖完成后可总结成开发 spec。
                </p>
                <Button
                  size="sm"
                  variant="outline"
                  disabled={busy || !currentSessionId}
                  onClick={() =>
                    run(
                      () => api.generateSpec(currentSessionId!),
                      "spec 生成中…"
                    )
                  }
                >
                  <Sparkles className="mr-1 h-3.5 w-3.5" />
                  生成 spec
                </Button>
              </div>
            )}

            {/* spec draft editor */}
            {isSpecDraft && (
              <div className="space-y-2">
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <FileText className="h-3.5 w-3.5" />
                  spec 草稿 — 编辑后确认，确认后自动生成 tickets
                </div>
                <textarea
                  className="min-h-[220px] w-full rounded-md border bg-background p-2 font-mono text-xs"
                  value={specDraft ?? spec ?? ""}
                  onChange={(e) => setSpecDraft(e.target.value)}
                  onBlur={() => {
                    const v = specDraft;
                    if (v !== null && v !== spec && currentSessionId) {
                      void run(() => api.saveSpec(currentSessionId, v), "spec 已保存");
                    }
                    setSpecDraft(null);
                  }}
                />
                <div className="flex items-center gap-2">
                  <Button
                    size="sm"
                    disabled={busy || !currentSessionId}
                    onClick={() =>
                      run(
                        () => api.confirmSpec(currentSessionId!),
                        "spec 已确认，正在生成 tickets…"
                      )
                    }
                  >
                    <Check className="mr-1 h-3.5 w-3.5" />
                    确认 spec
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={busy || !currentSessionId}
                    onClick={() =>
                      run(() => api.generateSpec(currentSessionId!))
                    }
                  >
                    <RotateCcw className="mr-1 h-3.5 w-3.5" />
                    重新生成
                  </Button>
                </div>
              </div>
            )}

            {/* tickets draft editor */}
            {isTicketsDraft && (
              <div className="space-y-2">
                <p className="text-xs text-muted-foreground">
                  tickets 草稿 — 增删/改描述/调依赖后确认；确认后进入分支地图开发
                </p>
                {tickets.map((t, i) => (
                  <div key={t.id} className="rounded-md border p-2 space-y-1.5">
                    <div className="flex items-center gap-1.5">
                      <Badge variant="outline" className="text-[10px]">
                        {t.id}
                      </Badge>
                      <Input
                        className="h-7 flex-1 text-xs"
                        value={t.title}
                        onChange={(e) =>
                          void patchTicket(t.id, { title: e.target.value })
                        }
                      />
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-6 w-6"
                        disabled={i === 0}
                        onClick={() => void moveTicket(t.id, -1)}
                      >
                        ↑
                      </Button>
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-6 w-6"
                        disabled={i === tickets.length - 1}
                        onClick={() => void moveTicket(t.id, 1)}
                      >
                        ↓
                      </Button>
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-6 w-6 text-destructive"
                        onClick={() =>
                          void saveTickets(tickets.filter((x) => x.id !== t.id))
                        }
                      >
                        <Trash2 className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                    <textarea
                      className="min-h-[40px] w-full rounded border bg-background p-1.5 text-xs"
                      value={t.description ?? ""}
                      placeholder="描述 + 验收标准"
                      onChange={(e) =>
                        void patchTicket(t.id, { description: e.target.value })
                      }
                    />
                    {tickets.length > 1 && (
                      <div className="flex flex-wrap items-center gap-1 text-[10px] text-muted-foreground">
                        依赖：
                        {tickets
                          .filter((x) => x.id !== t.id)
                          .map((x) => (
                            <button
                              key={x.id}
                              className={`rounded border px-1 ${
                                t.depends_on.includes(x.id)
                                  ? "bg-primary text-primary-foreground"
                                  : ""
                              }`}
                              onClick={() =>
                                void patchTicket(t.id, {
                                  depends_on: t.depends_on.includes(x.id)
                                    ? t.depends_on.filter((d) => d !== x.id)
                                    : [...t.depends_on, x.id],
                                })
                              }
                            >
                              {x.id}
                            </button>
                          ))}
                      </div>
                    )}
                  </div>
                ))}
                <div className="flex items-center gap-1.5">
                  <Input
                    className="h-7 flex-1 text-xs"
                    placeholder="新 ticket 标题…"
                    value={newTitle}
                    onChange={(e) => setNewTitle(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && addTicket()}
                  />
                  <Button size="sm" variant="outline" onClick={addTicket}>
                    <Plus className="h-3.5 w-3.5" />
                  </Button>
                </div>
                <div className="flex items-center gap-2">
                  <Button
                    size="sm"
                    disabled={busy || !currentSessionId || tickets.length === 0}
                    onClick={() =>
                      run(
                        () => api.confirmTickets(currentSessionId!),
                        "已进入开发阶段"
                      )
                    }
                  >
                    <Check className="mr-1 h-3.5 w-3.5" />
                    确认 tickets
                  </Button>
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={busy || !currentSessionId}
                    onClick={() =>
                      run(() => api.generateTickets(currentSessionId!))
                    }
                  >
                    <RotateCcw className="mr-1 h-3.5 w-3.5" />
                    重新生成
                  </Button>
                </div>
              </div>
            )}

            {/* developing — compact ticket status */}
            {isDeveloping && (
              <div className="space-y-1.5">
                {tickets.map((t) => (
                  <div
                    key={t.id}
                    className="flex items-center gap-2 text-xs"
                  >
                    <Badge
                      variant={t.status === "done" ? "default" : "outline"}
                      className="text-[10px]"
                    >
                      {TICKET_STATUS_LABEL[t.status] ?? t.status}
                    </Badge>
                    <span className="flex-1 truncate">{t.title}</span>
                    {t.branches.length > 0 && (
                      <span className="flex items-center gap-0.5 text-muted-foreground">
                        <GitBranch className="h-3 w-3" />
                        {t.branches.length}
                      </span>
                    )}
                  </div>
                ))}
                <p className="text-[10px] text-muted-foreground">
                  在右侧「地图」页签运行各条开发线
                </p>
              </div>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
