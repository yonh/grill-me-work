import { useEffect, useState } from "react";
import { api } from "@/lib/tauri";
import { useSessionStore } from "@/store/sessionStore";
import { toast } from "sonner";
import type { Settings } from "@/lib/types";

const defaultSettings: Settings = {
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

export function useSettings() {
  const { settings, setSettings } = useSessionStore();
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    let mounted = true;
    (async () => {
      setLoading(true);
      try {
        const s = await api.getSettings();
        if (mounted) setSettings(s);
      } catch (err) {
        console.error("Failed to load settings", err);
        if (mounted) setSettings(defaultSettings);
      } finally {
        if (mounted) setLoading(false);
      }
    })();
    return () => {
      mounted = false;
    };
  }, [setSettings]);

  const saveSettings = async (s: Settings) => {
    try {
      await api.saveSettings(s);
      setSettings(s);
      toast.success("设置已保存");
    } catch (err) {
      console.error(err);
      toast.error("保存设置失败");
      throw err;
    }
  };

  return { settings, loading, saveSettings };
}
