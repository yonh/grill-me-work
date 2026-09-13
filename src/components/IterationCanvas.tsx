import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  Handle,
  MiniMap,
  Position,
  ReactFlow,
  addEdge,
  useEdgesState,
  useNodesState,
  type Connection,
  type Edge,
  type Node,
  type NodeProps,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
  Ban,
  Circle,
  GitBranch,
  Loader2,
  Play,
  Plus,
  RotateCcw,
  Save,
  Sparkles,
  Square,
  Trash2,
  Workflow,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { api, onGraphNodeStatus, onGraphProgress } from "@/lib/tauri";
import type { GraphNode as ApiGraphNode, IterationGraph } from "@/lib/types";
import { cn } from "@/lib/utils";
import { toast } from "sonner";

type RfNodeData = {
  kind: string;
  label: string;
  instruction: string;
  focus: string;
  count: number;
  status: string;
  lastError?: string | null;
  lastCommit?: string | null;
  onChange?: (id: string, patch: Partial<RfNodeData>) => void;
  onDelete?: (id: string) => void;
};

type RfNode = Node<RfNodeData, "blueprint">;

const KIND_META: Record<
  string,
  { label: string; color: string; border: string; bg: string; desc: string }
> = {
  start: {
    label: "开始",
    color: "text-rose-300",
    border: "border-rose-400/70",
    bg: "bg-rose-500/15",
    desc: "蓝图入口 · 控制起点",
  },
  agent: {
    label: "Agent 轮次",
    color: "text-violet-300",
    border: "border-violet-400/70",
    bg: "bg-violet-500/15",
    desc: "调用 coding agent 改一次原型",
  },
  loop: {
    label: "自动打磨 ×N",
    color: "text-cyan-300",
    border: "border-cyan-400/70",
    bg: "bg-cyan-500/15",
    desc: "按轮数串行跑多轮 agent",
  },
  note: {
    label: "备注",
    color: "text-amber-200",
    border: "border-amber-400/50",
    bg: "bg-amber-500/10",
    desc: "仅注释，不执行",
  },
  end: {
    label: "结束",
    color: "text-emerald-300",
    border: "border-emerald-400/70",
    bg: "bg-emerald-500/15",
    desc: "迭代终点",
  },
};

/** Dark-theme field styles for blueprint nodes (do not inherit light shadcn tokens). */
const darkFieldClass =
  "nodrag w-full rounded-md border border-zinc-700/80 bg-zinc-950 px-2.5 py-1.5 text-[11px] leading-snug text-zinc-100 " +
  "placeholder:text-zinc-500 " +
  "focus:outline-none focus:ring-1 focus:ring-cyan-400/50 focus:border-cyan-400/60 " +
  "selection:bg-cyan-400/30 selection:text-zinc-50";

function statusDot(status: string) {
  if (status === "running") return "bg-amber-400 animate-pulse";
  if (status === "completed") return "bg-emerald-400";
  if (status === "failed") return "bg-rose-500";
  return "bg-zinc-500";
}

function BlueprintNode({ id, data }: NodeProps<RfNode>) {
  const meta = KIND_META[data.kind] ?? KIND_META.agent;
  const isStart = data.kind === "start";
  const isEnd = data.kind === "end";
  const update = (patch: Partial<RfNodeData>) => data.onChange?.(id, patch);

  return (
    <div
      className={cn(
        "w-64 rounded-lg border shadow-[0_8px_24px_rgba(0,0,0,0.45)] backdrop-blur-sm",
        meta.border,
        "bg-zinc-900"
      )}
    >
      {!isStart && (
        <Handle
          type="target"
          position={Position.Left}
          className="!size-2.5 !border-zinc-900 !bg-violet-400"
        />
      )}
      <div
        className={cn(
          "flex items-center gap-2 rounded-t-lg border-b px-2.5 py-1.5",
          meta.border,
          meta.bg
        )}
      >
        {isStart ? (
          <Play className={cn("h-3.5 w-3.5", meta.color)} />
        ) : isEnd ? (
          <Square className={cn("h-3.5 w-3.5", meta.color)} />
        ) : data.kind === "loop" ? (
          <RotateCcw className={cn("h-3.5 w-3.5", meta.color)} />
        ) : data.kind === "note" ? (
          <Circle className={cn("h-3.5 w-3.5", meta.color)} />
        ) : (
          <Sparkles className={cn("h-3.5 w-3.5", meta.color)} />
        )}
        <span className="truncate text-xs font-semibold text-zinc-50">{data.label}</span>
        <span className={cn("ml-auto h-2 w-2 rounded-full", statusDot(data.status))} />
        {!isStart && !isEnd && (
          <button
            type="button"
            className="nodrag rounded p-0.5 text-zinc-400 hover:text-rose-400"
            title="删除"
            onClick={() => data.onDelete?.(id)}
          >
            <Trash2 className="h-3 w-3" />
          </button>
        )}
      </div>
      <div className="space-y-2 p-2.5">
        <p className="text-[10px] leading-snug text-zinc-400">{meta.desc}</p>
        {(data.kind === "agent" || data.kind === "loop" || data.kind === "note") && (
          <input
            value={data.label}
            onChange={(e) => update({ label: e.target.value })}
            className={cn(darkFieldClass, "h-7 font-medium")}
            placeholder="节点名称"
          />
        )}
        {(data.kind === "agent" || data.kind === "loop") && (
          <>
            <textarea
              value={data.instruction}
              onChange={(e) => update({ instruction: e.target.value })}
              placeholder="交给 coding agent 的修改指令…"
              className={cn(darkFieldClass, "min-h-[72px] resize-y")}
            />
            <div className="grid grid-cols-2 gap-1.5">
              <input
                value={data.focus}
                onChange={(e) => update({ focus: e.target.value })}
                placeholder="重点如 ux/css"
                className={cn(darkFieldClass, "h-7")}
              />
              {data.kind === "loop" && (
                <input
                  type="number"
                  min={1}
                  max={20}
                  value={data.count}
                  onChange={(e) =>
                    update({ count: Math.max(1, Math.min(20, Number(e.target.value) || 1)) })
                  }
                  className={cn(darkFieldClass, "h-7")}
                  title="自动跑几轮"
                />
              )}
            </div>
          </>
        )}
        {data.kind === "start" && (
          <p className="rounded border border-dashed border-zinc-600 bg-zinc-950/60 p-2 text-[10px] leading-snug text-zinc-300">
            连接到 Agent / Loop 节点，点「运行蓝图」自动串行调用 coding agent。每次成功会打一个 git commit。
          </p>
        )}
        {data.lastError && (
          <p className="rounded bg-rose-500/15 p-1.5 text-[10px] leading-snug text-rose-200">{data.lastError}</p>
        )}
        {data.lastCommit && (
          <p className="truncate rounded bg-emerald-500/15 p-1.5 font-mono text-[10px] text-emerald-200">
            commit {data.lastCommit}
          </p>
        )}
      </div>
      {!isEnd && (
        <Handle
          type="source"
          position={Position.Right}
          className="!size-2.5 !border-zinc-900 !bg-cyan-400"
        />
      )}
    </div>
  );
}

const nodeTypes = { blueprint: memo(BlueprintNode) };

function toRfNodes(
  graph: IterationGraph,
  handlers: {
    onChange: (id: string, patch: Partial<RfNodeData>) => void;
    onDelete: (id: string) => void;
  }
): RfNode[] {
  return graph.nodes.map((n) => ({
    id: n.id,
    type: "blueprint" as const,
    position: { x: n.position.x, y: n.position.y },
    data: {
      kind: n.kind,
      label: n.label,
      instruction: n.instruction,
      focus: n.focus ?? "",
      count: n.count ?? 1,
      status: n.status ?? "idle",
      lastError: n.last_error,
      lastCommit: n.last_commit,
      onChange: handlers.onChange,
      onDelete: handlers.onDelete,
    },
  }));
}

function toApiGraph(sessionId: string, base: IterationGraph, nodes: RfNode[], edges: Edge[]): IterationGraph {
  return {
    ...base,
    session_id: sessionId,
    nodes: nodes.map((n) => ({
      id: n.id,
      kind: n.data.kind,
      label: n.data.label,
      position: { x: n.position.x, y: n.position.y },
      instruction: n.data.instruction,
      focus: n.data.focus || null,
      count: n.data.kind === "loop" ? n.data.count : null,
      status: n.data.status,
      last_error: n.data.lastError ?? null,
      last_commit: n.data.lastCommit ?? null,
      completed_rounds: null,
    } satisfies ApiGraphNode)),
    edges: edges.map((e) => ({ id: e.id, source: e.source, target: e.target })),
    entry_id: nodes.find((n) => n.data.kind === "start")?.id ?? base.entry_id,
  };
}

interface IterationCanvasProps {
  sessionId: string;
}

export function IterationCanvas({ sessionId }: IterationCanvasProps) {
  const [graph, setGraph] = useState<IterationGraph | null>(null);
  const [nodes, setNodes, onNodesChange] = useNodesState<RfNode>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<{ index: number; total: number; label?: string } | null>(
    null
  );
  const [dirty, setDirty] = useState(false);
  const graphRef = useRef<IterationGraph | null>(null);

  const patchNode = useCallback((id: string, patch: Partial<RfNodeData>) => {
    setNodes((cur) => cur.map((n) => (n.id === id ? { ...n, data: { ...n.data, ...patch } } : n)));
    setDirty(true);
  }, [setNodes]);

  const deleteNode = useCallback(
    (id: string) => {
      setNodes((cur) => cur.filter((n) => n.id !== id));
      setEdges((cur) => cur.filter((e) => e.source !== id && e.target !== id));
      setDirty(true);
    },
    [setEdges, setNodes]
  );

  const load = useCallback(async () => {
    try {
      const g = await api.getIterationGraph(sessionId);
      graphRef.current = g;
      setGraph(g);
      setNodes(toRfNodes(g, { onChange: patchNode, onDelete: deleteNode }));
      setEdges(
        g.edges.map((e) => ({
          id: e.id,
          source: e.source,
          target: e.target,
          animated: true,
          style: { stroke: "#22d3ee", strokeWidth: 1.5 },
        }))
      );
      setDirty(false);
    } catch (e) {
      console.error(e);
      toast.error("加载迭代蓝图失败");
    }
  }, [deleteNode, patchNode, sessionId, setEdges, setNodes]);

  useEffect(() => {
    setGraph(null);
    setNodes([]);
    setEdges([]);
    setRunning(false);
    setProgress(null);
    load();
  }, [load, setEdges, setNodes]);

  useEffect(() => {
    let cancelled = false;
    const offs: Array<() => void> = [];
    onGraphProgress((p) => {
      if (cancelled || p.session_id !== sessionId) return;
      if (p.status === "started") {
        setRunning(true);
        setProgress({ index: 0, total: p.step_total });
      } else if (p.status === "step_start") {
        setProgress({ index: p.step_index, total: p.step_total, label: p.label ?? undefined });
      } else if (p.status === "done" || p.status === "failed" || p.status === "cancelled") {
        setRunning(false);
        setProgress(null);
        load();
      }
    }).then((u) => {
      if (cancelled) u();
      else offs.push(u);
    });
    onGraphNodeStatus((p) => {
      if (cancelled || p.session_id !== sessionId) return;
      setNodes((cur) =>
        cur.map((n) =>
          n.id === p.node_id
            ? {
                ...n,
                data: {
                  ...n.data,
                  status: p.status,
                  lastError: p.error ?? null,
                  lastCommit: p.commit ?? n.data.lastCommit,
                },
              }
            : n
        )
      );
    }).then((u) => {
      if (cancelled) u();
      else offs.push(u);
    });
    return () => {
      cancelled = true;
      offs.forEach((u) => u());
    };
  }, [load, sessionId, setNodes]);

  const onConnect = useCallback(
    (connection: Connection) => {
      setEdges((eds) =>
        addEdge(
          {
            ...connection,
            animated: true,
            style: { stroke: "#22d3ee", strokeWidth: 1.5 },
          },
          eds
        )
      );
      setDirty(true);
    },
    [setEdges]
  );

  const addAgent = useCallback(() => {
    const id = `n_agent_${Date.now()}`;
    setNodes((cur) => [
      ...cur,
      {
        id,
        type: "blueprint",
        position: { x: 280 + (cur.length % 5) * 40, y: 80 + cur.length * 20 },
        data: {
          kind: "agent",
          label: "Agent",
          instruction: "",
          focus: "",
          count: 1,
          status: "idle",
          onChange: patchNode,
          onDelete: deleteNode,
        },
      },
    ]);
    setDirty(true);
  }, [deleteNode, patchNode, setNodes]);

  const addLoop = useCallback(() => {
    const id = `n_loop_${Date.now()}`;
    setNodes((cur) => [
      ...cur,
      {
        id,
        type: "blueprint",
        position: { x: 320 + (cur.length % 5) * 40, y: 120 + cur.length * 20 },
        data: {
          kind: "loop",
          label: "打磨 ×3",
          instruction: "对照决策与当前原型，修复最影响真实感的 1–3 处问题。",
          focus: "polish",
          count: 3,
          status: "idle",
          onChange: patchNode,
          onDelete: deleteNode,
        },
      },
    ]);
    setDirty(true);
  }, [deleteNode, patchNode, setNodes]);

  const save = useCallback(async () => {
    if (!graphRef.current) return;
    try {
      const payload = toApiGraph(sessionId, graphRef.current, nodes, edges);
      await api.saveIterationGraph(sessionId, payload);
      graphRef.current = payload;
      setGraph(payload);
      setDirty(false);
      toast.success("蓝图已保存");
    } catch (e) {
      console.error(e);
      toast.error("保存失败");
    }
  }, [edges, nodes, sessionId]);

  const run = useCallback(async () => {
    if (!graphRef.current) return;
    try {
      if (dirty) {
        const payload = toApiGraph(sessionId, graphRef.current, nodes, edges);
        await api.saveIterationGraph(sessionId, payload);
        graphRef.current = payload;
        setDirty(false);
      }
      await api.runIterationGraph(sessionId);
      setRunning(true);
      toast.success("迭代蓝图已启动");
    } catch (e) {
      console.error(e);
      toast.error(String(e));
      setRunning(false);
    }
  }, [dirty, edges, nodes, sessionId]);

  const cancel = useCallback(async () => {
    try {
      await api.cancelIterationGraph(sessionId);
      toast.info("已请求取消（当前轮结束后停止）");
    } catch (e) {
      console.error(e);
    }
  }, [sessionId]);

  const problemHints = useMemo(() => {
    const starts = nodes.filter((n) => n.data.kind === "start").length;
    const agents = nodes.filter((n) => n.data.kind === "agent" || n.data.kind === "loop").length;
    const hints: string[] = [];
    if (starts === 0) hints.push("缺少开始节点");
    if (starts > 1) hints.push("多个开始节点");
    if (agents === 0) hints.push("没有 Agent/Loop 节点");
    for (const n of nodes) {
      if ((n.data.kind === "agent" || n.data.kind === "loop") && !n.data.instruction.trim()) {
        hints.push(`「${n.data.label}」缺少指令`);
      }
    }
    return hints;
  }, [nodes]);

  return (
    <div className="flex h-full min-h-0 flex-col bg-[#0b0f14]">
      <div className="flex flex-wrap items-center gap-1.5 border-b border-zinc-800/80 px-2 py-1.5">
        <GitBranch className="h-3.5 w-3.5 text-cyan-300" />
        <span className="text-xs font-semibold text-zinc-100">迭代蓝图</span>
        <Badge variant="outline" className="border-zinc-700 text-[10px] text-zinc-400">
          UE5 风格节点图
        </Badge>
        {dirty && (
          <Badge className="bg-amber-500/20 text-[10px] text-amber-300">未保存</Badge>
        )}
        {running && (
          <Badge className="gap-1 bg-violet-500/20 text-[10px] text-violet-200">
            <Loader2 className="h-3 w-3 animate-spin" />
            {progress ? `${progress.index + 1}/${progress.total}${progress.label ? ` · ${progress.label}` : ""}` : "运行中"}
          </Badge>
        )}
        <div className="ml-auto flex items-center gap-1">
          <button
            type="button"
            className="inline-flex h-7 items-center rounded-md px-2 text-[11px] text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-zinc-100 disabled:pointer-events-none disabled:opacity-50"
            onClick={addAgent}
          >
            <Plus className="mr-1 h-3 w-3" /> Agent
          </button>
          <button
            type="button"
            className="inline-flex h-7 items-center rounded-md px-2 text-[11px] text-zinc-300 transition-colors hover:bg-zinc-800 hover:text-zinc-100 disabled:pointer-events-none disabled:opacity-50"
            onClick={addLoop}
          >
            <Plus className="mr-1 h-3 w-3" /> Loop
          </button>
          <button
            type="button"
            className="inline-flex h-7 items-center rounded-md border border-zinc-600 bg-zinc-900 px-2.5 text-[11px] text-zinc-100 transition-colors hover:bg-zinc-800 hover:text-white disabled:pointer-events-none disabled:opacity-50"
            onClick={save}
            disabled={running}
          >
            <Save className="mr-1 h-3 w-3" /> 保存
          </button>
          {running ? (
            <button
              type="button"
              className="inline-flex h-7 items-center rounded-md border border-rose-700 bg-rose-950/40 px-2.5 text-[11px] text-rose-200 transition-colors hover:bg-rose-900/50"
              onClick={cancel}
            >
              <Ban className="mr-1 h-3 w-3" /> 取消
            </button>
          ) : (
            <button
              type="button"
              className="inline-flex h-7 items-center rounded-md bg-cyan-500 px-2.5 text-[11px] font-medium text-zinc-950 transition-colors hover:bg-cyan-400 disabled:pointer-events-none disabled:opacity-50"
              onClick={run}
              disabled={problemHints.length > 0}
            >
              <Play className="mr-1 h-3 w-3" /> 运行蓝图
            </button>
          )}
        </div>
      </div>

      {problemHints.length > 0 && (
        <div className="border-b border-rose-900/50 bg-rose-950/30 px-3 py-1 text-[11px] text-rose-300">
          {problemHints.join(" · ")}
        </div>
      )}

      <div className="min-h-0 flex-1">
        {graph ? (
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            nodeTypes={nodeTypes}
            fitView
            proOptions={{ hideAttribution: true }}
            className="bg-[#0b0f14]"
            defaultEdgeOptions={{
              animated: true,
              style: { stroke: "#22d3ee", strokeWidth: 1.5 },
            }}
          >
            <Background variant={BackgroundVariant.Dots} gap={18} size={1} color="#1e293b" />
            <Controls className="!bg-zinc-900 !border-zinc-700 [&>button]:!bg-zinc-800 [&>button]:!border-zinc-700 [&>button]:!fill-zinc-300" />
            <MiniMap
              className="!bg-zinc-950 !border-zinc-800"
              nodeColor={(n) => {
                const k = (n.data as RfNodeData)?.kind;
                if (k === "start") return "#fb7185";
                if (k === "loop") return "#22d3ee";
                if (k === "end") return "#34d399";
                if (k === "note") return "#fbbf24";
                return "#a78bfa";
              }}
              maskColor="rgba(0,0,0,0.65)"
            />
          </ReactFlow>
        ) : (
          <div className="flex h-full items-center justify-center text-xs text-zinc-500">
            <Workflow className="mr-2 h-4 w-4" /> 加载蓝图…
          </div>
        )}
      </div>
    </div>
  );
}
