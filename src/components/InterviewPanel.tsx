import { useState } from "react";
import { CheckCircle2, Clock, SkipForward, CheckCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Badge } from "@/components/ui/badge";
import { useSessionStore } from "@/store/sessionStore";
import { InlineQuestionCard } from "./InlineQuestionCard";
import { OutlinePanel } from "./OutlinePanel";
import { api } from "@/lib/tauri";
import { toast } from "sonner";

interface InterviewPanelProps {
  mayComplete: boolean;
}

/** Structured interview only — no free-form chat. */
export function InterviewPanel({ mayComplete }: InterviewPanelProps) {
  const { questions, currentSessionId } = useSessionStore();
  const [finishing, setFinishing] = useState(false);

  const pending = questions.filter((q) => q.status === "ready" || q.status === "stale");
  const answered = questions.filter((q) => q.status === "answered");
  const skipped = questions.filter((q) => q.status === "skipped");

  const handleFinish = async () => {
    if (!currentSessionId) return;
    setFinishing(true);
    try {
      await api.finishSession(currentSessionId);
      toast.success("访谈已完成");
    } catch (err) {
      console.error(err);
      toast.error("完成访谈失败");
    } finally {
      setFinishing(false);
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b px-4 py-2">
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Clock className="h-3.5 w-3.5" />
          <span>
            待答 <strong className="text-foreground">{pending.length}</strong>
          </span>
          <span className="opacity-40">·</span>
          <CheckCircle className="h-3.5 w-3.5 text-green-600" />
          <span>
            已答 <strong className="text-foreground">{answered.length}</strong>
          </span>
          {mayComplete && (
            <Badge variant="outline" className="ml-1 gap-1 text-[10px] text-green-600">
              <CheckCircle2 className="h-3 w-3" />
              建议结束
            </Badge>
          )}
        </div>
        <Button onClick={handleFinish} disabled={finishing} size="sm">
          {finishing ? "处理中..." : "完成访谈"}
        </Button>
      </div>

      <ScrollArea className="flex-1">
        <div className="mx-auto max-w-2xl space-y-4 p-4">
          <OutlinePanel />
          <Section title="待答" icon={<Clock className="h-3.5 w-3.5 text-blue-500" />}>
            {pending.length === 0 ? (
              <EmptyHint text="没有待答问题。答题后右侧原型会由 agent 自动更新。" />
            ) : (
              <div className="space-y-2">
                {pending.map((q) => (
                  <InlineQuestionCard key={q.id} question={q} />
                ))}
              </div>
            )}
          </Section>

          {answered.length > 0 && (
            <Section title="已答" icon={<CheckCircle2 className="h-3.5 w-3.5 text-green-500" />}>
              <div className="space-y-2">
                {answered.map((q) => (
                  <InlineQuestionCard key={q.id} question={q} />
                ))}
              </div>
            </Section>
          )}

          {skipped.length > 0 && (
            <Section title="跳过" icon={<SkipForward className="h-3.5 w-3.5 text-muted-foreground" />}>
              <div className="space-y-2">
                {skipped.map((q) => (
                  <InlineQuestionCard key={q.id} question={q} />
                ))}
              </div>
            </Section>
          )}
        </div>
      </ScrollArea>
    </div>
  );
}

function Section({
  title,
  icon,
  children,
}: {
  title: string;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div>
      <div className="mb-2 flex items-center gap-1.5">
        {icon}
        <span className="text-sm font-medium">{title}</span>
      </div>
      {children}
    </div>
  );
}

function EmptyHint({ text }: { text: string }) {
  return <p className="py-2 text-xs text-muted-foreground">{text}</p>;
}
