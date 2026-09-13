import { useEffect, useRef } from "react";
import {
  api,
  onNewQuestion,
  onStaleMarked,
  onSessionComplete as onSessionCompleteEvent,
  onError,
  onBatchStatus,
  onInterviewMayComplete,
  onOutlineUpdated,
  onPipelineUpdated,
  onRoundStarted,
  onRoundArchived,
  onChatStream,
  onChatMessageDone,
  onPrototypeStatus,
  onPrototypeUpdated,
} from "@/lib/tauri";
import { useSessionStore } from "@/store/sessionStore";
import { useChatStore } from "@/store/chatStore";
import { toast } from "sonner";

interface SessionCompleteInfo {
  sessionId: string;
  suggestion: string;
}

interface PrototypeUpdatedInfo {
  session_id: string;
  version: number;
  preview_url: string;
  changelog?: string | null;
  file_count: number;
}

export function useSessionEvents(
  onComplete?: (info: SessionCompleteInfo) => void,
  onMayComplete?: (sessionId: string) => void,
  onPrototypeReady?: (payload: PrototypeUpdatedInfo) => void
) {
  const { currentSessionId, addQuestion, markStale, setGenerating, setPrototypeGenerating, setOutline, setQuestions, bumpSessionPrototypeVersion } =
    useSessionStore();

  const currentSessionIdRef = useRef(currentSessionId);
  currentSessionIdRef.current = currentSessionId;

  const onCompleteRef = useRef(onComplete);
  onCompleteRef.current = onComplete;

  const onMayCompleteRef = useRef(onMayComplete);
  onMayCompleteRef.current = onMayComplete;

  const onPrototypeReadyRef = useRef(onPrototypeReady);
  onPrototypeReadyRef.current = onPrototypeReady;

  useEffect(() => {
    const unsubs: Array<() => void> = [];
    let cancelled = false;

    (async () => {
      const u1 = await onNewQuestion((q) => {
        if (currentSessionIdRef.current === q.session_id) {
          addQuestion(q);
        }
      });
      if (cancelled) { u1(); return; }
      unsubs.push(u1);

      const u2 = await onStaleMarked((sessionId, questionIds, triggeredByText) => {
        if (currentSessionIdRef.current === sessionId) {
          markStale(questionIds, triggeredByText);
          toast.warning(`${questionIds.length} 个问题已标记为过期`);
        }
      });
      if (cancelled) { u2(); return; }
      unsubs.push(u2);

      const u3 = await onSessionCompleteEvent((sessionId, suggestion) => {
        if (currentSessionIdRef.current === sessionId) {
          onCompleteRef.current?.({ sessionId, suggestion });
        }
      });
      if (cancelled) { u3(); return; }
      unsubs.push(u3);

      const u4 = await onError((_sessionId, message, kind) => {
        toast.error(`[${kind}] ${message}`);
      });
      if (cancelled) { u4(); return; }
      unsubs.push(u4);

      const u5 = await onBatchStatus((_sessionId, status, remaining) => {
        if (status === "generating") {
          setGenerating(true);
        } else if (status === "done" || status === "failed") {
          if (remaining <= 0) {
            setGenerating(false);
          }
        }
      });
      if (cancelled) { u5(); return; }
      unsubs.push(u5);

      const u6 = await onInterviewMayComplete((sessionId) => {
        if (currentSessionIdRef.current === sessionId) {
          setGenerating(false);
          onMayCompleteRef.current?.(sessionId);
        }
      });
      if (cancelled) { u6(); return; }
      unsubs.push(u6);

      const u6b = await onOutlineUpdated(async (payload) => {
        if (currentSessionIdRef.current === payload.session_id) {
          setOutline(payload.status, payload.nodes);
          // Outline edits may skip questions server-side — resync the list.
          try {
            setQuestions(await api.getQuestions(payload.session_id));
          } catch (e) {
            console.error("refresh questions after outline update", e);
          }
        }
      });
      if (cancelled) { u6b(); return; }
      unsubs.push(u6b);

      const u6c = await onPipelineUpdated((payload) => {
        if (currentSessionIdRef.current === payload.session_id) {
          useSessionStore
            .getState()
            .setPipeline(
              payload.stage,
              payload.spec ?? null,
              payload.tickets,
              payload.round,
              payload.running
            );
        }
      });
      if (cancelled) { u6c(); return; }
      unsubs.push(u6c);

      const u7 = await onChatStream((sessionId, delta) => {
        if (currentSessionIdRef.current === sessionId) {
          useChatStore.getState().appendStreamText(delta);
        }
      });
      if (cancelled) { u7(); return; }
      unsubs.push(u7);

      const u8 = await onChatMessageDone((message) => {
        if (currentSessionIdRef.current === message.session_id) {
          useChatStore.getState().finishStream(message);
        }
      });
      if (cancelled) { u8(); return; }
      unsubs.push(u8);

      const u9 = await onPrototypeStatus((sessionId, status) => {
        if (currentSessionIdRef.current === sessionId) {
          setPrototypeGenerating(status === "generating");
        }
      });
      if (cancelled) { u9(); return; }
      unsubs.push(u9);

      const reloadAfterRoundChange = async (sessionId: string) => {
        if (currentSessionIdRef.current !== sessionId) return;
        try {
          const st = useSessionStore.getState();
          const [qs, outline, p, rounds] = await Promise.all([
            api.getQuestions(sessionId),
            api.getOutline(sessionId),
            api.getPipeline(sessionId),
            api.listRounds(sessionId),
          ]);
          st.setQuestions(qs);
          st.setOutline(outline.status, outline.nodes);
          st.setPipeline(p.stage, p.spec ?? null, p.tickets, p.round, p.running);
          st.setRounds(rounds, p.round?.id);
          // Session row changed (current_round_id) — refresh sidebar entry.
          const session = await api.getSession(sessionId);
          if (session) {
            useSessionStore.setState((prev) => ({
              sessions: prev.sessions.map((s) => (s.id === sessionId ? session : s)),
            }));
          }
        } catch (e) {
          console.error("reload after round change", e);
        }
      };

      const u9a = await onRoundStarted(async ({ session_id, round }) => {
        toast.success(`第 ${round.number} 轮已开启：${round.title}`);
        await reloadAfterRoundChange(session_id);
      });
      if (cancelled) { u9a(); return; }
      unsubs.push(u9a);

      const u9b = await onRoundArchived(async ({ session_id, round }) => {
        toast.info(`第 ${round.number} 轮「${round.title}」已归档`);
        await reloadAfterRoundChange(session_id);
      });
      if (cancelled) { u9b(); return; }
      unsubs.push(u9b);

      const u10 = await onPrototypeUpdated((payload) => {
        bumpSessionPrototypeVersion(payload.session_id, payload.version);
        if (currentSessionIdRef.current === payload.session_id) {
          setPrototypeGenerating(false);
          onPrototypeReadyRef.current?.(payload);
          if (payload.changelog) {
            toast.success(`原型 v${payload.version}：${payload.changelog}`);
          } else {
            toast.success(`原型已更新到 v${payload.version}`);
          }
        }
      });
      if (cancelled) { u10(); return; }
      unsubs.push(u10);
    })();

    return () => {
      cancelled = true;
      unsubs.forEach((u) => u());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
