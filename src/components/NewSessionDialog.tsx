import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useSessions } from "@/hooks/useSessions";
import { Users, Code, Palette, ClipboardList, Pencil } from "lucide-react";

const PRESET_ROLES = [
  { id: "pm", label: "需求方/产品经理", icon: Users, desc: "厘清要什么：功能、权限、数据、业务流程" },
  { id: "dev", label: "开发者/技术负责人", icon: Code, desc: "怎么实现：技术栈、架构、API、数据模型" },
  { id: "designer", label: "设计师", icon: Palette, desc: "用户怎么用：交互、视觉、流程、可访问性" },
  { id: "manager", label: "项目经理", icon: ClipboardList, desc: "范围与优先级：里程碑、风险、资源、边界" },
];

interface NewSessionDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function NewSessionDialog({ open, onOpenChange }: NewSessionDialogProps) {
  const { createSession } = useSessions();
  const [title, setTitle] = useState("");
  const [selectedRole, setSelectedRole] = useState("pm");
  const [customRole, setCustomRole] = useState("");
  const [context, setContext] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const reset = () => {
    setTitle("");
    setSelectedRole("pm");
    setCustomRole("");
    setContext("");
  };

  const isCustom = selectedRole === "custom";
  const role = isCustom ? customRole.trim() : selectedRole;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!title.trim() || !role) return;
    setSubmitting(true);
    try {
      await createSession(title.trim(), role, context.trim() || undefined);
      reset();
      onOpenChange(false);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>新建会话</DialogTitle>
          <DialogDescription>创建一个新的需求访谈会话</DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="session-title">标题</Label>
            <Input
              id="session-title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="例如：电商后台系统"
              required
              autoFocus
            />
          </div>

          <div className="space-y-2">
            <Label>角色</Label>
            <div className="grid grid-cols-2 gap-2">
              {PRESET_ROLES.map((r) => {
                const Icon = r.icon;
                const active = selectedRole === r.id;
                return (
                  <button
                    key={r.id}
                    type="button"
                    onClick={() => setSelectedRole(r.id)}
                    className={`flex flex-col items-start gap-1 rounded-lg border p-3 text-left transition-colors ${
                      active
                        ? "border-primary bg-primary/5 ring-1 ring-primary"
                        : "border-border hover:bg-accent"
                    }`}
                  >
                    <div className="flex items-center gap-2">
                      <Icon className="h-4 w-4 text-primary" />
                      <span className="text-sm font-medium">{r.label}</span>
                    </div>
                    <span className="text-xs text-muted-foreground">{r.desc}</span>
                  </button>
                );
              })}
            </div>
            <button
              type="button"
              onClick={() => setSelectedRole("custom")}
              className={`flex items-center gap-2 rounded-lg border p-3 w-full transition-colors ${
                isCustom
                  ? "border-primary bg-primary/5 ring-1 ring-primary"
                  : "border-border hover:bg-accent"
              }`}
            >
              <Pencil className="h-4 w-4 text-primary shrink-0" />
              <Input
                value={customRole}
                onChange={(e) => {
                  setCustomRole(e.target.value);
                  setSelectedRole("custom");
                }}
                placeholder="自定义角色描述，如：运维工程师，关注部署、监控、容灾"
                className="border-0 p-0 h-auto focus-visible:ring-0 text-sm"
                onFocus={() => setSelectedRole("custom")}
              />
            </button>
          </div>

          <div className="space-y-2">
            <Label htmlFor="session-context">初始背景（可选）</Label>
            <Textarea
              id="session-context"
              value={context}
              onChange={(e) => setContext(e.target.value)}
              placeholder="描述项目背景、目标用户、核心场景等..."
              className="min-h-[120px]"
            />
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              取消
            </Button>
            <Button type="submit" disabled={!title.trim() || !role || submitting}>
              {submitting ? "创建中..." : "创建"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
