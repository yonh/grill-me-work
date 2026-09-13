/// Built-in design system injected into prototype prompts to reduce "AI flavor".
/// This provides the LLM with a concrete CSS foundation, component patterns,
/// and anti-patterns to avoid, so generated prototypes look like real products
/// rather than generic AI output.

/// The design system CSS that the LLM should include as a base in <style>.
/// Uses CSS variables for easy theming, modern spacing scale, and real
/// component patterns.
pub const DESIGN_SYSTEM_CSS: &str = r#"
/* === Design System Base === */
:root {
  /* Color palette — use these variables, do NOT invent new colors */
  --c-bg: #f7f8fa;
  --c-surface: #ffffff;
  --c-surface-2: #f0f2f5;
  --c-border: #e4e7eb;
  --c-border-strong: #cbd2d9;
  --c-text: #1a1f36;
  --c-text-2: #5c6478;
  --c-text-3: #8a92a6;
  --c-primary: #2563eb;
  --c-primary-hover: #1d4ed8;
  --c-primary-light: #eff6ff;
  --c-success: #16a34a;
  --c-warning: #d97706;
  --c-danger: #dc2626;

  /* Spacing scale — 4px base */
  --sp-1: 4px; --sp-2: 8px; --sp-3: 12px; --sp-4: 16px;
  --sp-5: 20px; --sp-6: 24px; --sp-8: 32px; --sp-10: 40px;

  /* Radius */
  --r-sm: 6px; --r-md: 8px; --r-lg: 12px;

  /* Shadows — subtle, not glowing */
  --sh-sm: 0 1px 2px rgba(0,0,0,0.05);
  --sh-md: 0 2px 8px rgba(0,0,0,0.08);
  --sh-lg: 0 8px 24px rgba(0,0,0,0.12);

  /* Typography */
  --fs-xs: 12px; --fs-sm: 13px; --fs-base: 14px;
  --fs-lg: 16px; --fs-xl: 20px; --fs-2xl: 28px;
  --lh: 1.6;
}

* { margin: 0; padding: 0; box-sizing: border-box; }

body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC",
    "Hiragino Sans GB", "Microsoft YaHei", sans-serif;
  font-size: var(--fs-base);
  line-height: var(--lh);
  color: var(--c-text);
  background: var(--c-bg);
  -webkit-font-smoothing: antialiased;
}

/* Layout primitives */
.app-layout { display: flex; min-height: 100vh; }
.app-sidebar { width: 240px; background: var(--c-surface); border-right: 1px solid var(--c-border); flex-shrink: 0; }
.app-main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.app-header { height: 56px; background: var(--c-surface); border-bottom: 1px solid var(--c-border); display: flex; align-items: center; padding: 0 var(--sp-6); gap: var(--sp-4); }
.app-content { flex: 1; padding: var(--sp-6); overflow-y: auto; }

/* Components */
.btn {
  display: inline-flex; align-items: center; gap: var(--sp-2);
  padding: var(--sp-2) var(--sp-4); border-radius: var(--r-sm);
  font-size: var(--fs-base); font-weight: 500; cursor: pointer;
  border: 1px solid transparent; transition: all 0.15s; white-space: nowrap;
}
.btn-primary { background: var(--c-primary); color: #fff; }
.btn-primary:hover { background: var(--c-primary-hover); }
.btn-ghost { background: transparent; color: var(--c-text-2); border-color: var(--c-border); }
.btn-ghost:hover { background: var(--c-surface-2); }

.card {
  background: var(--c-surface); border: 1px solid var(--c-border);
  border-radius: var(--r-md); box-shadow: var(--sh-sm);
}
.card-header { padding: var(--sp-4) var(--sp-5); border-bottom: 1px solid var(--c-border); font-weight: 600; font-size: var(--fs-lg); }
.card-body { padding: var(--sp-5); }

.input {
  width: 100%; padding: var(--sp-2) var(--sp-3); border: 1px solid var(--c-border);
  border-radius: var(--r-sm); font-size: var(--fs-base); background: var(--c-surface);
  transition: border-color 0.15s;
}
.input:focus { outline: none; border-color: var(--c-primary); box-shadow: 0 0 0 3px var(--c-primary-light); }

.badge {
  display: inline-flex; align-items: center; padding: 2px var(--sp-2);
  border-radius: 4px; font-size: var(--fs-xs); font-weight: 500;
}
.badge-success { background: #dcfce7; color: var(--c-success); }
.badge-warning { background: #fef3c7; color: var(--c-warning); }
.badge-danger { background: #fee2e2; color: var(--c-danger); }

/* Table */
.table { width: 100%; border-collapse: collapse; font-size: var(--fs-sm); }
.table th { text-align: left; padding: var(--sp-3) var(--sp-4); background: var(--c-surface-2); font-weight: 600; color: var(--c-text-2); border-bottom: 1px solid var(--c-border); }
.table td { padding: var(--sp-3) var(--sp-4); border-bottom: 1px solid var(--c-border); }
.table tr:hover td { background: var(--c-surface-2); }

/* Nav items */
.nav-item {
  display: flex; align-items: center; gap: var(--sp-3);
  padding: var(--sp-2) var(--sp-4); border-radius: var(--r-sm);
  color: var(--c-text-2); cursor: pointer; font-size: var(--fs-base);
  transition: all 0.15s;
}
.nav-item:hover { background: var(--c-surface-2); color: var(--c-text); }
.nav-item.active { background: var(--c-primary-light); color: var(--c-primary); font-weight: 500; }
"#;

/// Anti-pattern rules — explicit instructions on what NOT to do
pub const ANTI_PATTERNS: &str = r#"
反模式（禁止以下行为）：
1. 禁止使用紫色渐变、彩虹渐变背景、glow 发光效果
2. 禁止使用 Lorem Ipsum 占位文字 — 使用符合业务场景的真实中文占位数据
3. 禁止过度对称的 3/4 列等高卡片网格 — 根据真实信息层级布局
4. 禁止使用 emoji 作为图标 — 用 CSS 或 SVG 内联图标代替
5. 禁止使用 "Welcome to..." 式的空泛标题 — 用具体的产品名称和功能描述
6. 禁止所有元素都居中 — 真实产品有明确的左对齐信息层级
7. 禁止使用纯黑(#000)或纯白(#fff)做大面积背景 — 用设计系统的 --c-bg / --c-surface
8. 禁止圆角过大（>16px）— 使用设计系统的 --r-sm/md/lg
"#;

/// Build the full design system instruction block for the prompt.
pub fn build_design_system_directive() -> String {
    format!(
        "## 内置设计系统\n\
你必须使用以下 CSS 作为基础样式（放在 <style> 标签最前面），在此之上构建原型。\n\
可以使用设计系统中的 CSS 变量（--c-*, --sp-*, --r-*, --sh-*, --fs-*）来保持一致性。\n\
可以添加额外的组件样式，但必须与设计系统的色彩和间距保持一致。\n\n\
```css{css}```\n\n\
{anti_patterns}",
        css = DESIGN_SYSTEM_CSS,
        anti_patterns = ANTI_PATTERNS
    )
}
