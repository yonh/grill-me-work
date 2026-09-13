import { useEffect, useRef } from "react";
import {
  onNewQuestion,
  onStaleMarked,
  onSessionComplete as onSessionCompleteEvent,
  onError,
  onBatchStatus,
  onInterviewMayComplete,
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
  const { currentSessionId, addQuestion, markStale, setGenerating, setPrototypeGenerating, bumpSessionPrototypeVersion } =
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
