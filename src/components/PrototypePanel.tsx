import { useCallback, useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Layers,
  Loader2,
  RefreshCw,
  FileCode2,
  ExternalLink,
  FolderTree,
  Terminal,
  Monitor,
  Tablet,
  Smartphone,
  Maximize2,
  Pause,
  Play,
  Workflow,
  GitBranch,
  Network,
  Map as MapIcon,
  Activity as ActivityIcon,
  Keyboard,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Textarea } from "@/components/ui/textarea";
import { api, onAgentOutput } from "@/lib/tauri";
import { useSessionStore } from "@/store/sessionStore";
import { useSettings } from "@/hooks/useSettings";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import type { PrototypeFile } from "@/lib/types";
import { IterationCanvas } from "@/components/IterationCanvas";
import { TimelineGraph } from "@/components/TimelineGraph";
import { QuestionGraph } from "@/components/QuestionGraph";
import { BranchMap } from "@/components/BranchMap";
import { ActivityPanel } from "@/components/ActivityPanel";

type DeviceId = "desktop" | "tablet" | "mobile";
type PanelTab = "preview" | "blueprint" | "timeline" | "qgraph" | "branchmap" | "activity";

const DEVICES: {
  id: DeviceId;
  label: string;
  width: number | null;
  icon: typeof Monitor;
}[] = [
  { id: "desktop", label: "自适应", width: null, icon: Maximize2 },
  { id: "tablet", label: "平板 768", width: 768, icon: Tablet },
  { id: "mobile", label: "手机 390", width: 390, icon: Smartphone },
];

interface PrototypePanelProps {
  sessionId: string;
  version: number;
}

export function PrototypePanel({ sessionId, version }: PrototypePanelProps) {
  const isGenerating = useSessionStore((s) => s.isPrototypeGenerating);
  const { settings, saveSettings } = useSettings();
  const autoPaused = settings?.prototype_auto_paused ?? false;
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [files, setFiles] = useState<PrototypeFile[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [fileContent, setFileContent] = useState("");
  const [feedback, setFeedback] = useState("");
  const [showFiles, setShowFiles] = useState(false);
  const [showLog, setShowLog] = useState(false);
  const [agentLog, setAgentLog] = useState<string[]>([]);
  const [reloadKey, setReloadKey] = useState(0);
  const [device, setDevice] = useState<DeviceId>("desktop");
  const [tab, setTab] = useState<PanelTab>("preview");
  const iframeRef = useRef<HTMLIFrameElement>(null);

  // Interactive prototypes (games) need keyboard focus inside the iframe —
  // keys typed in the outer app never reach it. Auto-focus on load and when
  // the preview tab is shown.
  const focusPreview = useCallback(() => {
    iframeRef.current?.contentWindow?.focus();
  }, []);
  useEffect(() => {
    if (tab === "preview" && previewUrl) {
      // Defer so the iframe has painted before stealing focus.
      const t = window.setTimeout(focusPreview, 50);
      return () => window.clearTimeout(t);
    }
  }, [tab, previewUrl, reloadKey, focusPreview]);

  // Forward keys into the cross-origin preview iframe: the served HTML
  // carries a __grillKey relay that re-dispatches them as KeyboardEvents.
  // This works even when the webview refuses to give the iframe real focus.
  useEffect(() => {
    if (tab !== "preview" || !previewUrl) return;
    const forward = (kind: "keydown" | "keyup") => (e: KeyboardEvent) => {
      const el = document.activeElement as HTMLElement | null;
      if (
        el &&
        (el.tagName === "INPUT" ||
          el.tagName === "TEXTAREA" ||
          el.isContentEditable)
      ) {
        return;
      }
      iframeRef.current?.contentWindow?.postMessage(
        {
          __grillKey: true,
          kind,
          init: {
            key: e.key,
            code: e.code,
            repeat: e.repeat,
            bubbles: true,
            cancelable: true,
          },
        },
        "*"
      );
    };
    const kd = forward("keydown");
    const ku = forward("keyup");
    window.addEventListener("keydown", kd);
    window.addEventListener("keyup", ku);
    return () => {
      window.removeEventListener("keydown", kd);
      window.removeEventListener("keyup", ku);
    };
  }, [tab, previewUrl]);

  const refresh = useCallback(async () => {
    try {
      const url = await api.getPrototypePreviewUrl(sessionId);
      setPreviewUrl(url);
      const list = await api.getPrototypeFiles(sessionId);
      setFiles(list);
    } catch (e) {
      console.error("refresh prototype panel", e);
    }
  }, [sessionId]);

  useEffect(() => {
    setPreviewUrl(null);
    setFiles([]);
    setSelectedFile(null);
    setFileContent("");
    setAgentLog([]);
    refresh();
  }, [sessionId, refresh]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onAgentOutput((sid, stream, text) => {
      if (cancelled || sid !== sessionId) return;
      setAgentLog((prev) => {
        const next = [...prev, `[${stream}] ${text}`];
        return next.length > 400 ? next.slice(-400) : next;
      });
    }).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [sessionId]);

  useEffect(() => {
    if (version > 0) {
      setReloadKey((k) => k + 1);
      refresh();
    }
  }, [version, refresh]);

  const openFile = async (path: string) => {
    try {
      const content = await api.readPrototypeFile(sessionId, path);
      setSelectedFile(path);
      setFileContent(content);
      setShowFiles(true);
      setTab("preview");
    } catch (e) {
      console.error(e);
      toast.error("读取文件失败");
    }
  };

  const handleGenerate = async () => {
    setShowLog(true);
    setAgentLog([]);
    setTab("preview");
    try {
      const result = await api.generatePrototype(
        sessionId,
        feedback.trim() ? feedback.trim() : undefined
      );
      setFeedback("");
      setReloadKey((k) => k + 1);
      await refresh();
      toast.success(`原型已更新到 v${result.version}（${result.file_count} 个文件）`);
    } catch (e) {
      console.error(e);
      toast.error("原型生成失败");
    }
  };

  const toggleAutoPause = async () => {
    if (!settings) return;
    try {
      await saveSettings({
        ...settings,
        prototype_auto_paused: !settings.prototype_auto_paused,
      });
      toast.success(
        settings.prototype_auto_paused ? "已恢复自动生成" : "已暂停自动生成（仍可手动生成）"
      );
    } catch (e) {
      console.error(e);
    }
  };

  const hasFiles = files.length > 0;
  const showPreviewChrome = tab === "preview";

  return (
    <div className="flex h-full w-full min-w-0 flex-col border-l bg-background">
      <div className="flex items-center justify-between border-b px-3 py-2">
        <div className="flex min-w-0 items-center gap-2">
          <Layers className="h-4 w-4 shrink-0 text-primary" />
          <span className="text-sm font-semibold">原型</span>
          {version > 0 && (
            <Badge variant="secondary" className="text-[10px]">
              v{version}
            </Badge>
          )}
          {autoPaused && !isGenerating && (
            <Badge variant="outline" className="text-[10px] text-muted-foreground">
              自动暂停
            </Badge>
          )}
          {isGenerating && (
            <Badge variant="outline" className="gap-1 text-[10px] text-amber-600">
              <Loader2 className="h-3 w-3 animate-spin" />
              Agent 编码中
            </Badge>
          )}
          <div className="ml-1 flex shrink-0 items-center rounded-md border p-0.5">
            {(
              [
                { id: "preview" as const, label: "预览", icon: Monitor },
                { id: "blueprint" as const, label: "蓝图", icon: Workflow },
                { id: "timeline" as const, label: "时间线", icon: GitBranch },
                { id: "qgraph" as const, label: "问题图", icon: Network },
                { id: "branchmap" as const, label: "地图", icon: MapIcon },
                { id: "activity" as const, label: "动态", icon: ActivityIcon },
              ]
            ).map((t) => {
              const Icon = t.icon;
              return (
                <button
                  key={t.id}
                  type="button"
                  title={t.label}
                  onClick={() => setTab(t.id)}
                  className={cn(
                    "flex h-6 items-center gap-1 rounded px-1.5 text-[11px] transition-colors",
                    tab === t.id
                      ? "bg-primary text-primary-foreground"
                      : "text-muted-foreground hover:bg-accent hover:text-foreground"
                  )}
                >
                  <Icon className="h-3 w-3" />
                  <span className="hidden lg:inline">{t.label}</span>
                </button>
              );
            })}
          </div>
        </div>
        <div className="flex items-center gap-1">
          {showPreviewChrome && (
            <Button
              variant={autoPaused ? "outline" : "ghost"}
              size="sm"
              onClick={toggleAutoPause}
              title={autoPaused ? "恢复答题后自动生成" : "暂停答题后自动生成"}
              className={autoPaused ? "text-amber-600" : ""}
            >
              {autoPaused ? (
                <>
                  <Play className="h-3.5 w-3.5" />
                  <span className="ml-1 hidden sm:inline">恢复自动</span>
                </>
              ) : (
                <>
                  <Pause className="h-3.5 w-3.5" />
                  <span className="ml-1 hidden sm:inline">暂停自动</span>
                </>
              )}
            </Button>
          )}
          {showPreviewChrome && (
            <>
              <div className="mr-1 flex items-center rounded-md border p-0.5">
                {DEVICES.map((d) => {
                  const Icon = d.icon;
                  const active = device === d.id;
                  return (
                    <button
                      key={d.id}
                      type="button"
                      title={d.label}
                      onClick={() => setDevice(d.id)}
                      className={cn(
                        "flex h-7 items-center gap-1 rounded px-2 text-[11px] transition-colors",
                        active
                          ? "bg-primary text-primary-foreground"
                          : "text-muted-foreground hover:bg-accent hover:text-foreground"
                      )}
                    >
                      <Icon className="h-3.5 w-3.5" />
                      <span className="hidden lg:inline">{d.label}</span>
                    </button>
                  );
                })}
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setShowLog((v) => !v)}
                title="Agent 日志"
              >
                <Terminal className="h-3.5 w-3.5" />
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => setShowFiles((v) => !v)}
                title="文件列表"
              >
                <FolderTree className="h-3.5 w-3.5" />
              </Button>
              {previewUrl && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => openUrl(previewUrl).catch(console.error)}
                  title="在系统浏览器中打开"
                >
                  <ExternalLink className="h-3.5 w-3.5" />
                </Button>
              )}
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {
                  setReloadKey((k) => k + 1);
                  refresh();
                }}
                title="刷新"
              >
                <RefreshCw className="h-3.5 w-3.5" />
              </Button>
            </>
          )}
        </div>
      </div>

      {tab === "blueprint" ? (
        <div className="min-h-0 flex-1">
          <IterationCanvas sessionId={sessionId} />
        </div>
      ) : tab === "timeline" ? (
        <div className="min-h-0 flex-1">
          <TimelineGraph
            sessionId={sessionId}
            onPreviewRefresh={() => {
              setReloadKey((k) => k + 1);
              refresh();
              setTab("preview");
            }}
          />
        </div>
      ) : tab === "qgraph" ? (
        <div className="min-h-0 flex-1">
          <QuestionGraph />
        </div>
      ) : tab === "branchmap" ? (
        <div className="min-h-0 flex-1">
          <BranchMap />
        </div>
      ) : tab === "activity" ? (
        <div className="min-h-0 flex-1">
          <ActivityPanel sessionId={sessionId} />
        </div>
      ) : (
        <>
          <div className="flex min-h-0 flex-1">
            {showFiles && (
              <div className="w-48 shrink-0 border-r bg-muted/20">
                <ScrollArea className="h-full">
                  <div className="space-y-0.5 p-2">
                    {files.length === 0 && (
                      <p className="px-2 py-3 text-xs text-muted-foreground">暂无文件</p>
                    )}
                    {files.map((f) => (
                      <button
                        key={f.path}
                        onClick={() => openFile(f.path)}
                        className={`flex w-full items-center gap-1.5 rounded px-2 py-1 text-left text-xs hover:bg-accent ${
                          selectedFile === f.path ? "bg-accent font-medium" : ""
                        }`}
                      >
                        <FileCode2 className="h-3 w-3 shrink-0 opacity-60" />
                        <span className="truncate">{f.path}</span>
                      </button>
                    ))}
                  </div>
                </ScrollArea>
              </div>
            )}

            <div className="flex min-w-0 flex-1 flex-col">
              {showLog ? (
                <div className="flex min-h-0 flex-1 flex-col">
                  <div className="flex items-center justify-between border-b px-3 py-1 text-xs text-muted-foreground">
                    <span>Agent 输出</span>
                    <button className="hover:text-foreground" onClick={() => setShowLog(false)}>
                      关闭
                    </button>
                  </div>
                  <ScrollArea className="flex-1">
                    <div className="whitespace-pre-wrap p-3 font-mono text-[11px] leading-relaxed">
                      {agentLog.length === 0 ? "等待 agent 输出…" : agentLog.join("\n")}
                    </div>
                  </ScrollArea>
                </div>
              ) : selectedFile && showFiles ? (
                <div className="flex min-h-0 flex-1 flex-col">
                  <div className="border-b px-3 py-1 text-xs text-muted-foreground">
                    {selectedFile}
                  </div>
                  <ScrollArea className="flex-1">
                    <pre className="break-all whitespace-pre-wrap p-3 font-mono text-xs">
                      {fileContent}
                    </pre>
                  </ScrollArea>
                </div>
              ) : hasFiles && previewUrl ? (
                <div className="relative flex min-h-0 flex-1 items-stretch justify-center overflow-auto bg-muted/40 p-3">
                  <div className="pointer-events-none absolute left-1/2 top-4 z-10 flex -translate-x-1/2 items-center gap-1 rounded-full bg-background/80 px-2 py-0.5 text-[10px] text-muted-foreground shadow-sm">
                    <Keyboard className="h-3 w-3" />
                    可交互原型：点击画面后键盘操作
                  </div>
                  <div
                    className={cn(
                      "h-full overflow-hidden rounded-lg border bg-white shadow-sm",
                      device === "desktop" ? "w-full" : "shrink-0"
                    )}
                    style={
                      device === "desktop"
                        ? undefined
                        : {
                            width: DEVICES.find((d) => d.id === device)?.width ?? undefined,
                          }
                    }
                  >
                    <iframe
                      ref={iframeRef}
                      key={`${reloadKey}-${device}`}
                      src={`${previewUrl}/?v=${reloadKey}&d=${device}`}
                      className="h-full w-full border-0 bg-white"
                      title="原型预览"
                      sandbox="allow-scripts allow-same-origin"
                      onLoad={focusPreview}
                    />
                  </div>
                </div>
              ) : (
                <div className="flex h-full flex-col items-center justify-center gap-3 text-center text-muted-foreground">
                  <Layers className="h-12 w-12 opacity-40" />
                  <div className="max-w-sm space-y-1">
                    <p className="text-sm font-medium">答满 3 题后，coding agent 会生成第一版原型</p>
                    <p className="text-xs">
                      之后每答一题自动增量更新；也可用「迭代蓝图」自动跑多轮，或在下方写修改意见手动生成
                    </p>
                  </div>
                </div>
              )}
            </div>
          </div>

          <div className="border-t p-3">
            <div className="flex items-end gap-2">
              <Textarea
                value={feedback}
                onChange={(e) => setFeedback(e.target.value)}
                placeholder="修改意见，例如：侧栏改成深色、增加筛选…"
                className="min-h-[40px] resize-none text-sm"
                rows={2}
              />
              <Button
                size="sm"
                onClick={handleGenerate}
                disabled={isGenerating}
                className="shrink-0"
              >
                {isGenerating ? (
                  <>
                    <Loader2 className="mr-1 h-3.5 w-3.5 animate-spin" />
                    编码中
                  </>
                ) : (
                  <>
                    <Layers className="mr-1 h-3.5 w-3.5" />
                    {version > 0 ? `生成 v${version + 1}` : "生成原型"}
                  </>
                )}
              </Button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
