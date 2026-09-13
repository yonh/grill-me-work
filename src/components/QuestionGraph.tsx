import { useMemo, useState } from "react";
import {
  ReactFlow,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  type Node,
  type Edge,
  type NodeProps,
  Handle,
  Position,
  MarkerType,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { CheckCircle2, Clock, SkipForward, AlertTriangle, Loader2 } from "lucide-react";
import { useSessionStore } from "@/store/sessionStore";
import type { AnswerValue, Question, QuestionStatus } from "@/lib/types";
import { cn } from "@/lib/utils";

const STATUS_STYLE: Record<QuestionStatus, { color: string; label: string; icon: React.ReactNode }> = {
  ready: { color: "#3b82f6", label: "待答", icon: <Clock className="h-3 w-3" /> },
  answered: { color: "#22c55e", label: "已答", icon: <CheckCircle2 className="h-3 w-3" /> },
  skipped: { color: "#9ca3af", label: "跳过", icon: <SkipForward className="h-3 w-3" /> },
  stale: { color: "#f59e0b", label: "过期", icon: <AlertTriangle className="h-3 w-3" /> },
  generating: { color: "#a855f7", label: "生成中", icon: <Loader2 className="h-3 w-3 animate-spin" /> },
};

function answerText(a?: AnswerValue): string | null {
  if (!a) return null;
  if (a.kind === "choice") return a.option;
  if (a.kind === "multi") return a.options.join("、");
  return a.text;
}

type QNodeData = { question: Question; nodeTitle?: string };

function QuestionNode({ data, selected }: NodeProps<Node<QNodeData>>) {
  const q = data.question;
  const st = STATUS_STYLE[q.status];
  const ans = answerText(q.answer);
  return (
    <div
      className={cn(
        "w-[230px] rounded-md border bg-card text-left shadow-sm",
        selected && "ring-2 ring-primary"
      )}
      style={{ borderLeft: `3px solid ${st.color}` }}
    >
      <Handle type="target" position={Position.Top} className="!h-1.5 !w-1.5" />
      <div className="p-2">
        <div className="flex items-center gap-1" style={{ color: st.color }}>
          {st.icon}
          <span className="text-[10px] font-medium">{st.label}</span>
          {data.nodeTitle && (
            <span className="ml-auto max-w-[110px] truncate text-[9px] text-muted-foreground">
              {data.nodeTitle}
            </span>
          )}
        </div>
        <p className="mt-1 line-clamp-2 text-[11px] font-medium leading-snug">{q.question}</p>
        {ans && (
          <p className="mt-1 line-clamp-1 rounded bg-green-500/10 px-1 text-[10px] text-green-700 dark:text-green-400">
            ✓ {ans}
          </p>
        )}
        {q.status === "skipped" && (
          <p className="mt-1 text-[10px] text-muted-foreground">已跳过（负面信号）</p>
        )}
        {q.status === "stale" && (
          <p className="mt-1 text-[10px] text-amber-600">已过期 · 依赖答案变化</p>
        )}
      </div>
      <Handle type="source" position={Position.Bottom} className="!h-1.5 !w-1.5" />
    </div>
  );
}

const nodeTypes = { qnode: QuestionNode };

/** Layered DAG of interview questions; edges = depends_on. */
export function QuestionGraph() {
  const questions = useSessionStore((s) => s.questions);
  const outlineNodes = useSessionStore((s) => s.outlineNodes);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const nodeTitles = useMemo(
    () => new Map(outlineNodes.map((n) => [n.id, n.title])),
    [outlineNodes]
  );

  const { nodes, edges } = useMemo(() => {
    const byId = new Map(questions.map((q) => [q.id, q]));

    // depth = longest depends_on chain (cycle-safe)
    const depth = new Map<string, number>();
    const depthOf = (q: Question, seen: Set<string>): number => {
      const cached = depth.get(q.id);
      if (cached !== undefined) return cached;
      if (seen.has(q.id)) return 0;
      seen.add(q.id);
      const deps = q.depends_on.filter((d) => byId.has(d));
      const d =
        deps.length === 0 ? 0 : Math.max(...deps.map((p) => depthOf(byId.get(p)!, seen))) + 1;
      seen.delete(q.id);
      depth.set(q.id, d);
      return d;
    };
    questions.forEach((q) => depthOf(q, new Set()));

    // group by layer, preserve display_order within a layer
    const layers = new Map<number, Question[]>();
    [...questions]
      .sort((a, b) => a.display_order - b.display_order)
      .forEach((q) => {
        const d = depth.get(q.id) ?? 0;
        if (!layers.has(d)) layers.set(d, []);
        layers.get(d)!.push(q);
      });

    const nodes: Node<QNodeData>[] = [];
    layers.forEach((qs, layer) => {
      const w = 250;
      const total = qs.length * w;
      qs.forEach((q, i) => {
        nodes.push({
          id: q.id,
          type: "qnode",
          position: { x: i * w - total / 2, y: layer * 190 },
          data: {
            question: q,
            nodeTitle: q.outline_node_id ? nodeTitles.get(q.outline_node_id) : undefined,
          },
        });
      });
    });

    const edges: Edge[] = [];
    questions.forEach((q) =>
      q.depends_on.forEach((dep) => {
        if (!byId.has(dep)) return;
        edges.push({
          id: `${dep}->${q.id}`,
          source: dep,
          target: q.id,
          type: "smoothstep",
          markerEnd: { type: MarkerType.ArrowClosed, width: 14, height: 14 },
          style: { stroke: STATUS_STYLE[q.status].color, strokeWidth: 1.5, opacity: 0.7 },
        });
      })
    );

    return { nodes, edges };
  }, [questions, nodeTitles]);

  const selected = questions.find((q) => q.id === selectedId) ?? null;
  const ans = selected ? answerText(selected.answer) : null;

  return (
    <div className="relative h-full w-full">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        fitView
        minZoom={0.2}
        nodesDraggable={false}
        nodesConnectable={false}
        onNodeClick={(_, n) => setSelectedId(n.id)}
        onPaneClick={() => setSelectedId(null)}
        proOptions={{ hideAttribution: true }}
      >
        <Background variant={BackgroundVariant.Dots} gap={16} size={1} />
        <Controls className="!bg-card" />
        <MiniMap
          pannable
          className="!bg-card"
          nodeColor={(n) => STATUS_STYLE[(n.data as QNodeData).question.status].color}
        />
      </ReactFlow>

      {selected && (
        <div className="absolute bottom-3 left-3 right-3 z-10 max-h-[45%] overflow-auto rounded-md border bg-card/95 p-3 shadow-lg backdrop-blur">
          <div className="flex items-center gap-2 text-[10px]" style={{ color: STATUS_STYLE[selected.status].color }}>
            {STATUS_STYLE[selected.status].icon}
            <span>{STATUS_STYLE[selected.status].label}</span>
            <span className="font-mono text-muted-foreground">{selected.id}</span>
            {selected.outline_node_id && nodeTitles.get(selected.outline_node_id) && (
              <span className="rounded bg-secondary px-1 py-0.5 text-secondary-foreground">
                {nodeTitles.get(selected.outline_node_id)}
              </span>
            )}
          </div>
          <p className="mt-1.5 text-xs font-semibold leading-snug">{selected.question}</p>
          {selected.context && (
            <p className="mt-1 text-[11px] text-muted-foreground">{selected.context}</p>
          )}
          {selected.options.length > 0 && (
            <ul className="mt-2 space-y-1">
              {selected.options.map((o) => (
                <li
                  key={o.label}
                  className={cn(
                    "rounded border px-2 py-1 text-[11px]",
                    ans && (ans === o.label || ans.includes(o.label))
                      ? "border-green-500/50 bg-green-500/10"
                      : "text-muted-foreground"
                  )}
                >
                  {o.label}
                  {o.description && <span className="ml-1 opacity-70">— {o.description}</span>}
                </li>
              ))}
            </ul>
          )}
          {ans && <p className="mt-2 text-xs text-green-600 dark:text-green-400">回答：{ans}</p>}
          {selected.rationale && (
            <p className="mt-1 text-[10px] text-muted-foreground">推荐理由：{selected.rationale}</p>
          )}
        </div>
      )}
    </div>
  );
}
