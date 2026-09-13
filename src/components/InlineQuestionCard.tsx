import { useState } from "react";
import {
  ChevronDown,
  ChevronRight,
  Check,
  SkipForward,
  Loader2,
  PenLine,
  X,
} from "lucide-react";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Checkbox } from "@/components/ui/checkbox";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { api } from "@/lib/tauri";
import { useSessionStore } from "@/store/sessionStore";
import { toast } from "sonner";
import type { Question, AnswerValue } from "@/lib/types";

interface InlineQuestionCardProps {
  question: Question;
}

export function InlineQuestionCard({ question }: InlineQuestionCardProps) {
  const { currentSessionId, updateQuestion } = useSessionStore();
  const [expanded, setExpanded] = useState(false);
  const [multiSelected, setMultiSelected] = useState<string[]>([]);
  const [openText, setOpenText] = useState("");
  const [customMode, setCustomMode] = useState(false);
  const [customText, setCustomText] = useState("");

  const sessionId = currentSessionId ?? question.session_id;
  const isAnswered = question.status === "answered";
  const isSkipped = question.status === "skipped";
  const isReady = question.status === "ready";

  const answerText = (() => {
    if (!question.answer) return "";
    if (question.answer.kind === "choice") return question.answer.option;
    if (question.answer.kind === "multi") return question.answer.options.join("、");
    return question.answer.text;
  })();

  const submitAnswer = async (answer: AnswerValue) => {
    try {
      await api.answerQuestion(sessionId, question.id, answer);
      updateQuestion(question.id, { status: "answered", answer });
      toast.success("已回答");
    } catch (err) {
      console.error(err);
      toast.error("提交失败");
    }
  };

  const handleChoice = async (option: string) => {
    await submitAnswer({ kind: "choice", option });
  };

  const handleMultiConfirm = async () => {
    if (multiSelected.length === 0 && !customText.trim()) {
      toast.warning("请至少选择一项，或填写自定义回复");
      return;
    }
    // Custom text wins as open answer if provided; otherwise multi choice
    if (customText.trim()) {
      await submitAnswer({ kind: "open", text: customText.trim() });
      return;
    }
    await submitAnswer({ kind: "multi", options: multiSelected });
  };

  const handleOpenSubmit = async () => {
    if (!openText.trim()) {
      toast.warning("请输入回答");
      return;
    }
    await submitAnswer({ kind: "open", text: openText.trim() });
  };

  const handleCustomSubmit = async () => {
    if (!customText.trim()) {
      toast.warning("请填写自定义回复");
      return;
    }
    await submitAnswer({ kind: "open", text: customText.trim() });
  };

  const handleSkip = async () => {
    try {
      await api.skipQuestion(sessionId, question.id);
      updateQuestion(question.id, { status: "skipped" });
      setExpanded(false);
      toast.success("已跳过");
    } catch (err) {
      console.error(err);
      toast.error("操作失败");
    }
  };

  // Collapsed: answered
  if (isAnswered) {
    return (
      <Card className="border-green-300 bg-green-50/50 dark:border-green-800 dark:bg-green-950/20">
        <CardContent className="flex items-start gap-2 p-3">
          <Check className="mt-0.5 h-4 w-4 shrink-0 text-green-600" />
          <div className="min-w-0 flex-1 space-y-0.5">
            <p className="text-sm font-medium leading-snug break-words">{question.question}</p>
            <p className="text-xs text-muted-foreground leading-snug break-words">{answerText}</p>
          </div>
        </CardContent>
      </Card>
    );
  }

  // Collapsed: skipped
  if (isSkipped) {
    return (
      <Card className="opacity-60">
        <CardContent className="flex items-start gap-2 p-3">
          <SkipForward className="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" />
          <p className="flex-1 text-sm text-muted-foreground leading-snug break-words">{question.question} (已跳过)</p>
        </CardContent>
      </Card>
    );
  }

  // Collapsed: ready (not expanded)
  if (isReady && !expanded) {
    return (
      <Card
        className="cursor-pointer border-blue-300 hover:border-blue-400 dark:border-blue-800"
        onClick={() => setExpanded(true)}
      >
        <CardContent className="flex items-start gap-2 p-3">
          <ChevronRight className="mt-0.5 h-4 w-4 shrink-0 text-blue-500" />
          <div className="min-w-0 flex-1 space-y-0.5">
            <p className="text-sm font-medium leading-snug break-words">{question.question}</p>
            {question.rationale && (
              <p className="text-xs text-muted-foreground leading-snug break-words">{question.rationale}</p>
            )}
          </div>
          <Badge variant="outline" className="mt-0.5 shrink-0 text-[10px]">
            {question.q_type === "choice" ? "单选" : question.q_type === "multi" ? "多选" : "文本"}
          </Badge>
        </CardContent>
      </Card>
    );
  }

  const showOptions = question.options.length > 0 && question.q_type !== "open";

  // Expanded: ready
  return (
    <Card className="border-blue-300 dark:border-blue-800">
      <CardContent className="space-y-3 p-4">
        <div className="flex items-start gap-2">
          <button
            onClick={() => setExpanded(false)}
            className="mt-0.5 shrink-0 text-muted-foreground hover:text-foreground"
          >
            <ChevronDown className="h-4 w-4" />
          </button>
          <div className="flex-1 space-y-1">
            <div className="flex items-center gap-2">
              <Badge variant="outline" className="text-[10px]">
                {question.q_type === "choice" ? "单选" : question.q_type === "multi" ? "多选" : "文本"}
              </Badge>
            </div>
            <p className="text-sm font-semibold leading-snug">{question.question}</p>
            {question.rationale && (
              <p className="text-xs text-muted-foreground">{question.rationale}</p>
            )}
          </div>
        </div>

        {question.options.length === 0 && question.q_type !== "open" && (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            正在生成选项...
          </div>
        )}

        {/* Choice type */}
        {question.q_type === "choice" && showOptions && !customMode && (
          <RadioGroup onValueChange={handleChoice} className="space-y-1">
            {question.options.map((opt) => (
              <div
                key={opt.label}
                className="flex items-start gap-3 rounded-md border p-2 cursor-pointer hover:bg-accent"
                onClick={() => handleChoice(opt.label)}
              >
                <RadioGroupItem value={opt.label} className="mt-1" id={`inline-${question.id}-${opt.label}`} />
                <div className="flex-1 space-y-0.5">
                  <Label htmlFor={`inline-${question.id}-${opt.label}`} className="cursor-pointer text-sm font-medium">
                    {opt.label}
                  </Label>
                  {opt.description && (
                    <p className="text-xs text-muted-foreground">{opt.description}</p>
                  )}
                </div>
              </div>
            ))}
          </RadioGroup>
        )}

        {/* Multi type */}
        {question.q_type === "multi" && showOptions && !customMode && (
          <div className="space-y-1">
            {question.options.map((opt) => {
              const checked = multiSelected.includes(opt.label);
              return (
                <div
                  key={opt.label}
                  className="flex items-start gap-3 rounded-md border p-2 cursor-pointer hover:bg-accent"
                  onClick={() => {
                    if (checked) {
                      setMultiSelected((s) => s.filter((x) => x !== opt.label));
                    } else {
                      setMultiSelected((s) => [...s, opt.label]);
                    }
                  }}
                >
                  <Checkbox
                    id={`inline-${question.id}-${opt.label}`}
                    checked={checked}
                    onCheckedChange={(v) => {
                      if (v) {
                        setMultiSelected((s) => [...s, opt.label]);
                      } else {
                        setMultiSelected((s) => s.filter((x) => x !== opt.label));
                      }
                    }}
                    className="mt-1"
                  />
                  <div className="flex-1 space-y-0.5">
                    <Label htmlFor={`inline-${question.id}-${opt.label}`} className="cursor-pointer text-sm font-medium">
                      {opt.label}
                    </Label>
                    {opt.description && (
                      <p className="text-xs text-muted-foreground">{opt.description}</p>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {/* Custom reply toggle for choice/multi */}
        {showOptions && !customMode && (
          <Button
            variant="outline"
            size="sm"
            className="text-muted-foreground"
            onClick={() => setCustomMode(true)}
          >
            <PenLine className="mr-1 h-3.5 w-3.5" />
            以上都不合适？自定义回复
          </Button>
        )}

        {/* Custom reply box */}
        {customMode && (
          <div className="space-y-2 rounded-md border border-dashed p-3">
            <div className="flex items-center justify-between">
              <span className="text-xs font-medium text-muted-foreground">自定义回复</span>
              <Button
                variant="ghost"
                size="sm"
                className="h-6 px-2 text-muted-foreground"
                onClick={() => {
                  setCustomMode(false);
                  setCustomText("");
                }}
              >
                <X className="h-3.5 w-3.5" />
                返回选项
              </Button>
            </div>
            <Textarea
              value={customText}
              onChange={(e) => setCustomText(e.target.value)}
              placeholder="写下你真正想要的答案…（会覆盖预设选项）"
              className="min-h-[72px] text-sm"
              autoFocus
            />
            <Button size="sm" onClick={handleCustomSubmit}>
              <Check className="mr-1 h-4 w-4" />
              提交自定义
            </Button>
          </div>
        )}

        {/* Multi confirm */}
        {question.q_type === "multi" && showOptions && !customMode && (
          <Button onClick={handleMultiConfirm} size="sm">
            <Check className="mr-1 h-4 w-4" />
            确认{multiSelected.length > 0 ? ` (${multiSelected.length})` : ""}
          </Button>
        )}

        {/* Open type */}
        {question.q_type === "open" && (
          <>
            <Textarea
              value={openText}
              onChange={(e) => setOpenText(e.target.value)}
              placeholder="请输入你的回答..."
              className="min-h-[80px]"
            />
            <Button onClick={handleOpenSubmit} size="sm">
              提交
            </Button>
          </>
        )}

        <Button variant="ghost" size="sm" onClick={handleSkip} className="text-muted-foreground">
          <SkipForward className="mr-1 h-3.5 w-3.5" />
          跳过
        </Button>
      </CardContent>
    </Card>
  );
}
