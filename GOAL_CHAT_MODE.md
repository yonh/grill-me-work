# Goal: Chat + 问答混合模式

## 概述

将会话主界面从纯问答模式改造为 **Chat 对话为主 + 结构化问答为辅** 的混合模式。
用户与 AI 自由对话，AI 通过 tool-calling 自主决定何时生成结构化问题卡片。

## 设计决策

| 决策点 | 选择 |
|---|---|
| 会话入口 | Chat 为主界面，右侧抽屉显示所有问题列表 |
| 问答嵌入 | 内联卡片 — 问题嵌入在 AI 回复消息中，可展开答题 |
| 用户回答 | 仅卡片内答题，chat 输入框只用于自由对话 |
| 现有问答 | 完全替换 — 删除 QuestionList 主界面 |
| AI 工具 | create_batch(questions[]) + finish_interview() — LLM tool-calling |
| 流式输出 | 流式输出文本部分，tool_call 在流结束后处理生成卡片 |
| 抽屉内容 | 所有问题列表（按状态分组：待答/已答/跳过），可在抽屉内直接答题 |
| 会话结束 | AI 建议 + 用户手动，两者并存 |
| 历史消息 | 完整持久化到 DB，重新打开会话可恢复对话 |

## UI 布局

```
┌─ Header (session title + role badge + 完成访谈按钮) ──────────────┐
├─ Chat 区域 (flex-1) ────────────────────────────┬─ 抽屉 (可折叠) ─┐
│                                                  │ 待答 (3)       │
│  AI: 你好，我们来聊聊门店中心的需求...           │ ├ Q: 库存预警？│
│  User: 我想做多门店管理，主要解决库存问题        │ ├ Q: 多仓？    │
│  AI: 明白。关于库存有几个关键点需要确认：        │ ├ Q: 权限？    │
│  ┌────────────────────────────────────┐          │                │
│  │ 📋 结构化问题 (内联卡片)            │          │ 已答 (5)       │
│  │ ▸ Q1: 库存预警阈值？ [展开答题]     │          │ ├ Q: 门店数量？│
│  │ ▸ Q2: 是否需要多仓？ [展开答题]     │          │ └ Q: 角色权限？│
│  └────────────────────────────────────┘          │                │
│  User: 阈值 20%，需要多仓                         │ 跳过 (1)       │
│  AI: 已记录。关于门店管理...                      │ └ Q: 扩展计划？│
│                                                  │                │
├──────────────────────────────────────────────────┤                │
│  [输入框] 说点什么...              [发送]         │                │
└──────────────────────────────────────────────────┴────────────────┘
```

## 数据模型

### 新建 `messages` 表

```sql
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,          -- 'user' | 'assistant'
    content TEXT NOT NULL,       -- 文本内容
    question_ids TEXT,           -- JSON array of question IDs embedded in this message
    tool_calls TEXT,             -- JSON array of raw tool calls (for debugging)
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);
CREATE INDEX idx_messages_session ON messages(session_id);
```

### 现有表不变

- `questions` 表保持不变，新增 `message_id` 列关联到生成它的 chat 消息
- `decision_summary` 表不变
- `sessions` 表不变

### questions 表新增列

```sql
ALTER TABLE questions ADD COLUMN message_id TEXT;  -- 关联到生成它的 message
```

## LLM Tool Calling

### System Prompt 结构

```
你是 grill-me 需求访谈助手。你正在和用户进行自由对话，帮助梳理项目需求。

你可以使用以下工具：

1. create_batch(questions: Question[])
   - 当你需要确认特定细节时调用
   - 每个问题包含: type (choice/multi/text), question, options (可选), rationale
   - 一次可以生成 1-5 个问题
   - 问题会以卡片形式嵌入在你的回复中

2. finish_interview()
   - 当你认为需求已经充分讨论时调用
   - 会触发总结报告生成

对话原则：
- 自然对话，不要像考试一样连续提问
- 用户描述一个话题后，判断哪些细节需要结构化确认，调用 create_batch
- 不要每轮都生成问题，有些信息用户已经在对话中说清楚了
- 当所有关键需求都已确认，调用 finish_interview
```

### API 调用流程

```
用户发送消息
  → 构造 messages 数组 (历史 + 当前)
  → 调用 LLM chat/completions with tools
  → 流式输出 text 部分 (打字机效果)
  → 流结束后处理 tool_calls:
    → create_batch: 解析问题，存入 questions 表，关联 message_id，emit 事件
    → finish_interview: 触发会话结束流程
  → 保存 assistant message 到 messages 表
```

### 流式 + tool_call 处理

OpenAI streaming API 中，`tool_calls` 在流的最后几个 chunk 中返回。
处理方式：
1. 流式接收 chunks，text 部分实时追加到 UI
2. 累积 tool_calls chunks（delta 拼接）
3. 流结束后，解析完整的 tool_calls JSON
4. 执行 tool 对应的后端逻辑

## 后端改动

### 新增 SchedulerMsg

```rust
SendMessage {
    session_id: String,
    content: String,         // 用户输入
    app_handle: AppHandle,
    reply: oneshot::Sender<Result<(), String>>,
},
GetMessages {
    session_id: String,
    reply: oneshot::Sender<Result<Vec<ChatMessage>, String>>,
},
```

### 新增 Model

```rust
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,          // "user" | "assistant"
    pub content: String,
    pub question_ids: Vec<String>,
    pub created_at: String,
}
```

### 新增 Store 方法

```rust
fn insert_message(&self, msg: &ChatMessage) -> Result<()>;
fn get_messages(&self, session_id: &str) -> Result<Vec<ChatMessage>>;
fn add_message_id_to_question(&self, question_id: &str, message_id: &str) -> Result<()>;
```

### 新增 Command

```rust
#[tauri::command]
pub async fn send_message(session_id: String, content: String, ...) -> Result<(), String>;

#[tauri::command]
pub async fn get_messages(session_id: String, ...) -> Result<Vec<ChatMessage>, String>;
```

### LLM Client 改动

`openai.rs` 新增方法：

```rust
/// Stream a chat completion with tool calling support.
/// Calls on_text for each text delta, returns complete tool_calls at the end.
async fn chat_with_tools(
    &self,
    messages: &[ApiMessage],
    tools: &[ApiTool],
    on_text: Box<dyn FnMut(&str) + Send>,
) -> Result<(String, Vec<ToolCall>), LlmError>;
```

### handle_send_message 逻辑

```
1. 保存用户消息到 messages 表
2. 加载历史 messages → 构造 API messages 数组
3. 构造 tools 定义 (create_batch, finish_interview)
4. 调用 chat_with_tools，流式 emit text 到前端 (event: chat_stream)
5. 流结束后:
   a. 保存 assistant 消息到 messages 表
   b. 处理 tool_calls:
      - create_batch: 解析问题 → insert_question (带 message_id) → emit new_question 事件
      - finish_interview: 调用 handle_finish_session
   c. emit chat_message_done 事件 (前端停止打字机)
```

## 前端改动

### 新增组件

| 组件 | 说明 |
|---|---|
| `ChatView` | 主聊天界面，消息列表 + 输入框 |
| `ChatMessage` | 单条消息渲染（用户/AI 不同样式） |
| `InlineQuestionCard` | 嵌入在 AI 消息中的问题卡片，可展开答题 |
| `QuestionDrawer` | 右侧抽屉，显示所有问题列表，可折叠 |
| `DrawerQuestionItem` | 抽屉中的问题项，可直接展开答题 |

### 修改组件

| 组件 | 改动 |
|---|---|
| `App.tsx` | 主界面从 QuestionList 替换为 ChatView + QuestionDrawer |
| `QuestionList.tsx` | 删除（功能合并到 QuestionDrawer） |
| `QuestionCard.tsx` | 重构为 InlineQuestionCard + DrawerQuestionItem 共用 |
| `useSessionEvents.ts` | 新增 chat_stream / chat_message_done 事件监听 |

### 新增 Store

```typescript
// chatStore.ts
interface ChatState {
  messages: ChatMessage[];
  streamingText: string;     // 当前流式输出的文本
  isStreaming: boolean;
  loadMessages: (sessionId: string) => Promise<void>;
  addMessage: (msg: ChatMessage) => void;
  appendStreamText: (delta: string) => void;
  finishStream: () => void;
}
```

### 事件流

| 事件 | 方向 | 数据 | 说明 |
|---|---|---|---|
| `chat_stream` | BE→FE | `{ session_id, delta }` | 流式文本片段 |
| `chat_message_done` | BE→FE | `{ session_id, message }` | 完整的 assistant 消息 |
| `new_question` | BE→FE | `{ session_id, question }` | 新问题生成（现有，保留） |
| `interview_may_complete` | BE→FE | `{ session_id }` | AI 建议结束（现有，保留） |

### ChatView 组件结构

```tsx
function ChatView() {
  const { messages, streamingText, isStreaming } = useChatStore();
  const { questions } = useSessionStore();

  return (
    <div className="flex h-full flex-col">
      <ScrollArea className="flex-1">
        {messages.map(msg => (
          <ChatMessage key={msg.id} message={msg} questions={questions} />
        ))}
        {isStreaming && <StreamingMessage text={streamingText} />}
      </ScrollArea>
      <ChatInput onSend={handleSend} disabled={isStreaming} />
    </div>
  );
}
```

### ChatMessage 渲染

```tsx
function ChatMessage({ message, questions }) {
  const embeddedQuestions = questions.filter(q => message.question_ids.includes(q.id));
  return (
    <div className={message.role === 'user' ? 'user-bubble' : 'ai-bubble'}>
      <ReactMarkdown>{message.content}</ReactMarkdown>
      {embeddedQuestions.map(q => (
        <InlineQuestionCard key={q.id} question={q} />
      ))}
    </div>
  );
}
```

### InlineQuestionCard

```tsx
function InlineQuestionCard({ question }) {
  const [expanded, setExpanded] = useState(false);
  // 复用现有 QuestionCard 的答题逻辑
  if (question.status === 'answered') {
    return <CollapsedAnsweredCard question={question} />;
  }
  if (!expanded) {
    return <CollapsedCard question={question} onExpand={() => setExpanded(true)} />;
  }
  return <ExpandedCard question={question} onAnswer={...} onSkip={...} />;
}
```

### QuestionDrawer

```tsx
function QuestionDrawer({ open, onToggle }) {
  const { questions } = useSessionStore();
  const pending = questions.filter(q => q.status === 'ready');
  const answered = questions.filter(q => q.status === 'answered');
  const skipped = questions.filter(q => q.status === 'skipped');

  return (
    <Sheet open={open} onOpenChange={onToggle} side="right">
      <div className="flex flex-col gap-4 p-4">
        <Section title="待答" count={pending.length}>
          {pending.map(q => <DrawerQuestionItem key={q.id} question={q} />)}
        </Section>
        <Section title="已答" count={answered.length}>
          {answered.map(q => <DrawerQuestionItem key={q.id} question={q} readonly />)}
        </Section>
        <Section title="跳过" count={skipped.length}>
          {skipped.map(q => <DrawerQuestionItem key={q.id} question={q} readonly />)}
        </Section>
      </div>
    </Sheet>
  );
}
```

## 兼容性处理

- 老会话（纯问答模式创建）：加载时如果没有 messages，显示空 chat + 已有问题在抽屉中
- 新会话：创建后 AI 自动发送第一条欢迎消息（调用 LLM 生成开场白）
- 完成访谈、原型生成、导出等功能不变

## 实现顺序

1. DB: messages 表 + questions.message_id 列 + migration
2. Model: ChatMessage 结构体
3. Store: insert_message / get_messages / add_message_id_to_question
4. LLM: chat_with_tools 方法 (流式 + tool_calls)
5. Prompt: chat system prompt + tools 定义
6. Scheduler: SendMessage / GetMessages 消息处理 + handle_send_message
7. Commands: send_message / get_messages
8. 前端 Store: chatStore.ts
9. 前端 Events: chat_stream / chat_message_done 监听
10. 前端组件: ChatView / ChatMessage / InlineQuestionCard / QuestionDrawer
11. App.tsx: 替换主界面
12. 删除 QuestionList.tsx
13. 编译验证 + 运行

## 回归检查清单

- [ ] 新建会话 → AI 发送欢迎消息
- [ ] 用户发送消息 → AI 流式回复
- [ ] AI 回复中包含结构化问题卡片
- [ ] 卡片可展开答题，答完显示已答状态
- [ ] 卡片可跳过
- [ ] 抽屉显示所有问题，按状态分组
- [ ] 抽屉内可直接答题
- [ ] 抽屉答题后对话中卡片状态同步
- [ ] AI 建议结束 → 显示提示
- [ ] 用户手动完成访谈 → 生成总结
- [ ] 切换会话 → 加载对应 chat 历史
- [ ] 重启应用 → chat 历史恢复
- [ ] 原型生成功能正常
- [ ] 导出功能正常
- [ ] 老会话（无 messages）兼容显示
