import { useMemo, useState } from "react";
import {
  ListChecks,
  Loader2,
  ChevronDown,
  ChevronRight,
  Check,
  RotateCcw,
  Trash2,
  Plus,
  ArrowUp,
  ArrowDown,
  Sparkles,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Badge } from "@/components/ui/badge";
import { useSessionStore } from "@/store/sessionStore";
import { api } from "@/lib/tauri";
import { toast } from "sonner";
import type { OutlineNode } from "@/lib/types";

/** Interview outline: the user-confirmed scope contract that gates question generation. */
export function OutlinePanel() {
  const { outlineStatus, outlineNodes, questions, currentSessionId } = useSessionStore();
  const [expanded, setExpanded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [newTitle, setNewTitle] = useState("");

  // Per-node question stats
  const stats = useMemo(() => {
    const m = new Map<string, { answered: number; pending: number }>();
    for (const q of questions) {
      if (!q.outline_node_id) continue;
      const s = m.get(q.outline_node_id) ?? { answered: 0, pending: 0 };
      if (q.status === "answered") s.answered += 1;
      else if (q.status === "ready" || q.status === "stale" || q.status === "generating")
        s.pending += 1;
      m.set(q.outline_node_id, s);
    }
    return m;
  }, [questions]);

  const enabled = outlineNodes.filter((n) => n.status !== "excluded");
  const covered = enabled.filter((n) => n.status === "covered").length;
  const isDraft = outlineStatus === "draft";
  const isConfirmed = outlineStatus === "confirmed";

  const save = async (nodes: OutlineNode[]) => {
    if (!currentSessionId) return;
    setBusy(true);
    try {
      await api.saveOutline(currentSessionId, nodes);
    } catch (e) {
      console.error(e);
      toast.error("保存大纲失败");
    } finally {
      setBusy(false);
    }
  };

  const patchNode = (id: string, patch: Partial<OutlineNode>) =>
    save(outlineNodes.map((n) => (n.id === id ? { ...n, ...patch } : n)));

  const removeNode = (id: string) =>
    save(outlineNodes.filter((n) => n.id !== id));

  const moveNode = (id: string, dir: -1 | 1) => {
    const idx = outlineNodes.findIndex((n) => n.id === id);
    const j = idx + dir;
    if (idx < 0 || j < 0 || j >= outlineNodes.length) return;
    const next = [...outlineNodes];
    [next[idx], next[j]] = [next[j], next[idx]];
    save(next);
  };

  const addNode = () => {
    const title = newTitle.trim();
    if (!title || !currentSessionId) return;
    const node: OutlineNode = {
      id: `u_${crypto.randomUUID()}`,
      session_id: currentSessionId,
      title,
      status: "pending",
      display_order: outlineNodes.length,
      created_at: new Date().toISOString(),
    };
    setNewTitle("");
    save([...outlineNodes, node]);
  };

  const confirm = async () => {
    if (!currentSessionId) return;
    setBusy(true);
    try {
      await api.confirmOutline(currentSessionId);
      toast.success("大纲已确认，开始出题");
    } catch (e) {
      console.error(e);
      toast.error("确认失败");
    } finally {
      setBusy(false);
    }
  };

  const dismiss = async () => {
    if (!currentSessionId) return;
    setBusy(true);
    try {
      await api.dismissOutline(currentSessionId);
      toast.success("已进入自由访谈模式");
    } catch (e) {
      console.error(e);
      toast.error("操作失败");
    } finally {
      setBusy(false);
    }
  };

  const generate = async () => {
    if (!currentSessionId) return;
    setBusy(true);
    try {
      await api.generateOutline(currentSessionId);
    } catch (e) {
      console.error(e);
      toast.error("生成大纲失败");
      setBusy(false);
    }
    // generating → draft comes via outline_updated event; keep busy until then? release.
    setBusy(false);
  };

  const requestMore = async () => {
    if (!currentSessionId) return;
    try {
      await api.requestBatch(currentSessionId);
      toast.info("正在生成更多问题…");
    } catch (e) {
      console.error(e);
      toast.error("生成失败");
    }
  };

  // --- states without a node list ---

  if (outlineStatus === "generating") {
    return (
      <Card className="border-dashed">
        <CardContent className="flex items-center gap-2 p-4 text-sm text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" />
          <span className="flex-1">正在生成访谈大纲…</span>
          <Button
            size="sm"
            variant="ghost"
            className="h-6 px-2 text-xs"
            onClick={generate}
            disabled={busy}
          >
            重试
          </Button>
        </CardContent>
      </Card>
    );
  }

  if (outlineStatus === "none") {
    return (
      <Card>
        <CardContent className="space-y-2 p-4">
          <div className="flex items-center gap-2 text-sm font-medium">
            <ListChecks className="h-4 w-4 text-primary" />
            访谈大纲
          </div>
          <p className="text-xs text-muted-foreground">
            先生成大纲由你确认范围，之后只在大纲内出题，避免跑偏和无止境提问。
          </p>
          <div className="flex gap-2">
            <Button size="sm" onClick={generate} disabled={busy}>
              <Sparkles className="mr-1 h-3.5 w-3.5" />
              生成大纲
            </Button>
            <Button size="sm" variant="ghost" onClick={dismiss} disabled={busy}>
              直接开始出题
            </Button>
          </div>
        </CardContent>
      </Card>
    );
  }

  // --- draft / confirmed: node list ---

  const header = (
    <button
      type="button"
      className="flex w-full items-center gap-2 text-left"
      onClick={() => !isDraft && setExpanded((v) => !v)}
    >
      {isConfirmed &&
        (expanded ? (
          <ChevronDown className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
        ) : (
          <ChevronRight className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
        ))}
      <ListChecks className="h-4 w-4 shrink-0 text-primary" />
      <span className="text-sm font-medium">
        访谈大纲
        {isDraft && <span className="ml-1 text-xs text-amber-600">（待确认）</span>}
      </span>
      {isConfirmed && enabled.length > 0 && (
        <Badge variant="secondary" className="ml-auto text-[10px]">
          {covered}/{enabled.length} 已覆盖
        </Badge>
      )}
    </button>
  );

  const showList = isDraft || expanded;

  return (
    <Card className={isDraft ? "border-primary/40" : undefined}>
      <CardContent className="space-y-2 p-4">
        {header}

        {showList && (
          <div className="space-y-1">
            {outlineNodes.map((node, idx) => {
              const s = stats.get(node.id);
              const excluded = node.status === "excluded";
              return (
                <div
                  key={node.id}
                  className={`group flex items-start gap-2 rounded-md border px-2 py-1.5 ${
                    excluded ? "opacity-50" : ""
                  } ${node.status === "covered" ? "border-green-200 bg-green-50/50 dark:border-green-900 dark:bg-green-950/20" : "border-border"}`}
                >
                  <Checkbox
                    className="mt-0.5"
                    checked={!excluded}
                    disabled={busy}
                    onCheckedChange={(v) =>
                      patchNode(node.id, {
                        status: v ? "pending" : "excluded",
                      })
                    }
                  />
                  <div className="min-w-0 flex-1">
                    <Input
                      value={node.title}
                      disabled={busy}
                      onChange={(e) =>
                        // local-only title edit; persisted on blur
                        useSessionStore.setState((state) => ({
                          outlineNodes: state.outlineNodes.map((n) =>
                            n.id === node.id ? { ...n, title: e.target.value } : n
                          ),
                        }))
                      }
                      onBlur={(e) => {
                        const t = e.target.value.trim();
                        if (t && t !== node.title) patchNode(node.id, { title: t });
                      }}
                      className="h-6 border-0 bg-transparent p-0 text-sm font-medium focus-visible:ring-0"
                    />
                    {node.description && (
                      <p className="truncate text-xs text-muted-foreground" title={node.description}>
                        {node.description}
                      </p>
                    )}
                    <div className="mt-0.5 flex items-center gap-1.5 text-[10px] text-muted-foreground">
                      {node.status === "covered" && (
                        <span className="text-green-600">已覆盖</span>
                      )}
                      {node.status === "pending" && s && (s.answered > 0 || s.pending > 0) && (
                        <span>
                          已答 {s.answered} · 待答 {s.pending}
                        </span>
                      )}
                      {excluded && <span>已排除</span>}
                    </div>
                  </div>
                  <div className="flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                    {node.status === "pending" && isConfirmed && (
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-6 w-6"
                        title="标记为已覆盖"
                        disabled={busy}
                        onClick={() => patchNode(node.id, { status: "covered" })}
                      >
                        <Check className="h-3.5 w-3.5" />
                      </Button>
                    )}
                    {node.status === "covered" && isConfirmed && (
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-6 w-6"
                        title="重新打开该主题"
                        disabled={busy}
                        onClick={() => patchNode(node.id, { status: "pending" })}
                      >
                        <RotateCcw className="h-3.5 w-3.5" />
                      </Button>
                    )}
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-6 w-6"
                      title="上移"
                      disabled={busy || idx === 0}
                      onClick={() => moveNode(node.id, -1)}
                    >
                      <ArrowUp className="h-3.5 w-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-6 w-6"
                      title="下移"
                      disabled={busy || idx === outlineNodes.length - 1}
                      onClick={() => moveNode(node.id, 1)}
                    >
                      <ArrowDown className="h-3.5 w-3.5" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-6 w-6 text-destructive"
                      title="删除该主题"
                      disabled={busy}
                      onClick={() => removeNode(node.id)}
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </div>
              );
            })}

            <div className="flex gap-1.5 pt-1">
              <Input
                value={newTitle}
                onChange={(e) => setNewTitle(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && addNode()}
                placeholder="补充一个主题…"
                className="h-7 text-xs"
                disabled={busy}
              />
              <Button
                size="icon"
                variant="outline"
                className="h-7 w-7 shrink-0"
                onClick={addNode}
                disabled={busy || !newTitle.trim()}
              >
                <Plus className="h-3.5 w-3.5" />
              </Button>
            </div>
          </div>
        )}

        {isDraft && (
          <div className="flex gap-2 pt-1">
            <Button size="sm" onClick={confirm} disabled={busy || outlineNodes.length === 0}>
              确认大纲并开始
            </Button>
            <Button size="sm" variant="ghost" onClick={dismiss} disabled={busy}>
              不用大纲，直接开始
            </Button>
          </div>
        )}

        {isConfirmed && (
          <div className="pt-1">
            <Button size="sm" variant="outline" onClick={requestMore} disabled={busy}>
              再出一批问题
            </Button>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
