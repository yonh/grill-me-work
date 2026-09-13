import { useEffect } from "react";
import { api } from "@/lib/tauri";
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

  useEffect(() => {
    let mounted = true;
    (async () => {
      setLoading(true);
      try {
        const list = await api.getSessions();
        if (mounted) setSessions(list);
      } catch (err) {
        console.error("Failed to load sessions", err);
      } finally {
        if (mounted) setLoading(false);
      }
    })();
    return () => {
      mounted = false;
    };
  }, [setSessions, setLoading]);

  const createSession = async (title: string, role: string, initialContext?: string) => {
    try {
      const session = await api.createSession(title, role, initialContext);
      addSession(session);
      setCurrentSession(session.id);
      setQuestions([]);
      setGenerating(true);
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
    try {
      const questions = await api.getQuestions(id);
      setQuestions(questions);
    } catch (err) {
      console.error(err);
      toast.error("加载问题失败");
    }
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
