import { useState, useEffect, useCallback, useRef } from "react";
import { Toaster } from "sonner";
import { Flame } from "lucide-react";
import { Sidebar } from "@/components/Sidebar";
import { InterviewPanel } from "@/components/InterviewPanel";
import { PrototypePanel } from "@/components/PrototypePanel";
import { SettingsDialog } from "@/components/SettingsDialog";
import { SessionCompleteDialog } from "@/components/SessionCompleteDialog";
import { ExportDialog } from "@/components/ExportDialog";
import { useSessions } from "@/hooks/useSessions";
import { useSessionEvents } from "@/hooks/useSessionEvents";
import { useSessionStore } from "@/store/sessionStore";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { DecisionEntry } from "@/lib/types";

const INTERVIEW_WIDTH_KEY = "grill-me-v2:interview-width";
const INTERVIEW_MIN = 260;
const INTERVIEW_MAX = 440;

function loadInterviewWidth(): number {
  const raw = localStorage.getItem(INTERVIEW_WIDTH_KEY);
  const n = raw ? parseInt(raw, 10) : NaN;
  if (Number.isFinite(n)) {
    return Math.min(INTERVIEW_MAX, Math.max(INTERVIEW_MIN, n));
  }
  // Interview is content-light; prototype gets the rest
  return 320;
}

export default function App() {
  const { sessions, currentSessionId } = useSessions();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [completeOpen, setCompleteOpen] = useState(false);
  const [suggestion, setSuggestion] = useState("");
  const [exportOpen, setExportOpen] = useState(false);
  const [decisions, setDecisions] = useState<DecisionEntry[]>([]);
  const [mayComplete, setMayComplete] = useState(false);
  const [protoVersion, setProtoVersion] = useState(0);
  const [interviewWidth, setInterviewWidth] = useState(loadInterviewWidth);
  const [dragging, setDragging] = useState(false);
  const layoutRef = useRef<HTMLDivElement>(null);

  useSessionEvents(
    (info) => {
      setSuggestion(info.suggestion);
      setCompleteOpen(true);
    },
    (_sessionId) => {
      setMayComplete(true);
    },
    (payload) => {
      if (payload.session_id === currentSessionId) {
        setProtoVersion(payload.version);
      }
    }
  );

  const currentSession = sessions.find((s) => s.id === currentSessionId) ?? null;
  const isGenerating = useSessionStore((s) => s.isGenerating);
  const isPrototypeGenerating = useSessionStore((s) => s.isPrototypeGenerating);
  const isCompleted = currentSession?.status === "completed";

  useEffect(() => {
    setProtoVersion(currentSession?.prototype_version ?? 0);
    setMayComplete(false);
  }, [currentSessionId, currentSession?.prototype_version]);

  const onDividerPointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault();
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    setDragging(true);
  }, []);

  const onDividerPointerMove = useCallback((e: React.PointerEvent) => {
    if (!dragging || !layoutRef.current) return;
    const rect = layoutRef.current.getBoundingClientRect();
    // Width of left interview panel
    const width = Math.round(e.clientX - rect.left);
    setInterviewWidth(Math.min(INTERVIEW_MAX, Math.max(INTERVIEW_MIN, width)));
  }, [dragging]);

  const onDividerPointerUp = useCallback(() => {
    if (!dragging) return;
    setDragging(false);
    setInterviewWidth((w) => {
      localStorage.setItem(INTERVIEW_WIDTH_KEY, String(w));
      return w;
    });
  }, [dragging]);

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background">
      <Sidebar onOpenSettings={() => setSettingsOpen(true)} />

      {currentSession ? (
        <div
          ref={layoutRef}
          className="flex min-w-0 flex-1 overflow-hidden"
          style={{ cursor: dragging ? "col-resize" : undefined }}
        >
          {/* Interview column — narrow, content-light */}
          <main
            className="flex shrink-0 flex-col overflow-hidden"
            style={{ width: interviewWidth }}
          >
            <header className="flex items-center justify-between border-b px-3 py-2.5">
              <div className="flex min-w-0 items-center gap-2">
                <h1 className="truncate text-sm font-semibold">{currentSession.title}</h1>
                <Badge variant={isCompleted ? "secondary" : "default"} className="shrink-0 text-[10px]">
                  {isCompleted ? "完成" : "进行中"}
                </Badge>
              </div>
              {(isGenerating || isPrototypeGenerating) && (
                <Badge variant="outline" className="ml-1 shrink-0 text-[10px] text-amber-600">
                  {isPrototypeGenerating ? "原型中" : "生成中"}
                </Badge>
              )}
            </header>
            <div className="min-h-0 flex-1">
              <InterviewPanel mayComplete={mayComplete} />
            </div>
          </main>

          {/* Drag handle */}
          <div
            role="separator"
            aria-orientation="vertical"
            aria-label="调整访谈栏宽度"
            className={cn(
              "relative w-1 shrink-0 cursor-col-resize touch-none select-none bg-border/60 transition-colors",
              "hover:bg-primary/40 active:bg-primary/60",
              dragging && "bg-primary/60"
            )}
            onPointerDown={onDividerPointerDown}
            onPointerMove={onDividerPointerMove}
            onPointerUp={onDividerPointerUp}
            onPointerCancel={onDividerPointerUp}
          >
            <div className="absolute inset-y-0 -left-1 -right-1" />
          </div>

          {/* Prototype column — primary workspace */}
          <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
            <PrototypePanel sessionId={currentSession.id} version={protoVersion} />
          </div>
        </div>
      ) : (
        <div className="flex flex-1 flex-col items-center justify-center gap-4 text-center">
          <Flame className="h-12 w-12 text-orange-500" />
          <div className="space-y-1">
            <h1 className="text-2xl font-bold">Grill-Me V2</h1>
            <p className="text-muted-foreground">面向原型的需求访谈：答一题，原型跟着变</p>
          </div>
          <p className="text-sm text-muted-foreground">请在左侧点击「新建会话」开始</p>
        </div>
      )}

      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
      <SessionCompleteDialog
        open={completeOpen}
        onOpenChange={setCompleteOpen}
        suggestion={suggestion}
        onExport={(d, s) => {
          setDecisions(d);
          setSuggestion(s);
          setExportOpen(true);
        }}
      />
      <ExportDialog
        open={exportOpen}
        onOpenChange={setExportOpen}
        decisions={decisions}
      />
      <Toaster richColors position="bottom-right" />
    </div>
  );
}
