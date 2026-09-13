import {
  Bot,
  ListChecks,
  RefreshCw,
  MessageSquare,
  GitBranch,
  Sparkles,
  Activity as ActivityIcon,
} from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useSessionStore } from "@/store/sessionStore";
import { cn } from "@/lib/utils";
import type { Activity } from "@/lib/types";

const KIND_META: Record<string, { icon: typeof Bot; label: string; cls: string }> = {
  mcp: { icon: Bot, label: "MCP", cls: "text-violet-500" },
  pipeline: { icon: ListChecks, label: "流水线", cls: "text-blue-500" },
  round: { icon: RefreshCw, label: "轮次", cls: "text-teal-500" },
  interview: { icon: MessageSquare, label: "访谈", cls: "text-green-600" },
  agent: { icon: GitBranch, label: "开发", cls: "text-orange-500" },
  llm: { icon: Sparkles, label: "LLM", cls: "text-purple-400" },
};

const LEVEL_DOT: Record<string, string> = {
  ok: "bg-green-500",
  warn: "bg-amber-500",
  err: "bg-red-500",
  info: "bg-muted-foreground/40",
};

function fmtTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso.slice(11, 19);
  return d.toLocaleTimeString("zh-CN", { hour12: false });
}

function fmtDay(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return `${d.getMonth() + 1}/${d.getDate()}`;
}

/** 操作动态：谁在什么时间对当前项目触发了什么（MCP 调用、流水线流转、
 *  轮次生命周期、agent 开发线、答题/跳题）。数据持久化在后端，事件实时推送。 */
export function ActivityPanel({ sessionId }: { sessionId: string }) {
  const all = useSessionStore((s) => s.activities);
  const activities = all.filter((a) => a.session_id === sessionId);

  return (
    <ScrollArea className="h-full">
      <div className="p-2">
        {activities.length === 0 && (
          <div className="flex flex-col items-center gap-2 py-10 text-muted-foreground">
            <ActivityIcon className="h-5 w-5" />
            <p className="text-xs">
              暂无动态。外部 agent 的 MCP 操作与流水线事件会实时出现在这里。
            </p>
          </div>
        )}
        <div className="space-y-px">
          {activities.map((a: Activity, i: number) => {
            const meta = KIND_META[a.kind] ?? KIND_META.pipeline;
            const Icon = meta.icon;
            const day = fmtDay(a.created_at);
            const prev = activities[i - 1];
            const showDay = !prev || fmtDay(prev.created_at) !== day;
            return (
              <div key={a.id}>
                {showDay && (
                  <div className="px-2 pb-1 pt-2 text-[10px] font-medium text-muted-foreground">
                    {day}
                  </div>
                )}
                <div className="flex items-start gap-2 rounded px-2 py-1.5 hover:bg-accent/50">
                  <span
                    className={cn(
                      "mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full",
                      LEVEL_DOT[a.level] ?? LEVEL_DOT.info
                    )}
                  />
                  <Icon className={cn("mt-0.5 h-3.5 w-3.5 shrink-0", meta.cls)} />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-baseline gap-2">
                      <span className="truncate text-xs leading-5">{a.label}</span>
                      <span className="ml-auto shrink-0 text-[10px] tabular-nums text-muted-foreground">
                        {fmtTime(a.created_at)}
                      </span>
                    </div>
                    {a.detail && (
                      <p className="truncate text-[10px] text-muted-foreground" title={a.detail}>
                        {a.detail}
                      </p>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </ScrollArea>
  );
}
