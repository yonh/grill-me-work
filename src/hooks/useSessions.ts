import { useCallback, useEffect } from "react";
import { api, onActiveSessionChanged, onSessionCreated, onSessionDeleted } from "@/lib/tauri";
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
    setOutline,
    setPipeline,
    setRounds,
    setActivities,
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
      try {
        const outline = await api.getOutline(id);
        setOutline(outline.status, outline.nodes);
      } catch (err) {
        console.error(err);
      }
      try {
        const p = await api.getPipeline(id);
        setPipeline(p.stage, p.spec ?? null, p.tickets, p.round, p.running);
      } catch (err) {
        console.error(err);
      }
      try {
        const rounds = await api.listRounds(id);
        setRounds(rounds, useSessionStore.getState().currentRound?.id);
      } catch (err) {
        console.error(err);
      }
      try {
        const activities = await api.listActivity(id, 200);
        setActivities(activities);
      } catch (err) {
        console.error(err);
      }
    },
    [setQuestions, setOutline, setPipeline, setRounds, setActivities]
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

  // Sessions created/deleted outside the UI (MCP) push events to keep the
  // sidebar in sync — the list is only fully loaded at startup otherwise.
  useEffect(() => {
    let cancelled = false;
    const unsubs: (() => void)[] = [];
    onSessionCreated((session) => {
      if (cancelled) return;
      if (useSessionStore.getState().sessions.some((s) => s.id === session.id)) return;
      addSession(session);
      toast.info(`新项目已创建：${session.title}`);
    }).then((u) => { if (cancelled) u(); else unsubs.push(u); });
    onSessionDeleted((id) => {
      if (cancelled) return;
      removeSession(id);
    }).then((u) => { if (cancelled) u(); else unsubs.push(u); });
    return () => {
      cancelled = true;
      unsubs.forEach((u) => u());
    };
  }, [addSession, removeSession]);

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
      toast.success("已创建会话，正在生成访谈大纲...");
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
