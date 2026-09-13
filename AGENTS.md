# Grill-Me V2

Tauri v2 桌面应用。**面向原型开发**的需求访谈助手：每回答一道题，自动增量更新多文件 HTML 原型。

## 与 v1 的差异

| | v1 | v2 |
|---|---|---|
| 产出 | 单 HTML 字符串 | 多文件目录（index.html / css / js / pages） |
| 触发 | 稳定总结后再生成 | **答完一题自动增量更新** |
| 存储 | SQLite `prototype_html` | 磁盘 `~/.grill-work/<session_id>/{docs,prototype,.git}` |
| 预览 | iframe srcDoc | 本地静态服务 `http://127.0.0.1:<port>` |
| 历史 | 版本号 + DB snapshot | **git 决策树**（每次成功生成 = 1 commit） |
| 迭代 | 手动反馈单次 | **UE5 蓝图式节点图**（可自动跑 N 轮） |

## 命令

| 命令 | 说明 |
|------|------|
| `npm run dev` | Vite 开发服务器，端口 **1420** |
| `npm run tauri dev` | 完整桌面应用 |
| `cargo test` | 在 `src-tauri/` 下 |
| `npm run tauri build` | 生产构建 |

## 架构

- **前端** `src/`：React 19 + Zustand + Tailwind + shadcn/ui + @xyflow/react
- **后端** `src-tauri/`：Tauri v2、rusqlite、tokio actor
- **工作区** `src-tauri/src/workspace/`：`~/.grill-work/<session_id>/`
- **原型模块** `src-tauri/src/prototype/`：`prototype/` 目录读写、路径安全、tiny_http 预览
- **Git 模块** `src-tauri/src/git/`：系统 git 桥接（仓库在工作区根）
- **迭代图** `src-tauri/src/graph/`：蓝图节点编译为串行 agent 计划

### 磁盘布局

```
~/.grill-work/
├── grill-me-v2.db          # SQLite（会话/问题/设置；不再存 HTML 快照为真相源）
└── <session_id>/
    ├── docs/
    │   ├── decisions.md    # 决策索引（每次生成前同步写入）
    │   └── intent.md
    ├── prototype/          # 静态原型；agent 只改这里；预览 root
    │   ├── index.html
    │   ├── css/ js/ pages/
    ├── TASK.md             # 本次 agent 任务（工作区根）
    ├── .iteration-graph.json
    └── .git/               # 决策树；一次成功生成 = 1 commit
```

旧路径 `~/Library/Application Support/grill-me-v2/prototypes/<sid>` 会在 `ensure_workspace` 时自动迁移到 `~/.grill-work/<sid>/prototype`。

## 原型生成（coding agent CLI）

Grill-Me **不自己写原型代码**。每次需要更新时：

1. 同步写入 `docs/decisions.md` + `docs/intent.md`
2. 组装任务提示词写入工作区根 `TASK.md`（布局约束：只改 `prototype/`，先读 `docs/`）
3. agent 的 **cwd = 工作区根**，在 `prototype/` 内直接改文件
4. 成功后 bump version、在**工作区根** git commit、发 `prototype_updated`、刷新预览

设置：`agent_tool` / `agent_model` / `agent_effort` / `agent_auto_approve`。

我们只维护：**需求（访谈决策）+ 提示词调度**。

答题成功后 `spawn_auto_prototype_update`（800ms 防抖）自动触发 agent。

## 迭代蓝图（UE5 节点图）

右侧原型面板三个视图：**预览 | 蓝图 | 时间线**。

**蓝图**（`src/components/IterationCanvas.tsx`）：
- 节点：`start` / `agent`（单轮）/ `loop`（×N 自动打磨）/ `note` / `end`
- 连线定义依赖顺序；`compile_plan` 拓扑展开为串行 agent 步骤
- 「运行蓝图」后台跑完整计划；每步成功自动 git commit；可取消
- 存储：`prototypes/<sid>/.iteration-graph.json`

**时间线**（`src/components/TimelineGraph.tsx`）：
- 读 `git log --all`；节点 = commit；分支 tip 高亮
- 预览历史版本 / 切分支 / 从选中 commit 开新时间线

## MCP（AI 接入）

应用启动时在 `127.0.0.1:8787`（端口占用则 +1）拉起 MCP HTTP 服务，外部 AI 可调用全部能力。

| 端点 | 说明 |
|------|------|
| `POST /mcp` | JSON-RPC 2.0：`initialize` / `tools/list` / `tools/call` |
| `GET /health` | 健康检查 + 当前工具列表 |

已实现工具：

会话发现：

- `list_sessions` — 列出项目/会话（含 `is_active`、工作区路径、outline_status）
- `get_active_session` — 当前打开的项目
- `set_active_session` — 切换打开的项目（UI 会跟随 `active_session_changed` 事件）

访谈回路：`create_session` / `get_session_state` / `get_outline` / `generate_outline` / `save_outline` / `confirm_outline` / `dismiss_outline` / `list_questions` / `answer_question` / `skip_question` / `send_message` / `request_batch` / `finish_session` / `get_messages` / `regenerate_stale` / `dismiss_stale`

原型与分支：`generate_prototype` / `get_prototype_versions` / `cancel_prototype` / `get_agent_log` / `get_timeline` / `checkout_ref` / `fork_branch` / `run_graph` / `cancel_graph`

流水线（spec→tickets→分支地图）：`get_pipeline` / `generate_spec` / `save_spec` / `confirm_spec` / `generate_tickets` / `save_tickets` / `confirm_tickets` / `set_ticket_status` / `run_ticket`

访谈回路（agent 可驱动完整访谈）：

- `create_session` — {title, role?, initial_context?} 建会话并自动生成大纲
- `get_session_state` — 状态快照：大纲节点覆盖 + 各状态题数（轮询入口）
- `get_outline` / `generate_outline` / `save_outline` — 读/重生/编辑大纲（save 支持 excluded 排除节点）
- `confirm_outline` / `dismiss_outline` — 确认大纲开始出题 / 放弃大纲转自由模式
- `list_questions` — {status?} 列问题（含 options 与所属节点标题）
- `answer_question` — answer 传字符串（开放）或 `{kind:"choice"|"multi"|"open",...}`
- `skip_question` — 跳题（负面信号）
- `send_message` — 发访谈对话消息，表达方向/约束
- `request_batch` — 手动补一批题
- `finish_session` — 结束会话，返回决策+总结
- `generate_prototype` — 触发原型生成（阻塞至 agent 跑完，返回 version/changelog/file_count）
- `get_messages` / `get_prototype_versions` — 读对话记录 / 原型版本历史
- `regenerate_stale` / `dismiss_stale` — 重出/废弃 stale 题

决策树/抽卡式开发：

- `get_timeline` — git 状态 + 提交图（找基点 commit）
- `checkout_ref` — 切换分支/commit（prototype/ 与预览随之切换）
- `fork_branch` — 从 commit 检出新分支
- `run_graph` / `cancel_graph` — 运行/取消迭代蓝图

典型 agent 流程：`create_session` → 轮询 `get_outline` 到 `draft` → `save_outline`/`confirm_outline` → 轮询 `list_questions?status=ready` + `answer_question`/`skip_question` → 节点全覆盖后 `finish_session` → `generate_prototype`。

抽卡模式（单 working dir，**串行分叉**，非真并行）：`get_timeline` 找基点 → `fork_branch("feat-try1")` → `generate_prototype` ×N 轮（每轮自动 commit）→ `checkout_ref` 回基点 → `fork_branch("feat-try2")` → 再迭代 → `get_timeline`/`get_prototype_versions` 对比各线结果。真并行需 git worktree + 每分支独立预览端口，未实现。

「激活项目」存在 SQLite `settings.active_session_id`；UI 选中会话时写入，MCP 也可切换，方便 AI 默认作用于用户正在看的项目。

扩展方式：在 `src-tauri/src/mcp/tools.rs` 注册 tool 定义 + handler；handler 拿 `McpContext`（`store` + `scheduler_tx` + `app`）。MCP 服务跑在普通线程上（非 tokio runtime），调调度器用 `scheduler_tx.blocking_send` + `oneshot::blocking_recv`（见 `sched_call` helper）。

```bash
curl -s http://127.0.0.1:8787/health
curl -s http://127.0.0.1:8787/mcp -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_sessions","arguments":{}}}'
curl -s http://127.0.0.1:8787/mcp -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"set_active_session","arguments":{"session_id":"<id>"}}}'
```

## 开发流水线（staged flow）

新会话 `pipeline_stage = interviewing`，强制门控：

```
interviewing ──generate_spec──→ spec_draft ──confirm_spec──→ tickets_draft ──confirm_tickets──→ developing
```

- `spec_draft`：spec markdown 存 `sessions.spec`，可编辑/重生；确认时写入 `docs/spec.md` 并自动拆 tickets
- `tickets_draft`：tickets（标题/描述/依赖/排序）可编辑/重生
- `developing`：`run_ticket` 为某 ticket 开分支线（自动 fork + N 轮 agent 迭代）；同 ticket 可开多线抽卡
- **门控**：stage≠developing 时 `generate_prototype` 被拒绝（旧会话 stage=none 不受限）
- 右侧面板「地图」页签：每条 ticket 一条泳道，卡片+分支 commit 链

## 事件

- `prototype_status`: generating | done | failed
- `prototype_updated`: { session_id, version, preview_url, changelog, file_count }
- `agent_output`: { session_id, stream, text } — agent 实时日志
- `graph_progress`: started | step_start | step_done | step_failed | done | cancelled
- `graph_node_status`: per-node running/completed/failed + commit sha
- `timeline_updated`: branch/head 变更
- `pipeline_updated`: { session_id, stage, spec, tickets, running }
- 以及 v1 的 new_question / stale_marked / batch_status / error / chat_*

## 布局

```
┌ Sidebar ┌ 访谈对话 + 问题 ┌ 原型：预览 / 蓝图 / 时间线 ┐ ┐
```

右侧原型面板可切换三视图；预览下可展开文件树与 agent 日志；底部反馈手动生成下一版。
