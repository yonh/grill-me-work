import { useState } from "react";
import { motion } from "framer-motion";
import { Check, Loader2, Pencil, RotateCcw, SkipForward, Sparkles } from "lucide-react";
import {
  Card,
  CardContent,
  CardHeader,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Checkbox } from "@/components/ui/checkbox";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import { api } from "@/lib/tauri";
import { useSessionStore } from "@/store/sessionStore";
import { toast } from "sonner";
import type { Question, QuestionCategory, QuestionType, AnswerValue } from "@/lib/types";

const categoryVariant: Record<QuestionCategory, string> = {
  intent: "bg-blue-500 text-white border-transparent hover:bg-blue-500/80",
  choice: "bg-cyan-500 text-white border-transparent hover:bg-cyan-500/80",
  open: "bg-gray-500 text-white border-transparent hover:bg-gray-500/80",
  tradeoff: "bg-orange-500 text-white border-transparent hover:bg-orange-500/80",
  dependency: "bg-purple-500 text-white border-transparent hover:bg-purple-500/80",
};

const categoryLabel: Record<QuestionCategory, string> = {
  intent: "意图",
  choice: "选择",
  open: "开放",
  tradeoff: "权衡",
  dependency: "依赖",
};

const typeLabel: Record<QuestionType, string> = {
  choice: "单选",
  multi: "多选",
  open: "开放",
};

function answerToText(answer?: AnswerValue): string {
  if (!answer) return "";
  if (answer.kind === "choice") return answer.option;
  if (answer.kind === "multi") return answer.options.join("、");
  return answer.text;
}

interface QuestionCardProps {
  question: Question;
}

export function QuestionCard({ question }: QuestionCardProps) {
  const { currentSessionId, updateQuestion, questions } = useSessionStore();
  const [editing, setEditing] = useState(false);
  const [multiSelected, setMultiSelected] = useState<string[]>([]);
  const [openText, setOpenText] = useState("");

  const sessionId = currentSessionId ?? question.session_id;

  const isStale = question.status === "stale";
  const isAnswered = question.status === "answered" && !editing;
  const isSkipped = question.status === "skipped";
  const isGenerating = question.status === "generating";

  // Compute stale reason: prefer runtime trigger info, fall back to depends_on lookup
  const staleReason = (() => {
    if (!isStale) return null;
    if (question.stale_triggered_by) {
      return `你对「${question.stale_triggered_by}」的回答可能影响了这道题`;
    }
    // After page refresh, stale_triggered_by is lost. Use depends_on to find trigger sources.
    const depTexts = question.depends_on
      .map((id) => questions.find((q) => q.id === id)?.question)
      .filter(Boolean) as string[];
    if (depTexts.length > 0) {
      const list = depTexts.map((t) => `「${t}」`).join("、");
      return `你更新了 ${list} 的回答，此题的前提可能已变化`;
    }
    return "此题的前提可能已变化";
  })();

  const handleChoice = async (option: string) => {
    const answer: AnswerValue = { kind: "choice", option };
    try {
      await api.answerQuestion(sessionId, question.id, answer);
      updateQuestion(question.id, { status: "answered", answer });
      toast.success("已回答");
    } catch (err) {
      console.error(err);
      toast.error("提交失败");
    }
  };

  const handleMultiConfirm = async () => {
    if (multiSelected.length === 0) {
      toast.warning("请至少选择一项");
      return;
    }
    const answer: AnswerValue = { kind: "multi", options: multiSelected };
    try {
      await api.answerQuestion(sessionId, question.id, answer);
      updateQuestion(question.id, { status: "answered", answer });
      toast.success("已回答");
    } catch (err) {
      console.error(err);
      toast.error("提交失败");
    }
  };

  const handleOpenSubmit = async () => {
    if (!openText.trim()) {
      toast.warning("请输入回答");
      return;
    }
    const answer: AnswerValue = { kind: "open", text: openText.trim() };
    try {
      await api.answerQuestion(sessionId, question.id, answer);
      updateQuestion(question.id, { status: "answered", answer });
      toast.success("已回答");
    } catch (err) {
      console.error(err);
      toast.error("提交失败");
    }
  };

  const handleSkip = async () => {
    try {
      await api.skipQuestion(sessionId, question.id);
      updateQuestion(question.id, { status: "skipped" });
      toast.success("已跳过");
    } catch (err) {
      console.error(err);
      toast.error("操作失败");
    }
  };

  const handleRestore = () => {
    updateQuestion(question.id, { status: "ready" });
  };

  const handleEdit = () => {
    setEditing(true);
    if (question.answer?.kind === "multi") {
      setMultiSelected(question.answer.options);
    } else if (question.answer?.kind === "open") {
      setOpenText(question.answer.text);
    }
  };

  const handleDismissStale = async () => {
    try {
      await api.dismissStale(question.id);
      updateQuestion(question.id, { status: "ready" });
      toast.success("已忽略提醒");
    } catch (err) {
      console.error(err);
      toast.error("操作失败");
    }
  };

  const handleRegenerate = async () => {
    try {
      await api.regenerateStale(sessionId, [question.id]);
      updateQuestion(question.id, { status: "generating" });
      toast.success("已请求重新生成");
    } catch (err) {
      console.error(err);
      toast.error("操作失败");
    }
  };

  if (isGenerating) {
    return (
      <Card className="animate-pulse">
        <CardHeader className="space-y-2">
          <div className="h-4 w-24 rounded bg-muted" />
          <div className="h-5 w-3/4 rounded bg-muted" />
          <div className="h-4 w-full rounded bg-muted" />
        </CardHeader>
        <CardContent className="space-y-2">
          <div className="h-10 w-full rounded bg-muted" />
          <div className="h-10 w-full rounded bg-muted" />
        </CardContent>
      </Card>
    );
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3 }}
    >
      <Card className={cn(isStale && "border-orange-500")}>
        <CardHeader className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <Badge className={cn("text-[10px]", categoryVariant[question.category])}>
              {categoryLabel[question.category]}
            </Badge>
            <Badge variant="outline" className="text-[10px]">
              {typeLabel[question.q_type]}
            </Badge>
            <span className="ml-auto text-xs text-muted-foreground">
              #{question.display_order + 1}
            </span>
          </div>
          <p className="text-base font-semibold leading-snug">{question.question}</p>
          {question.context && (
            <p className="text-sm text-muted-foreground">{question.context}</p>
          )}
          {question.recommended_option && question.rationale && (
            <div className="flex items-start gap-2 rounded-md bg-amber-500/10 p-2 text-sm">
              <Sparkles className="mt-0.5 h-4 w-4 shrink-0 text-amber-500" />
              <span>
                <span className="font-medium">推荐：</span>
                {question.recommended_option} — {question.rationale}
              </span>
            </div>
          )}
        </CardHeader>

        <CardContent className="space-y-3">
          {isStale && staleReason && (
            <div className="rounded-md border border-orange-500 bg-orange-500/10 p-2 text-sm text-orange-600 dark:text-orange-400">
              ⚠️ {staleReason}，建议重新生成或忽略
            </div>
          )}

          {/* Generating options placeholder */}
          {question.options.length === 0 && !isAnswered && !isSkipped && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              正在生成选项...
            </div>
          )}

          {/* Choice type */}
          {question.q_type === "choice" && question.options.length > 0 && (
            isAnswered ? (
              <AnswerBox answer={answerToText(question.answer)} />
            ) : (
              <RadioGroup
                value={question.answer?.kind === "choice" ? question.answer.option : ""}
                onValueChange={handleChoice}
                disabled={isSkipped}
              >
                {question.options.map((opt) => (
                  <div
                    key={opt.label}
                    className="flex items-start gap-3 rounded-md border p-3 cursor-pointer hover:bg-accent"
                    onClick={() => !isSkipped && handleChoice(opt.label)}
                  >
                    <RadioGroupItem value={opt.label} className="mt-1" id={`${question.id}-${opt.label}`} />
                    <div className="flex-1 space-y-0.5">
                      <Label htmlFor={`${question.id}-${opt.label}`} className="cursor-pointer font-medium">
                        {opt.label}
                      </Label>
                      {opt.description && (
                        <p className="text-xs text-muted-foreground">{opt.description}</p>
                      )}
                    </div>
                  </div>
                ))}
              </RadioGroup>
            )
          )}

          {/* Multi type */}
          {question.q_type === "multi" && question.options.length > 0 && (
            isAnswered ? (
              <AnswerBox answer={answerToText(question.answer)} />
            ) : (
              <>
                <div className="space-y-2">
                  {question.options.map((opt) => {
                    const checked = multiSelected.includes(opt.label);
                    return (
                      <div
                        key={opt.label}
                        className="flex items-start gap-3 rounded-md border p-3 cursor-pointer hover:bg-accent"
                        onClick={() => {
                          if (isSkipped) return;
                          if (checked) {
                            setMultiSelected((s) => s.filter((x) => x !== opt.label));
                          } else {
                            setMultiSelected((s) => [...s, opt.label]);
                          }
                        }}
                      >
                        <Checkbox
                          id={`${question.id}-${opt.label}`}
                          checked={checked}
                          onCheckedChange={(v) => {
                            if (v) {
                              setMultiSelected((s) => [...s, opt.label]);
                            } else {
                              setMultiSelected((s) => s.filter((x) => x !== opt.label));
                            }
                          }}
                          className="mt-1"
                          disabled={isSkipped}
                        />
                        <div className="flex-1 space-y-0.5">
                          <Label htmlFor={`${question.id}-${opt.label}`} className="cursor-pointer font-medium">
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
                <Button onClick={handleMultiConfirm} disabled={isSkipped} size="sm">
                  <Check className="mr-1 h-4 w-4" />
                  确认
                </Button>
              </>
            )
          )}

          {/* Open type */}
          {question.q_type === "open" && (
            isAnswered ? (
              <AnswerBox answer={answerToText(question.answer)} />
            ) : (
              <>
                <Textarea
                  value={openText}
                  onChange={(e) => setOpenText(e.target.value)}
                  placeholder="请输入你的回答..."
                  disabled={isSkipped}
                  className="min-h-[100px]"
                />
                <Button onClick={handleOpenSubmit} disabled={isSkipped} size="sm">
                  提交
                </Button>
              </>
            )
          )}

          {/* Status actions */}
          {isAnswered && (
            <div className="flex gap-2">
              <Button variant="outline" size="sm" onClick={handleEdit}>
                <Pencil className="mr-1 h-3.5 w-3.5" />
                修改
              </Button>
            </div>
          )}

          {isSkipped && (
            <div className="flex items-center gap-2">
              <span className="text-sm text-muted-foreground">已跳过</span>
              <Button variant="outline" size="sm" onClick={handleRestore}>
                <RotateCcw className="mr-1 h-3.5 w-3.5" />
                恢复
              </Button>
            </div>
          )}

          {isStale && (
            <div className="flex gap-2">
              <Button variant="outline" size="sm" onClick={handleDismissStale}>
                忽略提醒
              </Button>
              <Button variant="default" size="sm" onClick={handleRegenerate}>
                <RotateCcw className="mr-1 h-3.5 w-3.5" />
                重新生成
              </Button>
            </div>
          )}

          {!isAnswered && !isSkipped && !isStale && question.options.length > 0 && (
            <Button variant="ghost" size="sm" onClick={handleSkip} className="text-muted-foreground">
              <SkipForward className="mr-1 h-3.5 w-3.5" />
              跳过
            </Button>
          )}
        </CardContent>
      </Card>
    </motion.div>
  );
}

function AnswerBox({ answer }: { answer: string }) {
  return (
    <div className="space-y-2">
      <div className="rounded-md border bg-secondary/50 p-3 text-sm">
        <span className="text-muted-foreground">你的回答：</span>
        <span className="font-medium">{answer}</span>
      </div>
    </div>
  );
}
