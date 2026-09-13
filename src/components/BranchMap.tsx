import { useCallback, useEffect, useMemo, useState } from "react";
import { GitBranch, Loader2, Play, CircleDot } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { useSessionStore } from "@/store/sessionStore";
import { api, onTimelineUpdated } from "@/lib/tauri";
import { toast } from "sonner";
import type { Ticket, TimelineCommit } from "@/lib/types";

const STATUS_STYLE: Record<string, string> = {
  pending: "border-muted-foreground/30 text-muted-foreground",
  in_progress: "border-blue-500/50 text-blue-500",
  done: "border-green-500/50 text-green-600",
  rejected: "border-destructive/40 text-destructive line-through",
};
const STATUS_LABEL: Record<string, string> = {
  pending: "待开发",
  in_progress: "进行中",
  done: "已完成",
  rejected: "已废弃",
};

/** Commits unique to a branch: walk parents from tip until main's ancestor set or a claimed sha. */
function branchCommits(
  tipSha: string,
  bySha: Map<string, TimelineCommit>,
  mainAncestors: Set<string>,
  claimed: Set<string>
): TimelineCommit[] {
  const out: TimelineCommit[] = [];
  let cur: string | undefined = tipSha;
  while (cur) {
    if (mainAncestors.has(cur) || claimed.has(cur)) break;
    const c = bySha.get(cur);
    if (!c) break;
    out.push(c);
    claimed.add(cur);
    cur = c.parent_shas[0];
  }
  return out;
}

function ancestorSet(tipSha: string | undefined, bySha: Map<string, TimelineCommit>): Set<string> {
  const set = new Set<string>();
  const stack = tipSha ? [tipSha] : [];
  while (stack.length) {
    const sha = stack.pop()!;
    if (set.has(sha)) continue;
    set.add(sha);
    const c = bySha.get(sha);
    if (c) stack.push(...c.parent_shas);
  }
  return set;
}

/** Branch map: one lane per ticket, ticket card on top, its dev branches + commits below. */
export function BranchMap() {
  const { currentSessionId, pipelineStage, tickets } = useSessionStore();
  const [commits, setCommits] = useState<TimelineCommit[]>([]);
  const [running, setRunning] = useState<Set<string>>(new Set());
  const [rounds, setRounds] = useState<Record<string, number>>({});
  const [hint, setHint] = useState<Record<string, string>>({});

  const reload = useCallback(async () => {
    if (!currentSessionId) return;
    try {
      setCommits(await api.listTimelineCommits(currentSessionId, 300));
    } catch (e) {
      console.error(e);
    }
  }, [currentSessionId]);

  useEffect(() => {
    void reload();
    let un: (() => void) | undefined;
    let cancelled = false;
    onTimelineUpdated((p) => {
      if (p.session_id === currentSessionId) void reload();
    }).then((u) => {
      if (cancelled) u();
      else un = u;
    });
    return () => {
      cancelled = true;
      un?.();
    };
  }, [reload, currentSessionId]);

  // sync running flags from pipeline (tickets store doesn't carry it — track locally)
  useEffect(() => {
    setRunning(new Set());
  }, [currentSessionId]);

  const bySha = useMemo(() => new Map(commits.map((c) => [c.sha, c])), [commits]);
  const mainTip = useMemo(
    () =>
      commits.find((c) => c.branch_tips.includes("main"))?.sha ??
      commits.find((c) => c.branch_tips.includes("master"))?.sha,
    [commits]
  );
  const mainAncestors = useMemo(() => ancestorSet(mainTip, bySha), [mainTip, bySha]);

  // ticket → [{branch, commits[]}]
  const lanes = useMemo(() => {
    const claimed = new Set<string>();
    const map = new Map<string, { branch: string; commits: TimelineCommit[] }[]>();
    for (const t of tickets) {
      const rows: { branch: string; commits: TimelineCommit[] }[] = [];
      for (const b of t.branches) {
        const tip = commits.find((c) => c.branch_tips.includes(b))?.sha;
        if (!tip) {
          rows.push({ branch: b, commits: [] });
          continue;
        }
        rows.push({ branch: b, commits: branchCommits(tip, bySha, mainAncestors, claimed) });
      }
      map.set(t.id, rows);
    }
    return map;
  }, [tickets, commits, bySha, mainAncestors]);

  if (pipelineStage !== "developing") {
    return (
      <div className="flex h-full items-center justify-center p-8 text-center text-sm text-muted-foreground">
        分支地图在确认 tickets 后开放——先完成访谈 → spec → tickets 流程。
      </div>
    );
  }

  const startRun = async (t: Ticket) => {
    if (!currentSessionId) return;
    const r = rounds[t.id] ?? 3;
    const feedback = hint[t.id]?.trim() || undefined;
    setRunning((s) => new Set(s).add(t.id));
    try {
      const branch = await api.runTicket(currentSessionId, t.id, {
        rounds: r,
        feedback,
      });
      toast.success(`已开线 ${branch}（${r} 轮迭代）`);
    } catch (e) {
      toast.error(String(e));
      setRunning((s) => {
        const n = new Set(s);
        n.delete(t.id);
        return n;
      });
    }
  };

  const setStatus = async (t: Ticket, status: string) => {
    try {
      await api.setTicketStatus(t.id, status);
    } catch (e) {
      toast.error(String(e));
    }
  };

  return (
    <div className="h-full overflow-x-auto p-4">
      <div className="flex h-full items-start gap-4">
        {tickets.map((t) => {
          const rows = lanes.get(t.id) ?? [];
          const isRunning = running.has(t.id) || t.status === "in_progress";
          return (
            <div key={t.id} className="w-64 shrink-0 space-y-3">
              {/* ticket card = lane header */}
              <div className={`rounded-lg border-2 bg-card p-3 ${STATUS_STYLE[t.status] ?? ""}`}>
                <div className="flex items-center justify-between gap-1">
                  <Badge variant="outline" className="text-[10px]">
                    {t.id}
                  </Badge>
                  <Badge variant="secondary" className="text-[10px]">
                    {STATUS_LABEL[t.status] ?? t.status}
                  </Badge>
                </div>
                <div className="mt-1.5 text-sm font-medium leading-snug">{t.title}</div>
                {t.description && (
                  <p className="mt-1 line-clamp-3 text-[11px] text-muted-foreground">
                    {t.description}
                  </p>
                )}
                {t.depends_on.length > 0 && (
                  <p className="mt-1 text-[10px] text-muted-foreground">
                    依赖：{t.depends_on.join(", ")}
                  </p>
                )}

                {t.status !== "rejected" && t.status !== "done" && (
                  <div className="mt-2 space-y-1.5">
                    <Input
                      className="h-6 text-[11px]"
                      placeholder="实现思路（可选）"
                      value={hint[t.id] ?? ""}
                      onChange={(e) =>
                        setHint((m) => ({ ...m, [t.id]: e.target.value }))
                      }
                    />
                    <div className="flex items-center gap-1">
                      <Input
                        className="h-6 w-12 text-[11px]"
                        type="number"
                        min={1}
                        max={10}
                        value={rounds[t.id] ?? 3}
                        onChange={(e) =>
                          setRounds((m) => ({
                            ...m,
                            [t.id]: Math.max(1, Math.min(10, Number(e.target.value) || 3)),
                          }))
                        }
                      />
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-6 flex-1 text-[11px]"
                        disabled={isRunning}
                        onClick={() => void startRun(t)}
                      >
                        {isRunning ? (
                          <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                        ) : (
                          <Play className="mr-1 h-3 w-3" />
                        )}
                        {t.branches.length > 0 ? "加线" : "开线"}
                      </Button>
                    </div>
                  </div>
                )}
                {t.status === "in_progress" && (
                  <div className="mt-2 flex gap-1">
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-6 flex-1 text-[11px] text-green-600"
                      onClick={() => void setStatus(t, "done")}
                    >
                      验收 ✓
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-6 flex-1 text-[11px] text-destructive"
                      onClick={() => void setStatus(t, "rejected")}
                    >
                      废弃
                    </Button>
                  </div>
                )}
              </div>

              {/* dev branches under the ticket */}
              {rows.map(({ branch, commits: cs }) => (
                <div key={branch} className="rounded-md border bg-muted/30 p-2">
                  <div className="flex items-center gap-1 text-[11px] font-medium">
                    <GitBranch className="h-3 w-3 text-primary" />
                    {branch}
                    <span className="text-muted-foreground">({cs.length})</span>
                  </div>
                  <div className="mt-1.5 space-y-0">
                    {cs.length === 0 && (
                      <p className="text-[10px] text-muted-foreground">尚无提交</p>
                    )}
                    {cs.map((c, i) => (
                      <div key={c.sha} className="relative flex gap-1.5 pb-1.5">
                        {/* vertical lane line */}
                        {i < cs.length - 1 && (
                          <span className="absolute left-[3px] top-3 h-full w-px bg-border" />
                        )}
                        <CircleDot
                          className={`mt-0.5 h-[7px] w-[7px] shrink-0 ${
                            c.is_head ? "text-primary" : "text-muted-foreground"
                          }`}
                        />
                        <div className="min-w-0">
                          <div className="truncate text-[11px] leading-tight">{c.subject}</div>
                          <div className="text-[9px] text-muted-foreground">
                            {c.short_sha}
                            {c.version != null && ` · v${c.version}`}
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          );
        })}
        {tickets.length === 0 && (
          <p className="text-sm text-muted-foreground">没有 tickets。</p>
        )}
      </div>
    </div>
  );
}
