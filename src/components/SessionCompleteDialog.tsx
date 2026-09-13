import { useState } from "react";
import { Sparkles } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Textarea } from "@/components/ui/textarea";
import { api } from "@/lib/tauri";
import { useSessionStore } from "@/store/sessionStore";
import { useSessions } from "@/hooks/useSessions";
import { toast } from "sonner";
import type { DecisionEntry } from "@/lib/types";

interface SessionCompleteDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  suggestion: string;
  onExport: (decisions: DecisionEntry[], summary: string) => void;
}

export function SessionCompleteDialog({
  open,
  onOpenChange,
  suggestion,
  onExport,
}: SessionCompleteDialogProps) {
  const { currentSessionId } = useSessionStore();
  const { refreshSessions } = useSessions();
  const [finishing, setFinishing] = useState(false);

  const handleFinish = async () => {
    if (!currentSessionId) return;
    setFinishing(true);
    try {
      const [decisions, summary] = await api.finishSession(currentSessionId);
      await refreshSessions();
      onExport(decisions, summary);
      onOpenChange(false);
      toast.success("访谈已结束");
    } catch (err) {
      console.error(err);
      toast.error("结束失败");
    } finally {
      setFinishing(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Sparkles className="h-5 w-5 text-amber-500" />
            会话完成建议
          </DialogTitle>
          <DialogDescription>LLM 已分析你的回答并给出建议</DialogDescription>
        </DialogHeader>

        <ScrollArea className="max-h-[300px] rounded-md border p-3">
          <Textarea
            readOnly
            value={suggestion}
            className="min-h-[200px] border-0 p-0 focus-visible:ring-0"
          />
        </ScrollArea>

        <DialogFooter className="gap-2">
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            继续深挖
          </Button>
          <Button onClick={handleFinish} disabled={finishing}>
            {finishing ? "处理中..." : "确认结束"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
