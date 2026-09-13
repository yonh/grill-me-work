//! Iteration Blueprint — UE5-style node graph for multi-round agent prototype runs.
//!
//! Editable graph is the *plan*; each successful agent step becomes a git commit
//! (timeline node). Auto-run walks the plan in dependency order.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

pub const NODE_KINDS: &[&str] = &["start", "agent", "loop", "note", "end"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeKind {
    Start,
    Agent,
    Loop,
    Note,
    End,
}

impl GraphNodeKind {
    pub fn from_str(s: &str) -> Self {
        match s {
            "agent" => GraphNodeKind::Agent,
            "loop" => GraphNodeKind::Loop,
            "note" => GraphNodeKind::Note,
            "end" => GraphNodeKind::End,
            _ => GraphNodeKind::Start,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            GraphNodeKind::Start => "start",
            GraphNodeKind::Agent => "agent",
            GraphNodeKind::Loop => "loop",
            GraphNodeKind::Note => "note",
            GraphNodeKind::End => "end",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPos {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub position: GraphPos,
    /// Agent / loop instruction injected as TASK feedback.
    #[serde(default)]
    pub instruction: String,
    /// Optional focus hint (css / ux / a11y / features …).
    #[serde(default)]
    pub focus: Option<String>,
    /// Loop node: how many sequential agent rounds.
    #[serde(default)]
    pub count: Option<u32>,
    /// Runtime status (not persisted as authoritative; filled on load/run).
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub last_commit: Option<String>,
    #[serde(default)]
    pub completed_rounds: Option<u32>,
}

impl GraphNode {
    pub fn kind_enum(&self) -> GraphNodeKind {
        GraphNodeKind::from_str(&self.kind)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationGraph {
    pub id: String,
    pub session_id: String,
    pub name: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub entry_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl IterationGraph {
    pub fn default_for_session(session_id: &str, title: &str) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let start_id = "n_start".to_string();
        let agent_id = "n_agent1".to_string();
        let loop_id = "n_loop1".to_string();
        let end_id = "n_end".to_string();
        IterationGraph {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            name: format!("{title} · 迭代蓝图"),
            nodes: vec![
                GraphNode {
                    id: start_id.clone(),
                    kind: "start".into(),
                    label: "开始".into(),
                    position: GraphPos { x: 40.0, y: 160.0 },
                    instruction: String::new(),
                    focus: None,
                    count: None,
                    status: Some("idle".into()),
                    last_error: None,
                    last_commit: None,
                    completed_rounds: None,
                },
                GraphNode {
                    id: agent_id.clone(),
                    kind: "agent".into(),
                    label: "首轮实现".into(),
                    position: GraphPos { x: 280.0, y: 120.0 },
                    instruction: "根据访谈决策搭建完整多文件原型：导航、主页面、关键交互一次到位。"
                        .into(),
                    focus: Some("features".into()),
                    count: None,
                    status: Some("idle".into()),
                    last_error: None,
                    last_commit: None,
                    completed_rounds: None,
                },
                GraphNode {
                    id: loop_id.clone(),
                    kind: "loop".into(),
                    label: "自动打磨 ×N".into(),
                    position: GraphPos { x: 540.0, y: 120.0 },
                    instruction:
                        "对照当前原型与决策，找出最影响真实感的 1-3 处问题并直接修复：间距、层级、文案、交互反馈。只改必要的文件。"
                            .into(),
                    focus: Some("polish".into()),
                    count: Some(5),
                    status: Some("idle".into()),
                    last_error: None,
                    last_commit: None,
                    completed_rounds: None,
                },
                GraphNode {
                    id: end_id.clone(),
                    kind: "end".into(),
                    label: "结束".into(),
                    position: GraphPos { x: 820.0, y: 160.0 },
                    instruction: String::new(),
                    focus: None,
                    count: None,
                    status: Some("idle".into()),
                    last_error: None,
                    last_commit: None,
                    completed_rounds: None,
                },
            ],
            edges: vec![
                GraphEdge {
                    id: "e1".into(),
                    source: start_id.clone(),
                    target: agent_id.clone(),
                },
                GraphEdge {
                    id: "e2".into(),
                    source: agent_id.clone(),
                    target: loop_id.clone(),
                },
                GraphEdge {
                    id: "e3".into(),
                    source: loop_id,
                    target: end_id,
                },
            ],
            entry_id: Some(start_id),
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

/// One concrete agent invocation derived from the plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub node_id: String,
    pub label: String,
    pub instruction: String,
    pub focus: Option<String>,
    pub round: u32,
    pub total_rounds: u32,
}

/// Expand a graph into an ordered list of agent steps (start/note/end are control-only).
pub fn compile_plan(graph: &IterationGraph) -> Result<Vec<PlanStep>, String> {
    let by_id: HashMap<&str, &GraphNode> = graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    if by_id.is_empty() {
        return Err("迭代图为空".into());
    }

    let entry = graph
        .entry_id
        .as_deref()
        .and_then(|id| by_id.get(id).copied())
        .or_else(|| {
            graph
                .nodes
                .iter()
                .find(|n| n.kind_enum() == GraphNodeKind::Start)
        })
        .or_else(|| graph.nodes.first())
        .ok_or("找不到入口节点")?;

    // adjacency + indegree for Kahn topological walk
    let mut out_edges: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut indeg: HashMap<&str, usize> = HashMap::new();
    for n in &graph.nodes {
        indeg.entry(n.id.as_str()).or_insert(0);
        out_edges.entry(n.id.as_str()).or_default();
    }
    for e in &graph.edges {
        if !by_id.contains_key(e.source.as_str()) || !by_id.contains_key(e.target.as_str()) {
            continue;
        }
        out_edges.entry(e.source.as_str()).or_default().push(e.target.as_str());
        *indeg.entry(e.target.as_str()).or_insert(0) += 1;
    }

    // Prefer walking only the component reachable from entry
    let mut reachable: HashSet<&str> = HashSet::new();
    let mut q = VecDeque::new();
    q.push_back(entry.id.as_str());
    while let Some(id) = q.pop_front() {
        if !reachable.insert(id) {
            continue;
        }
        if let Some(nexts) = out_edges.get(id) {
            for n in nexts {
                q.push_back(n);
            }
        }
    }

    let mut queue: VecDeque<&str> = VecDeque::new();
    for n in &graph.nodes {
        let id = n.id.as_str();
        if !reachable.contains(id) {
            continue;
        }
        // indegree within reachable subgraph
        let local_in = graph
            .edges
            .iter()
            .filter(|e| e.target == id && reachable.contains(e.source.as_str()))
            .count();
        if local_in == 0 {
            queue.push_back(id);
        }
    }

    let mut order: Vec<&str> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut indeg_local: HashMap<&str, usize> = HashMap::new();
    for id in &reachable {
        let local_in = graph
            .edges
            .iter()
            .filter(|e| e.target.as_str() == *id && reachable.contains(e.source.as_str()))
            .count();
        indeg_local.insert(*id, local_in);
    }

    while let Some(id) = queue.pop_front() {
        if !seen.insert(id) {
            continue;
        }
        order.push(id);
        if let Some(nexts) = out_edges.get(id) {
            for n in nexts {
                if !reachable.contains(n) {
                    continue;
                }
                if let Some(d) = indeg_local.get_mut(n) {
                    *d = d.saturating_sub(1);
                    if *d == 0 {
                        queue.push_back(n);
                    }
                }
            }
        }
    }

    // If cycle blocked some nodes, append remaining reachable by position
    if order.len() < reachable.len() {
        let mut rest: Vec<&GraphNode> = reachable
            .iter()
            .filter(|id| !seen.contains(*id))
            .filter_map(|id| by_id.get(id).copied())
            .collect();
        rest.sort_by(|a, b| {
            a.position
                .x
                .partial_cmp(&b.position.x)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for n in rest {
            order.push(n.id.as_str());
        }
    }

    let mut steps = Vec::new();
    for id in order {
        let node = by_id[id];
        match node.kind_enum() {
            GraphNodeKind::Agent => {
                steps.push(PlanStep {
                    node_id: node.id.clone(),
                    label: node.label.clone(),
                    instruction: node.instruction.clone(),
                    focus: node.focus.clone(),
                    round: 1,
                    total_rounds: 1,
                });
            }
            GraphNodeKind::Loop => {
                let count = node.count.unwrap_or(1).clamp(1, 20);
                for i in 1..=count {
                    let instr = if node.instruction.trim().is_empty() {
                        "对照决策与当前原型，修复最影响真实感的问题。".to_string()
                    } else {
                        node.instruction.clone()
                    };
                    let instruction = if count > 1 {
                        format!("（第 {i}/{count} 轮）{instr}")
                    } else {
                        instr
                    };
                    steps.push(PlanStep {
                        node_id: node.id.clone(),
                        label: if count > 1 {
                            format!("{} · {}/{}", node.label, i, count)
                        } else {
                            node.label.clone()
                        },
                        instruction,
                        focus: node.focus.clone(),
                        round: i,
                        total_rounds: count,
                    });
                }
            }
            _ => {}
        }
    }

    if steps.is_empty() {
        return Err("图中没有可执行的 Agent / Loop 节点".into());
    }
    Ok(steps)
}

pub fn validate_graph(graph: &IterationGraph) -> Vec<String> {
    let mut problems = Vec::new();
    let ids: HashSet<&str> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
    if ids.is_empty() {
        problems.push("图为空".into());
    }
    let starts = graph
        .nodes
        .iter()
        .filter(|n| n.kind_enum() == GraphNodeKind::Start)
        .count();
    if starts == 0 {
        problems.push("缺少「开始」节点".into());
    }
    if starts > 1 {
        problems.push("存在多个「开始」节点".into());
    }
    for e in &graph.edges {
        if !ids.contains(e.source.as_str()) {
            problems.push(format!("边 {} 源节点不存在", e.id));
        }
        if !ids.contains(e.target.as_str()) {
            problems.push(format!("边 {} 目标节点不存在", e.id));
        }
    }
    for n in &graph.nodes {
        match n.kind_enum() {
            GraphNodeKind::Agent if n.instruction.trim().is_empty() => {
                problems.push(format!("Agent 节点「{}」缺少指令", n.label));
            }
            GraphNodeKind::Loop => {
                let c = n.count.unwrap_or(0);
                if c == 0 || c > 20 {
                    problems.push(format!("Loop 节点「{}」轮数应在 1–20", n.label));
                }
            }
            _ => {}
        }
    }
    match compile_plan(graph) {
        Ok(steps) => {
            if steps.is_empty() {
                problems.push("计划为空".into());
            }
        }
        Err(e) => problems.push(e),
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_graph_compiles() {
        let g = IterationGraph::default_for_session("s1", "测试");
        let plan = compile_plan(&g).unwrap();
        // 1 agent + 5 loop rounds
        assert_eq!(plan.len(), 6);
        assert_eq!(plan[0].round, 1);
        assert_eq!(plan[1].total_rounds, 5);
        assert!(plan[1].instruction.contains("第 1/5 轮"));
    }

    #[test]
    fn sanitize_and_validate() {
        let mut g = IterationGraph::default_for_session("s1", "x");
        g.nodes[1].instruction = String::new();
        let problems = validate_graph(&g);
        assert!(problems.iter().any(|p| p.contains("缺少指令")));
    }
}
