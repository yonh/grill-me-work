import { useCallback, useEffect, useMemo, useState } from "react";
import {
  FolderOpen,
  GitBranch,
  GitCommitHorizontal,
  Loader2,
  RefreshCw,
  Split,
  Eye,
  Rocket,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Input } from "@/components/ui/input";
import {
  api,
  onPrototypeUpdated,
  onTimelineUpdated,
} from "@/lib/tauri";
import type { GitStatus, TimelineCommit } from "@/lib/types";
import { toast } from "sonner";
import { cn } from "@/lib/utils";

interface TimelineGraphProps {
  sessionId: string;
  onPreviewRefresh?: () => void;
}

/** Compact list-style decision tree: one row per commit, branch tips highlighted. */
export function TimelineGraph({ sessionId, onPreviewRefresh }: TimelineGraphProps) {
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [commits, setCommits] = useState<TimelineCommit[]>([]);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [branchName, setBranchName] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const st = await api.getTimeline(sessionId);
      setStatus(st);
      if (st.initialized) {
        const log = await api.listTimelineCommits(sessionId, 80);
        setCommits(log);
      } else {
        setCommits([]);
      }
    } catch (e) {
      console.error(e);
      setStatus({
        available: false,
        initialized: false,
        dirty: false,
        path: "",
        has_files: false,
      });
      setCommits([]);
    } finally {
      setLoading(false);
    }
  }, [sessionId]);

  useEffect(() => {
    setStatus(null);
    setCommits([]);
    setSelected(null);
    setBranchName("");
    refresh();
  }, [refresh, sessionId]);

  useEffect(() => {
    let cancelled = false;
    const offs: Array<() => void> = [];
    onTimelineUpdated((p) => {
      if (!cancelled && p.session_id === sessionId) refresh();
    }).then((u) => {
      if (cancelled) u();
      else offs.push(u);
    });
    onPrototypeUpdated((p) => {
      if (!cancelled && p.session_id === sessionId) refresh();
    }).then((u) => {
      if (cancelled) u();
      else offs.push(u);
    });
    return () => {
      cancelled = true;
      offs.forEach((u) => u());
    };
  }, [refresh, sessionId]);

  const initGit = async () => {
    setBusy(true);
    try {
      const st = await api.initTimeline(sessionId);
      setStatus(st);
      toast.success("已初始化 git 并提交当前原型");
      await refresh();
    } catch (e) {
      console.error(e);
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };

  const openDir = async () => {
    try {
      const path = await api.openPrototypeDir(sessionId);
      toast.success(`工作区：${path}`);
    } catch (e) {
      console.error(e);
      toast.error(String(e));
    }
  };

  const checkout = async (refName: string) => {
    setBusy(true);
    try {
      await api.checkoutTimeline(sessionId, refName);
      onPreviewRefresh?.();
      toast.success(`已切换到 ${refName}`);
      await refresh();
    } catch (e) {
      console.error(e);
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };

  const fork = async () => {
    const name = branchName.trim() || `timeline-${commits.length + 1}`;
    setBusy(true);
    try {
      const created = await api.forkTimeline(sessionId, name, selected ?? undefined);
      setBranchName("");
      onPreviewRefresh?.();
      toast.success(`已开分支 ${created}`);
      await refresh();
    } catch (e) {
      console.error(e);
      toast.error(String(e));
    } finally {
      setBusy(false);
    }
  };

  const selectedCommit = useMemo(
    () => commits.find((c) => c.short_sha === selected || c.sha === selected) ?? null,
    [commits, selected]
  );

  if (!status) {
    return (
      <div className="flex h-full items-center justify-center gap-2 text-xs text-muted-foreground">
        <Loader2 className="h-3.5 w-3.5 animate-spin" /> 读取时间线…
      </div>
    );
  }

  if (!status.available) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 p-6 text-center text-muted-foreground">
        <GitBranch className="h-8 w-8 opacity-40" />
        <p className="text-sm font-medium">未检测到 git</p>
        <p className="text-xs">安装 git 后，每次原型生成会自动形成决策树节点</p>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex flex-wrap items-center gap-1.5 border-b px-2 py-1.5">
        <GitBranch className="h-3.5 w-3.5 text-primary" />
        <span className="text-xs font-semibold">决策时间线</span>
        {status.branch && (
          <Badge variant="secondary" className="text-[10px]">
            {status.branch}
          </Badge>
        )}
        {status.head && (
          <span className="font-mono text-[10px] text-muted-foreground">{status.head}</span>
        )}
        <Button
          size="sm"
          variant="ghost"
          className="ml-auto h-7"
          title={status.path || "打开原型工作区"}
          onClick={openDir}
        >
          <FolderOpen className="h-3.5 w-3.5" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          className="h-7"
          onClick={refresh}
          disabled={loading}
        >
          {loading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="h-3.5 w-3.5" />}
        </Button>
      </div>

      {status.path && (
        <div className="border-b bg-muted/20 px-2 py-1 font-mono text-[10px] text-muted-foreground truncate" title={status.path}>
          {status.path}
        </div>
      )}

      {!status.initialized ? (
        <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
          <GitBranch className="h-8 w-8 text-muted-foreground opacity-40" />
          <div className="space-y-1 max-w-sm">
            <p className="text-sm font-medium">
              {status.has_files ? "原型已在磁盘，尚未纳入 git" : "尚无原型文件，也未初始化 git"}
            </p>
            <p className="text-xs text-muted-foreground">
              工作区路径：
              <span className="font-mono break-all">{status.path || "（会话原型目录）"}</span>
            </p>
            <p className="text-xs text-muted-foreground">
              {status.has_files
                ? "可立即初始化并提交当前文件，之后每次生成都会追加 commit。"
                : "下一次成功生成原型后会自动建仓并提交；也可现在空仓初始化。"}
            </p>
          </div>
          <div className="flex gap-2">
            <Button size="sm" onClick={initGit} disabled={busy}>
              <Rocket className="mr-1 h-3.5 w-3.5" />
              {status.has_files ? "初始化并提交当前原型" : "初始化空 git 仓库"}
            </Button>
            <Button size="sm" variant="outline" onClick={openDir}>
              <FolderOpen className="mr-1 h-3.5 w-3.5" />
              打开工作区
            </Button>
          </div>
        </div>
      ) : (
        <>
          <div className="border-b px-2 py-1.5">
            <div className="flex items-center gap-1.5">
              <Input
                value={branchName}
                onChange={(e) => setBranchName(e.target.value)}
                placeholder={selected ? `从 ${selected} 开新分支…` : "从 HEAD 开新分支名…"}
                className="h-7 text-xs"
              />
              <Button size="sm" variant="outline" className="h-7 shrink-0" onClick={fork} disabled={busy}>
                <Split className="mr-1 h-3 w-3" />
                分叉
              </Button>
            </div>
            {selectedCommit && (
              <div className="mt-1.5 rounded-md border bg-muted/30 p-2 text-[11px]">
                <p className="font-medium">{selectedCommit.subject}</p>
                {selectedCommit.body && (
                  <pre className="mt-1 max-h-28 overflow-auto whitespace-pre-wrap rounded bg-background/60 p-1.5 font-sans text-[10px] leading-relaxed text-muted-foreground">
                    {selectedCommit.body}
                  </pre>
                )}
                <p className="mt-0.5 font-mono text-[10px] text-muted-foreground">
                  {selectedCommit.short_sha} · {selectedCommit.author_time}
                </p>
                <div className="mt-1.5 flex gap-1">
                  <Button
                    size="sm"
                    variant="secondary"
                    className="h-6 text-[10px]"
                    disabled={busy}
                    onClick={() => checkout(selectedCommit.short_sha)}
                  >
                    <Eye className="mr-1 h-3 w-3" /> 预览此版本
                  </Button>
                  {selectedCommit.branch_tips.map((b) => (
                    <Button
                      key={b}
                      size="sm"
                      variant="outline"
                      className="h-6 text-[10px]"
                      disabled={busy || status.branch === b}
                      onClick={() => checkout(b)}
                    >
                      切到 {b}
                    </Button>
                  ))}
                </div>
              </div>
            )}
          </div>

          <ScrollArea className="min-h-0 flex-1">
            <div className="space-y-0.5 p-2">
              {commits.length === 0 && (
                <p className="px-2 py-4 text-center text-xs text-muted-foreground">
                  仓库已初始化，还没有 commit。生成一次原型即可写入历史。
                </p>
              )}
              {commits.map((c) => (
                <button
                  key={c.sha}
                  type="button"
                  onClick={() => setSelected(c.short_sha)}
                  className={cn(
                    "flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left transition-colors",
                    selected === c.short_sha ? "bg-accent" : "hover:bg-accent/50"
                  )}
                >
                  <GitCommitHorizontal
                    className={cn(
                      "mt-0.5 h-3.5 w-3.5 shrink-0",
                      c.is_head ? "text-primary" : "text-muted-foreground"
                    )}
                  />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-1">
                      <span
                        className={cn(
                          "truncate text-xs",
                          c.is_head ? "font-semibold text-foreground" : "text-foreground/90"
                        )}
                      >
                        {c.subject}
                      </span>
                      {c.branch_tips.map((b) => (
                        <Badge
                          key={b}
                          variant={b === status.branch ? "default" : "outline"}
                          className="px-1 py-0 text-[9px]"
                        >
                          {b}
                        </Badge>
                      ))}
                    </div>
                    <div className="mt-0.5 flex items-center gap-2 font-mono text-[10px] text-muted-foreground">
                      <span>{c.short_sha}</span>
                      {c.version != null && <span>v{c.version}</span>}
                      <span>{c.author_time.slice(0, 16).replace("T", " ")}</span>
                    </div>
                    {c.body && selected === c.short_sha && (
                      <p className="mt-1 whitespace-pre-wrap rounded bg-muted/40 p-1.5 text-[10px] leading-relaxed text-muted-foreground">
                        {c.body}
                      </p>
                    )}
                  </div>
                </button>
              ))}
            </div>
          </ScrollArea>
        </>
      )}
    </div>
  );
}
