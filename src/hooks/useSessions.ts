import { useCallback, useEffect } from "react";
import { api, onActiveSessionChanged } from "@/lib/tauri";
import { useSessionStore } from "@/store/sessionStore";
import { toast } from "sonner";

export function useSessions() {
  const {
    sessions,
    currentSessionId,
    setSessions,
    setCurrentSession,
    addSession,
    removeSession,
    setQuestions,
    setLoading,
    setGenerating,
  } = useSessionStore();

  const loadQuestions = useCallback(
    async (id: string) => {
      try {
        const questions = await api.getQuestions(id);
        setQuestions(questions);
      } catch (err) {
        console.error(err);
        toast.error("加载问题失败");
      }
    },
    [setQuestions]
  );

  // Load sessions + restore backend "active session" so UI and MCP stay aligned.
  useEffect(() => {
    let mounted = true;
    (async () => {
      setLoading(true);
      try {
        const list = await api.getSessions();
        if (mounted) setSessions(list);
        try {
          const active = await api.getActiveSession();
          if (mounted && active.session_id) {
            setCurrentSession(active.session_id);
            await loadQuestions(active.session_id);
          }
        } catch (e) {
          console.warn("no active session", e);
        }
      } catch (err) {
        console.error("Failed to load sessions", err);
      } finally {
        if (mounted) setLoading(false);
      }
    })();
    return () => {
      mounted = false;
    };
  }, [setSessions, setLoading, setCurrentSession, loadQuestions]);

  // MCP (or another window path) can switch the open project.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    onActiveSessionChanged(async (payload) => {
      if (cancelled) return;
      const id = payload.session_id;
      if (!id) {
        setCurrentSession(null);
        setQuestions([]);
        return;
      }
      if (id === useSessionStore.getState().currentSessionId) return;
      setCurrentSession(id);
      await loadQuestions(id);
      if (payload.title) {
        toast.info(`已切换项目：${payload.title}`);
      }
    }).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [loadQuestions, setCurrentSession, setQuestions]);

  const createSession = async (title: string, role: string, initialContext?: string) => {
    try {
      const session = await api.createSession(title, role, initialContext);
      addSession(session);
      setCurrentSession(session.id);
      setQuestions([]);
      setGenerating(true);
      void api.setActiveSession(session.id).catch(() => {});
      toast.success("已创建会话，正在生成问题...");
      return session;
    } catch (err) {
      console.error(err);
      toast.error("创建会话失败");
      throw err;
    }
  };

  const deleteSession = async (id: string) => {
    try {
      await api.deleteSession(id);
      removeSession(id);
      toast.success("已删除会话");
    } catch (err) {
      console.error(err);
      toast.error("删除会话失败");
    }
  };

  const selectSession = async (id: string) => {
    setCurrentSession(id);
    setQuestions([]);
    // Persist as the app-wide active project (visible to MCP/AI).
    void api.setActiveSession(id).catch((e) => console.warn("setActiveSession", e));
    await loadQuestions(id);
  };

  const refreshSessions = async () => {
    try {
      const list = await api.getSessions();
      setSessions(list);
    } catch (err) {
      console.error("Failed to refresh sessions", err);
    }
  };

  return {
    sessions,
    currentSessionId,
    createSession,
    deleteSession,
    selectSession,
    refreshSessions,
  };
}
