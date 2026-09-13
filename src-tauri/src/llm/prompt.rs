use crate::model::schema::QUESTION_JSON_TEMPLATE;
use crate::model::{DecisionEntry, Question, Session};
use crate::llm::openai::{ApiTool, ApiFunction};

/// Build the role-specific directive that guides question direction and depth.
fn build_role_directive(role: &str) -> String {
    match role {
        "pm" => "你是面向产品经理/需求方的问题生成器。\
重点关注：系统功能边界、用户故事、权限模型、数据流向、业务流程、验收标准。\
避免问技术选型、架构实现等开发细节。问题应帮助需求方厘清「要什么」而非「怎么做」。".to_string(),
        "dev" => "你是面向开发者/技术负责人的问题生成器。\
重点关注：技术栈选型、架构设计、数据模型、API 设计、性能约束。\
可以深入技术细节，假设用户有技术背景。".to_string(),
        "designer" => "你是面向设计师的问题生成器。\
重点关注：交互流程、界面形态、用户路径、视觉风格、可访问性。\
避免问后端架构和数据库设计。".to_string(),
        "manager" => "你是面向项目经理的问题生成器。\
重点关注：范围边界、优先级、时间约束、风险、依赖关系、资源分配。\
保持全局视角，避免深入具体技术或设计细节。".to_string(),
        custom => format!(
            "你是面向以下角色的问题生成器：{custom}。\
根据该角色的关注点和专业领域，调整问题的方向和深度。",
            custom = custom
        ),
    }
}

/// Role-specific cold start dimensions, replacing the generic 6-dimension template.
fn build_role_cold_start_dimensions(role: &str) -> String {
    match role {
        "pm" => "1. 目标用户 — 谁使用？核心使用场景是什么？\n\
2. 核心功能 — 系统能做什么？功能边界在哪里？\n\
3. 权限模型 — 有哪些角色？各自能做什么？\n\
4. 数据要求 — 核心数据实体有哪些？数据如何流动？\n\
5. 业务流程 — 关键业务流程是什么？\n\
6. 验收标准 — 怎么算完成？非功能需求有哪些？\n",
        "dev" => "1. 问题域与目标 — 要解决什么问题？给谁用？\n\
2. 技术栈选型 — 语言/框架/运行时\n\
3. 架构设计 — 模块划分、数据流\n\
4. 数据模型 — 核心实体、存储方案\n\
5. API 设计 — 关键接口、通信协议\n\
6. 性能约束 — 吞吐量、延迟、并发要求\n",
        "designer" => "1. 用户画像 — 主要用户是谁？使用场景是什么？\n\
2. 核心场景 — 用户要完成什么任务？\n\
3. 界面形态 — 平台、布局方向、响应式策略\n\
4. 交互流程 — 核心用户流程是什么？\n\
5. 视觉风格 — 风格关键词、色彩倾向、参考\n\
6. 可访问性 — 有哪些可访问性要求？\n",
        "manager" => "1. 项目目标 — 要达成什么？成功标准是什么？\n\
2. 范围边界 — 包含什么、不包含什么？\n\
3. 优先级 — 哪些是必须的？哪些可以延后？\n\
4. 时间约束 — 有没有截止日期？里程碑是什么？\n\
5. 风险与依赖 — 有哪些已知风险和外部依赖？\n\
6. 资源评估 — 需要多少人？什么技能？\n",
        _ => "1. 问题域与目标 — 要解决什么问题？给谁用？\n\
2. 技术栈选型 — 语言/框架/运行时\n\
3. 架构设计 — 模块划分、数据流\n\
4. 数据与持久化 — 存什么、怎么存\n\
5. 交互与 UX — 界面形态、核心交互\n\
6. 约束与边界 — 性能要求、平台、时间\n",
    }
    .to_string()
}

/// Build the system prompt for question generation.
pub fn build_system_prompt(batch_size: i32, role: &str) -> String {
    format!(
        r#"{role_directive}

你是 grill-me 需求访谈器。基于用户已确认的决策，生成下一批问题。
规则：
1. 每个问题用 <question> 标签包裹，标签内是一个 JSON 对象
2. 一次生成 {batch_size} 个问题
3. 基于已确认决策调整后续问题方向，不要重复已问过的
4. 每题提供 recommended_option 和 rationale
5. 问题按依赖排序，独立的在前
6. 不要问代码库能回答的问题
7. 标签外可以写解释性文字，会被忽略
8. JSON 必须在单个 <question> 标签内闭合

JSON Schema:
{schema}"#,
        role_directive = build_role_directive(role),
        batch_size = batch_size,
        schema = QUESTION_JSON_TEMPLATE,
    )
}

/// Build the user prompt for a batch.
/// If `answered_questions` is empty, this is a cold start — include role-specific dimensions.
/// Otherwise, include the decision summary and last 3 raw answers.
pub fn build_user_prompt(
    session: &Session,
    decision_summary: &[DecisionEntry],
    recent_answers: &[Question],
    batch_size: i32,
) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!("项目主题：{}\n", session.title));

    if let Some(ctx) = &session.initial_context {
        if !ctx.is_empty() {
            prompt.push_str(&format!("用户提供的初始上下文：{}\n\n", ctx));
        }
    }

    if decision_summary.is_empty() && recent_answers.is_empty() {
        // Cold start — include role-specific dimensions
        prompt.push_str("建议覆盖维度：\n");
        prompt.push_str(&build_role_cold_start_dimensions(&session.role));
        prompt.push('\n');
    } else {
        // Incremental — include decision summary
        if !decision_summary.is_empty() {
            prompt.push_str("已确认的决策：\n");
            for entry in decision_summary {
                let rationale = entry
                    .rationale
                    .as_ref()
                    .map(|r| format!("（{}）", r))
                    .unwrap_or_default();
                prompt.push_str(&format!(
                    "- {}: {}{}\n",
                    entry.question, entry.answer, rationale
                ));
            }
            prompt.push('\n');
        }

        // Last 3 raw answers
        if !recent_answers.is_empty() {
            prompt.push_str("最近回答：\n");
            for q in recent_answers.iter().rev().take(3) {
                let answer = q
                    .answer
                    .as_ref()
                    .map(|a| a.to_human_string())
                    .unwrap_or_default();
                prompt.push_str(&format!("Q: \"{}\" → A: \"{}\"\n", q.question, answer));
            }
            prompt.push('\n');
        }
    }

    prompt.push_str(&format!("请生成下一批 {} 个问题。\n", batch_size));

    prompt
}

/// Build the system prompt for the session summary generation.
pub fn build_summary_system_prompt(role: &str) -> String {
    let role_directive = build_role_directive(role);
    let report_template = build_role_report_template(role);
    format!(
        "{role_directive}\n\n\
你是 grill-me 需求访谈总结器。基于用户已确认的所有决策，按以下模板生成报告：\n\n\
{report_template}\n\n\
用 Markdown 格式输出。如果某些章节信息不足，标注「待补充」而非编造。"
    )
}

/// Role-specific report templates.
fn build_role_report_template(role: &str) -> String {
    match role {
        "pm" => r#"# {项目标题} 需求规格

## 1. 项目概述
（一段话描述项目目标和价值）

## 2. 目标用户
- 主要用户角色
- 核心使用场景

## 3. 功能清单
| 功能 | 描述 | 优先级 | 验收标准 |
|---|---|---|---|

## 4. 权限模型
（角色定义 + 权限矩阵）

## 5. 数据要求
（核心数据实体 + 数据流向 + 存储要求）

## 6. 业务流程
（关键流程的文字描述）

## 7. 约束与非功能需求
（性能、安全、合规等）

## 8. 待定问题与风险"#,
        "dev" => r#"# {项目标题} 技术方案

## 1. 技术选型
| 层级 | 选型 | 理由 |
|---|---|---|

## 2. 架构设计
（模块划分 + 数据流）

## 3. 数据模型
（核心实体 + 关系 + 存储方案）

## 4. API 设计
（关键接口定义）

## 5. 关键技术决策
| 决策点 | 方案 | 权衡 |
|---|---|---|

## 6. 性能与约束

## 7. 技术风险与对策"#,
        "designer" => r#"# {项目标题} 设计简报

## 1. 用户画像与场景

## 2. 信息架构
（页面/模块层级结构）

## 3. 核心用户流程

## 4. 界面形态
（平台、布局方向、响应式策略）

## 5. 视觉方向
（风格关键词、色彩倾向、参考）

## 6. 组件需求清单

## 7. 可访问性要求

## 8. 设计待定问题"#,
        "manager" => r#"# {项目标题} 项目计划

## 1. 项目目标与范围

## 2. 范围边界
（包含 / 不包含）

## 3. 里程碑
| 阶段 | 交付物 | 预估周期 |
|---|---|---|

## 4. 优先级矩阵
| 功能/任务 | 优先级 | 依赖 |
|---|---|---|

## 5. 风险清单
| 风险 | 影响 | 缓解措施 |
|---|---|---|

## 6. 资源需求

## 7. 待定决策"#,
        _ => r#"# {项目标题} 需求总结

## 1. 项目概述
（一段话描述项目目标和价值）

## 2. 关键决策概览
| 决策点 | 结论 | 理由 |
|---|---|---|

## 3. 已确定的核心需求

## 4. 潜在风险与未解决问题

## 5. 建议的后续步骤

请根据该角色的关注点侧重报告内容。"#,
    }
    .to_string()
}

/// Build the user prompt for the session summary generation.
pub fn build_summary_user_prompt(session: &Session, decision_summary: &[DecisionEntry]) -> String {
    let mut prompt = String::new();
    prompt.push_str(&format!("项目主题：{}\n\n", session.title));

    if let Some(ctx) = &session.initial_context {
        if !ctx.is_empty() {
            prompt.push_str(&format!("初始上下文：{}\n\n", ctx));
        }
    }

    prompt.push_str("已确认的决策：\n");
    for entry in decision_summary {
        let rationale = entry
            .rationale
            .as_ref()
            .map(|r| format!("（原因：{}）", r))
            .unwrap_or_default();
        prompt.push_str(&format!(
            "- {}: {}{}\n",
            entry.question, entry.answer, rationale
        ));
    }

    prompt.push_str("\n请生成总结。");
    prompt
}

/// Build the system prompt for multi-file prototype generation (v2).
pub fn build_prototype_system_prompt(role: &str) -> String {
    let role_directive = build_role_directive(role);
    let design_system = crate::llm::design_system::build_design_system_directive();
    format!(
        "{role_directive}\n\n\
你是 grill-me-v2 原型生成器。基于用户需求访谈的决策，生成或增量更新一个**多文件静态 HTML 原型**。\n\
你是一个资深前端工程师，原型应看起来像真实产品而非 AI 演示。\n\n\
## 输出格式（严格 JSON，不要 markdown 代码块）\n\
{{\n\
  \"changelog\": \"本次改动说明（中文，一两句）\",\n\
  \"delete_paths\": [\"可选：需要删除的相对路径\"],\n\
  \"files\": [\n\
    {{\"path\": \"index.html\", \"content\": \"完整文件内容\"}},\n\
    {{\"path\": \"css/styles.css\", \"content\": \"...\"}},\n\
    {{\"path\": \"js/app.js\", \"content\": \"...\"}},\n\
    {{\"path\": \"pages/detail.html\", \"content\": \"...\"}}\n\
  ]\n\
}}\n\n\
## 目录约定\n\
- 入口固定为 `index.html`\n\
- 样式放 `css/styles.css`（可再拆 `css/*.css`）\n\
- 逻辑放 `js/app.js`（可再拆 `js/*.js`）\n\
- 次级页面放 `pages/*.html`，用相对路径互相引用\n\
- 不要使用外部 CDN / 字体 / 图片 URL；图标用内联 SVG 或 CSS\n\
- 不要生成构建工具、package.json、框架代码\n\n\
## 增量更新规则\n\
- 若提供了「当前原型文件」，只重写**需要变更**的文件内容（在 files 里给出完整新内容）\n\
- 未变更的文件不要放进 files（避免无意义重写）\n\
- 首次生成时 files 必须包含完整可运行的入口与资源\n\
- 删除不再需要的页面时放进 delete_paths\n\n\
## 质量要求\n\
1. 使用设计系统中的 CSS 变量与组件风格\n\
2. 占位数据必须符合业务场景（中文），不要 Lorem Ipsum\n\
3. 布局完整：导航/侧栏/主区/关键操作，像可点击的产品原型\n\
4. 交互用原生 JS 即可（切换页签、筛选、弹层等）\n\
5. 只输出 JSON 对象本身\n\n\
{design_system}"
    )
}

/// Build the user prompt for multi-file prototype generation/update.
pub fn build_prototype_user_prompt(
    session: &Session,
    decision_summary: &[DecisionEntry],
    current_files: &[crate::prototype::PrototypeFile],
    feedback: Option<&str>,
    trigger_note: Option<&str>,
) -> String {
    let mut prompt = String::new();
    prompt.push_str(&format!("项目主题：{}\n\n", session.title));

    if let Some(ctx) = &session.initial_context {
        if !ctx.is_empty() {
            prompt.push_str(&format!("初始上下文：{}\n\n", ctx));
        }
    }

    prompt.push_str("已确认的访谈决策（按时间顺序）：\n");
    if decision_summary.is_empty() {
        prompt.push_str("（暂无）\n");
    }
    for entry in decision_summary {
        prompt.push_str(&format!("- {}: {}\n", entry.question, entry.answer));
    }
    prompt.push('\n');

    if let Some(note) = trigger_note {
        if !note.is_empty() {
            prompt.push_str(&format!("本次触发原因：{}\n\n", note));
        }
    }

    if let Some(fb) = feedback {
        if !fb.is_empty() {
            prompt.push_str("用户对当前原型的修改意见：\n");
            prompt.push_str(fb);
            prompt.push_str("\n\n");
        }
    }

    if current_files.is_empty() {
        prompt.push_str("当前尚无原型文件，请生成完整的多文件初版。\n");
    } else {
        prompt.push_str("当前原型文件如下（path + content）：\n");
        for f in current_files {
            // Cap each file to keep prompt size reasonable
            let max = 12000usize;
            let content = if f.content.len() > max {
                &f.content[..max]
            } else {
                f.content.as_str()
            };
            prompt.push_str(&format!("\n===== FILE: {} =====\n{}\n", f.path, content));
        }
        prompt.push_str("\n请根据访谈决策与触发原因，增量更新原型。只返回需要变更/新增/删除的文件。\n");
    }

    prompt
}

/// Build the system prompt for chat mode.
pub fn build_chat_system_prompt(session: &Session) -> String {
    let role_directive = build_role_directive(&session.role);
    format!(
        "{role_directive}\n\n\
你是 grill-me 需求访谈助手。你正在和用户进行自由对话，帮助梳理项目需求。\n\n\
你可以使用以下工具：\n\n\
1. create_batch — 当你需要确认特定细节时调用，生成结构化问题\n\
   - 每个问题包含: type (choice/multi/text), question, options (choice/multi 类型需要), rationale\n\
   - 一次可以生成 1-5 个问题\n\
   - 问题会以卡片形式嵌入在你的回复下方，用户点击卡片展开答题\n\n\
2. finish_interview — 当你认为需求已经充分讨论时调用\n\
   - 会触发总结报告生成\n\n\
对话原则：\n\
- 自然对话，不要像考试一样连续提问\n\
- 用户描述一个话题后，判断哪些细节需要结构化确认，调用 create_batch\n\
- 不要每轮都生成问题，有些信息用户已经在对话中说清楚了就不需要再问\n\
- 先用文字回复用户，然后在需要时调用工具\n\
- 当所有关键需求都已确认，调用 finish_interview\n\n\
项目主题：{title}\n\
初始上下文：{context}",
        role_directive = role_directive,
        title = session.title,
        context = session.initial_context.as_deref().unwrap_or("无"),
    )
}

/// Build the tools definition for chat mode.
pub fn build_chat_tools() -> Vec<ApiTool> {
    vec![
        ApiTool {
            tool_type: "function".to_string(),
            function: ApiFunction {
                name: "create_batch".to_string(),
                description: "生成结构化问题供用户确认细节。当需要确认特定需求细节时调用。".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "questions": {
                            "type": "array",
                            "description": "要生成的结构化问题列表",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "type": {
                                        "type": "string",
                                        "enum": ["choice", "multi", "text"],
                                        "description": "问题类型：choice=单选, multi=多选, text=文本"
                                    },
                                    "question": {
                                        "type": "string",
                                        "description": "问题文本"
                                    },
                                    "options": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "label": {"type": "string"},
                                                "description": {"type": "string"}
                                            },
                                            "required": ["label"]
                                        },
                                        "description": "选项列表（choice/multi 类型必填）"
                                    },
                                    "rationale": {
                                        "type": "string",
                                        "description": "为什么要问这个问题的理由"
                                    }
                                },
                                "required": ["type", "question"]
                            }
                        }
                    },
                    "required": ["questions"]
                }),
            },
        },
        ApiTool {
            tool_type: "function".to_string(),
            function: ApiFunction {
                name: "finish_interview".to_string(),
                description: "当认为需求已经充分讨论时调用，触发会话结束和总结报告生成。".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        },
    ]
}


