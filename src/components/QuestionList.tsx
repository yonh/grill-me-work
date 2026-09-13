import { useState } from "react";
import { Loader2, Sparkles, CheckCircle2 } from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { QuestionCard } from "@/components/QuestionCard";
import { ExportDialog } from "@/components/ExportDialog";
import { api } from "@/lib/tauri";
import { useSessionStore } from "@/store/sessionStore";
import { toast } from "sonner";
import type { DecisionEntry } from "@/lib/types";

interface QuestionListProps {
  mayComplete?: boolean;
}

export function QuestionList({ mayComplete }: QuestionListProps) {
  const { questions, isGenerating, currentSessionId } = useSessionStore();
  const [exportOpen, setExportOpen] = useState(false);
  const [decisions, setDecisions] = useState<DecisionEntry[]>([]);
  const [finishing, setFinishing] = useState(false);

  const sorted = [...questions].sort((a, b) => a.display_order - b.display_order);

  const handleFinish = async () => {
    if (!currentSessionId) return;
    setFinishing(true);
    try {
      const [decisionsResult] = await api.finishSession(currentSessionId);
      setDecisions(decisionsResult);
      setExportOpen(true);
      toast.success("访谈已完成");
    } catch (err) {
      console.error(err);
      toast.error("完成访谈失败");
    } finally {
      setFinishing(false);
    }
  };

  const isEmpty = sorted.length === 0 && !isGenerating;

  return (
    <div className="flex h-full flex-col">
      <ScrollArea className="flex-1">
        <div className="mx-auto max-w-2xl space-y-4 p-6">
          {mayComplete && (
            <Card className="border-green-300 bg-green-50 dark:border-green-800 dark:bg-green-950/30">
              <CardContent className="flex items-center gap-3 p-4">
                <CheckCircle2 className="h-5 w-5 shrink-0 text-green-600" />
                <div className="flex-1 text-sm">
                  <p className="font-medium text-green-900 dark:text-green-200">
                    AI 认为需求已充分讨论
                  </p>
                  <p className="text-green-700 dark:text-green-400">
                    可以点击「完成访谈」结束，或继续回答剩余问题
                  </p>
                </div>
                <Button onClick={handleFinish} disabled={finishing} size="sm">
                  {finishing ? "处理中..." : "完成访谈"}
                </Button>
              </CardContent>
            </Card>
          )}

          {isEmpty && (
            <div className="flex flex-col items-center justify-center gap-2 py-20 text-center text-muted-foreground">
              <Sparkles className="h-8 w-8" />
              <p>等待生成问题...</p>
            </div>
          )}

          {sorted.length === 0 && isGenerating && (
            <Card className="animate-pulse">
              <CardContent className="space-y-2 p-6">
                <div className="h-4 w-24 rounded bg-muted" />
                <div className="h-5 w-3/4 rounded bg-muted" />
                <div className="h-10 w-full rounded bg-muted" />
                <div className="h-10 w-full rounded bg-muted" />
              </CardContent>
            </Card>
          )}

          {sorted.map((q) => (
            <QuestionCard key={q.id} question={q} />
          ))}

          {isGenerating && sorted.length > 0 && (
            <Card className="border-dashed">
              <CardContent className="flex items-center justify-center gap-2 p-6 text-sm text-muted-foreground">
                <Loader2 className="h-4 w-4 animate-spin" />
                等待新题...
              </CardContent>
            </Card>
          )}

          {sorted.length > 0 && (
            <div className="flex justify-center pt-4">
              <Button onClick={handleFinish} disabled={finishing} size="lg">
                {finishing ? "处理中..." : "完成访谈"}
              </Button>
            </div>
          )}
        </div>
      </ScrollArea>
      <ExportDialog
        open={exportOpen}
        onOpenChange={setExportOpen}
        decisions={decisions}
      />
    </div>
  );
}
