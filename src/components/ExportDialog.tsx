import { useState } from "react";
import { Download } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { ScrollArea } from "@/components/ui/scroll-area";
import { api } from "@/lib/tauri";
import { useSessionStore } from "@/store/sessionStore";
import { toast } from "sonner";
import type { DecisionEntry, QuestionCategory } from "@/lib/types";

interface ExportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  decisions: DecisionEntry[];
}

const categoryLabel: Record<QuestionCategory, string> = {
  intent: "意图",
  choice: "选择",
  open: "开放",
  tradeoff: "权衡",
  dependency: "依赖",
};

export function ExportDialog({ open, onOpenChange, decisions }: ExportDialogProps) {
  const { currentSessionId } = useSessionStore();
  const [content, setContent] = useState("");
  const [format, setFormat] = useState<"md" | "json">("md");

  const doExport = async (fmt: "md" | "json") => {
    if (!currentSessionId) return;
    setFormat(fmt);
    try {
      const result = await api.exportSession(currentSessionId, fmt);
      setContent(result);
      toast.success(`已导出 ${fmt.toUpperCase()}`);
    } catch (err) {
      console.error(err);
      toast.error("导出失败");
    }
  };

  const handleDownload = () => {
    if (!content) return;
    const blob = new Blob([content], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `session-${currentSessionId}.${format}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>访谈总结</DialogTitle>
          <DialogDescription>共记录 {decisions.length} 个决策</DialogDescription>
        </DialogHeader>

        <ScrollArea className="max-h-[300px] rounded-md border p-3">
          <div className="space-y-3">
            {decisions.length === 0 && (
              <p className="text-sm text-muted-foreground">暂无决策记录</p>
            )}
            {decisions.map((d, i) => (
              <div key={d.question_id} className="space-y-1 text-sm">
                <div className="flex items-center gap-2">
                  <span className="text-xs text-muted-foreground">#{i + 1}</span>
                  <span className="font-medium">{d.question}</span>
                  <span className="ml-auto rounded bg-secondary px-1.5 py-0.5 text-[10px]">
                    {categoryLabel[d.category]}
                  </span>
                </div>
                <p className="pl-4 text-muted-foreground">→ {d.answer}</p>
                {d.rationale && (
                  <p className="pl-4 text-xs text-muted-foreground">理由：{d.rationale}</p>
                )}
              </div>
            ))}
          </div>
        </ScrollArea>

        {content && (
          <Textarea
            readOnly
            value={content}
            className="min-h-[120px] font-mono text-xs"
          />
        )}

        <DialogFooter className="gap-2">
          <Button variant="outline" onClick={() => doExport("md")}>
            导出 Markdown
          </Button>
          <Button variant="outline" onClick={() => doExport("json")}>
            导出 JSON
          </Button>
          {content && (
            <Button onClick={handleDownload}>
              <Download className="mr-1 h-4 w-4" />
              下载
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
