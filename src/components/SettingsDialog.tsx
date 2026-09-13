import { useEffect, useState } from "react";
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
import { Eye, EyeOff } from "lucide-react";
import { useSettings } from "@/hooks/useSettings";
import { api } from "@/lib/tauri";
import type { Settings, AgentToolStatus } from "@/lib/types";

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const defaultForm: Settings = {
  base_url: "",
  api_key: "",
  model_name: "",
  temperature: 0.7,
  batch_size: 5,
  max_concurrent_batches: 2,
  debounce_seconds: 3,
  questions_per_screen: 5,
  agent_tool: "opencode",
  agent_model: "",
  agent_effort: "auto",
  agent_auto_approve: true,
  prototype_auto_paused: false,
};

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const { settings, saveSettings } = useSettings();
  const [form, setForm] = useState<Settings>(defaultForm);
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving] = useState(false);
  const [agentTools, setAgentTools] = useState<AgentToolStatus[]>([]);

  useEffect(() => {
    if (open && settings) {
      setForm({ ...defaultForm, ...settings });
    }
  }, [open, settings]);

  useEffect(() => {
    if (!open) return;
    api.listAgentTools().then(setAgentTools).catch(() => setAgentTools([]));
  }, [open]);

  const update = <K extends keyof Settings>(key: K, value: Settings[K]) => {
    setForm((f) => ({ ...f, [key]: value }));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      await saveSettings(form);
      onOpenChange(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-xl">
        <DialogHeader>
          <DialogTitle>设置</DialogTitle>
          <DialogDescription>配置 LLM API 与生成参数</DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="base-url">Base URL</Label>
            <Input
              id="base-url"
              value={form.base_url}
              onChange={(e) => update("base_url", e.target.value)}
              placeholder="https://api.openai.com/v1"
              autoCapitalize="off"
              autoCorrect="off"
              spellCheck={false}
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="api-key">API Key</Label>
            <div className="flex gap-2">
              <Input
                id="api-key"
                type={showKey ? "text" : "password"}
                value={form.api_key}
                onChange={(e) => update("api_key", e.target.value)}
                placeholder="sk-..."
                className="flex-1"
                autoCapitalize="off"
                autoCorrect="off"
                spellCheck={false}
              />
              <Button
                type="button"
                variant="outline"
                size="icon"
                onClick={() => setShowKey((v) => !v)}
              >
                {showKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </Button>
            </div>
          </div>
          <div className="space-y-2">
            <Label htmlFor="model-name">模型名称</Label>
            <Input
              id="model-name"
              value={form.model_name}
              onChange={(e) => update("model_name", e.target.value)}
              placeholder="gpt-4o"
              autoCapitalize="off"
              autoCorrect="off"
              spellCheck={false}
            />
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label htmlFor="temperature">Temperature</Label>
              <Input
                id="temperature"
                type="number"
                step="0.1"
                min="0"
                max="2"
                value={form.temperature}
                onChange={(e) => update("temperature", parseFloat(e.target.value) || 0)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="batch-size">Batch Size</Label>
              <Input
                id="batch-size"
                type="number"
                min="1"
                value={form.batch_size}
                onChange={(e) => update("batch_size", parseInt(e.target.value) || 1)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="max-concurrent">最大并发批次</Label>
              <Input
                id="max-concurrent"
                type="number"
                min="1"
                value={form.max_concurrent_batches}
                onChange={(e) => update("max_concurrent_batches", parseInt(e.target.value) || 1)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="debounce">防抖秒数</Label>
              <Input
                id="debounce"
                type="number"
                min="0"
                value={form.debounce_seconds}
                onChange={(e) => update("debounce_seconds", parseInt(e.target.value) || 0)}
              />
            </div>
            <div className="space-y-2 col-span-2">
              <Label htmlFor="questions-per-screen">每屏问题数</Label>
              <Input
                id="questions-per-screen"
                type="number"
                min="1"
                value={form.questions_per_screen}
                onChange={(e) => update("questions_per_screen", parseInt(e.target.value) || 1)}
              />
            </div>
          </div>

          <div className="space-y-3 rounded-lg border p-3">
            <div>
              <p className="text-sm font-medium">Coding Agent（生成原型）</p>
              <p className="text-xs text-muted-foreground">
                原型由本地 agent CLI 直接写目录；Grill-Me 只负责需求与提示词
              </p>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-2">
                <Label htmlFor="agent-tool">工具</Label>
                <select
                  id="agent-tool"
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  value={form.agent_tool}
                  onChange={(e) => update("agent_tool", e.target.value)}
                >
                  {(agentTools.length > 0
                    ? agentTools
                    : (["opencode", "codex", "devin", "agy"] as const).map((id) => ({
                        id,
                        available: true,
                        path: null,
                      }))
                  ).map((t) => (
                    <option key={t.id} value={t.id} disabled={"available" in t && !t.available}>
                      {t.id}
                      {"available" in t && !t.available ? "（未安装）" : ""}
                    </option>
                  ))}
                </select>
              </div>
              <div className="space-y-2">
                <Label htmlFor="agent-model">模型（可空）</Label>
                <Input
                  id="agent-model"
                  value={form.agent_model}
                  onChange={(e) => update("agent_model", e.target.value)}
                  placeholder="留空用工具默认"
                  autoCapitalize="off"
                  autoCorrect="off"
                  spellCheck={false}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="agent-effort">推理档位</Label>
                <select
                  id="agent-effort"
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  value={form.agent_effort}
                  onChange={(e) => update("agent_effort", e.target.value)}
                >
                  {["auto", "low", "medium", "high", "xhigh", "max"].map((v) => (
                    <option key={v} value={v}>
                      {v}
                    </option>
                  ))}
                </select>
              </div>
              <div className="flex items-end gap-2 pb-2">
                <input
                  id="agent-auto"
                  type="checkbox"
                  checked={form.agent_auto_approve}
                  onChange={(e) => update("agent_auto_approve", e.target.checked)}
                />
                <Label htmlFor="agent-auto" className="font-normal">
                  自动批准写文件
                </Label>
              </div>
            </div>
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              取消
            </Button>
            <Button type="submit" disabled={saving}>
              {saving ? "保存中..." : "保存"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
