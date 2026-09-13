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

- `list_sessions` — 列出项目/会话（含 `is_active`、工作区路径）
- `get_active_session` — 当前打开的项目
- `set_active_session` — 切换打开的项目（UI 会跟随 `active_session_changed` 事件）

「激活项目」存在 SQLite `settings.active_session_id`；UI 选中会话时写入，MCP 也可切换，方便 AI 默认作用于用户正在看的项目。

扩展方式：在 `src-tauri/src/mcp/tools.rs` 注册 tool 定义 + handler；handler 拿 `McpContext`（`store` + `scheduler_tx` + `app`）。

```bash
curl -s http://127.0.0.1:8787/health
curl -s http://127.0.0.1:8787/mcp -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_sessions","arguments":{}}}'
curl -s http://127.0.0.1:8787/mcp -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"set_active_session","arguments":{"session_id":"<id>"}}}'
```

## 事件

- `prototype_status`: generating | done | failed
- `prototype_updated`: { session_id, version, preview_url, changelog, file_count }
- `agent_output`: { session_id, stream, text } — agent 实时日志
- `graph_progress`: started | step_start | step_done | step_failed | done | cancelled
- `graph_node_status`: per-node running/completed/failed + commit sha
- `timeline_updated`: branch/head 变更
- 以及 v1 的 new_question / stale_marked / batch_status / error / chat_*

## 布局

```
┌ Sidebar ┌ 访谈对话 + 问题 ┌ 原型：预览 / 蓝图 / 时间线 ┐ ┐
```

右侧原型面板可切换三视图；预览下可展开文件树与 agent 日志；底部反馈手动生成下一版。
