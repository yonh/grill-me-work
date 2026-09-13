//! Session workspace layout under `~/.grill-work/<session_id>/`.
//!
//! ```text
//! ~/.grill-work/
//! ├── grill-me-v2.db
//! └── <session_id>/
//!     ├── docs/           # structured spec (decisions.md, …)
//!     ├── prototype/      # static site; coding agent edits only here
//!     ├── .git/           # decision tree at workspace root
//!     └── .iteration-graph.json
//! ```

use std::path::{Path, PathBuf};

/// Root for all workspaces + app DB. Visible and easy to open.
pub fn grill_work_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let root = PathBuf::from(home).join(".grill-work");
    std::fs::create_dir_all(&root).ok();
    root
}

pub fn db_path() -> PathBuf {
    grill_work_root().join("grill-me-v2.db")
}

fn safe_id(session_id: &str) -> String {
    session_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Absolute workspace root for a session (contains docs/, prototype/, .git).
pub fn session_workspace_dir(session_id: &str) -> PathBuf {
    grill_work_root().join(safe_id(session_id))
}

/// Coding-agent / preview / static-site directory.
pub fn session_prototype_dir(session_id: &str) -> PathBuf {
    session_workspace_dir(session_id).join("prototype")
}

pub fn session_docs_dir(session_id: &str) -> PathBuf {
    session_workspace_dir(session_id).join("docs")
}

pub fn iteration_graph_path(session_id: &str) -> PathBuf {
    session_workspace_dir(session_id).join(".iteration-graph.json")
}

/// Create workspace skeleton. Migrates a legacy flat prototype dir if present.
pub fn ensure_workspace(session_id: &str) -> std::io::Result<PathBuf> {
    let ws = session_workspace_dir(session_id);
    let proto = ws.join("prototype");
    let docs = ws.join("docs");
    std::fs::create_dir_all(&proto)?;
    std::fs::create_dir_all(&docs)?;

    // Migrate legacy: Application Support/grill-me-v2/prototypes/<sid>/*
    let legacy = legacy_prototype_dir(session_id);
    if legacy.exists() && legacy.join("index.html").exists() && !proto.join("index.html").exists() {
        log::info!(
            "[workspace] migrating legacy prototype {} → {}",
            legacy.display(),
            proto.display()
        );
        copy_dir_recursive(&legacy, &proto)?;
    }

    Ok(ws)
}

fn legacy_app_root() -> PathBuf {
    let dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.join("grill-me-v2")
}

fn legacy_prototype_dir(session_id: &str) -> PathBuf {
    legacy_app_root().join("prototypes").join(safe_id(session_id))
}

fn sqlite_session_count(path: &Path) -> usize {
    let Ok(conn) = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return 0;
    };
    conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get::<_, i64>(0))
        .unwrap_or(0)
        .max(0) as usize
}

fn remove_sqlite_sidecars(db: &Path) {
    for ext in ["", "-wal", "-shm"] {
        let mut p = db.as_os_str().to_os_string();
        p.push(ext);
        let _ = std::fs::remove_file(PathBuf::from(p));
    }
}

/// Copy old DB over new when new is missing or empty (0 sessions).
fn migrate_legacy_db() -> Result<bool, String> {
    let old_db = legacy_app_root().join("grill-me-v2.db");
    if !old_db.exists() {
        return Ok(false);
    }
    let old_count = sqlite_session_count(&old_db);
    if old_count == 0 {
        return Ok(false);
    }
    let new_db = db_path();
    let new_count = if new_db.exists() {
        sqlite_session_count(&new_db)
    } else {
        0
    };
    if new_count > 0 {
        log::info!("[migrate] new DB already has {new_count} sessions; skip DB copy");
        return Ok(false);
    }

    log::info!(
        "[migrate] copying legacy DB ({} sessions) {} → {}",
        old_count,
        old_db.display(),
        new_db.display()
    );

    // Prefer SQLite backup API so WAL is checkpointed into one file.
    let result = (|| -> Result<(), String> {
        let src = rusqlite::Connection::open_with_flags(
            &old_db,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| format!("open old db: {e}"))?;
        if new_db.exists() {
            remove_sqlite_sidecars(&new_db);
        }
        if let Some(parent) = new_db.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut dst =
            rusqlite::Connection::open(&new_db).map_err(|e| format!("open new db: {e}"))?;
        let mut backup = rusqlite::backup::Backup::new(&src, &mut dst)
            .map_err(|e| format!("backup init: {e}"))?;
        backup
            .run_to_completion(100, std::time::Duration::from_millis(5), None)
            .map_err(|e| format!("backup: {e}"))?;
        drop(backup);
        Ok(())
    })();

    match result {
        Ok(()) => Ok(true),
        Err(e) => {
            log::warn!("[migrate] backup API failed ({e}); falling back to file copy");
            remove_sqlite_sidecars(&new_db);
            std::fs::copy(&old_db, &new_db).map_err(|e| format!("copy db: {e}"))?;
            for ext in ["-wal", "-shm"] {
                let src = PathBuf::from(format!("{}{ext}", old_db.display()));
                if src.exists() {
                    let dst = PathBuf::from(format!("{}{ext}", new_db.display()));
                    let _ = std::fs::copy(&src, &dst);
                }
            }
            Ok(true)
        }
    }
}

/// Migrate every legacy prototype directory into `~/.grill-work/<sid>/prototype`.
fn migrate_legacy_prototypes() -> usize {
    let root = legacy_app_root().join("prototypes");
    let Ok(rd) = std::fs::read_dir(&root) else {
        return 0;
    };
    let mut n = 0;
    for entry in rd.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let sid = entry.file_name().to_string_lossy().to_string();
        let has_content = path.join("index.html").exists()
            || path.join("css").exists()
            || path.join("pages").exists();
        if !has_content {
            continue;
        }
        let proto = session_prototype_dir(&sid);
        if proto.join("index.html").exists() {
            continue;
        }
        match ensure_workspace(&sid) {
            Ok(_) => {
                if proto.join("index.html").exists() {
                    n += 1;
                    log::info!("[migrate] prototype {sid} → {}", proto.display());
                }
            }
            Err(e) => log::warn!("[migrate] prototype {sid} failed: {e}"),
        }
    }
    n
}

/// One-shot: pull DB + all prototypes from Application Support into `~/.grill-work`.
/// Safe to call multiple times; skips when destination already has data.
pub fn migrate_legacy_on_startup() {
    match migrate_legacy_db() {
        Ok(true) => log::info!("[migrate] legacy database migrated"),
        Ok(false) => {}
        Err(e) => log::error!("[migrate] database migration failed: {e}"),
    }
    let n = migrate_legacy_prototypes();
    if n > 0 {
        log::info!("[migrate] migrated {n} prototype workspace(s)");
    }
}

fn copy_dir_recursive(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
        let src = entry.path();
        let name = entry.file_name();
        let name_s = name.to_string_lossy().to_string();
        if name_s.starts_with('.') || name_s == "node_modules" || name_s == "TASK.md" {
            continue;
        }
        let dst = to.join(&name);
        if src.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else if src.is_file() {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Write `docs/decisions.md` from confirmed interview decisions.
/// `outline` is the user-confirmed interview outline — the scope contract.
pub fn write_decisions_md(
    session_id: &str,
    title: &str,
    role: &str,
    initial_context: Option<&str>,
    decisions: &[crate::model::DecisionEntry],
    outline: &[crate::model::OutlineNode],
) -> std::io::Result<PathBuf> {
    ensure_workspace(session_id)?;
    let docs = session_docs_dir(session_id);
    std::fs::create_dir_all(&docs)?;

    let mut md = String::new();
    md.push_str("# 决策索引\n\n");
    md.push_str(&format!("- **主题**: {title}\n"));
    md.push_str(&format!("- **角色视角**: {role}\n"));
    if let Some(ctx) = initial_context {
        if !ctx.trim().is_empty() {
            md.push_str(&format!("- **初始上下文**: {ctx}\n"));
        }
    }
    md.push_str(&format!(
        "- **更新时间**: {}\n\n",
        chrono::Utc::now().to_rfc3339()
    ));

    if !outline.is_empty() {
        md.push_str("## 访谈大纲（用户确认的范围）\n\n");
        for n in outline {
            let mark = match n.status {
                crate::model::OutlineNodeStatus::Covered => "x",
                crate::model::OutlineNodeStatus::Excluded => "-",
                crate::model::OutlineNodeStatus::Pending => " ",
            };
            let desc = n
                .description
                .as_deref()
                .map(|d| format!(" — {d}"))
                .unwrap_or_default();
            let suffix = match n.status {
                crate::model::OutlineNodeStatus::Excluded => "（已排除）",
                _ => "",
            };
            md.push_str(&format!("- [{mark}] {}{desc}{suffix}\n", n.title));
        }
        md.push('\n');
    }

    md.push_str("## 已确认决策\n\n");
    if decisions.is_empty() {
        md.push_str("（暂无）\n");
    }
    for d in decisions {
        md.push_str(&format!("### {}\n\n", d.question));
        md.push_str(&format!("- **回答**: {}\n", d.answer));
        md.push_str(&format!("- **分类**: {}\n", d.category.as_str()));
        if let Some(r) = &d.rationale {
            if !r.trim().is_empty() {
                md.push_str(&format!("- **理由**: {r}\n"));
            }
        }
        md.push_str(&format!("- **question_id**: `{}`\n\n", d.question_id));
    }

    let path = docs.join("decisions.md");
    std::fs::write(&path, md)?;
    Ok(path)
}

/// Short note file the agent can read for product intent (lightweight).
pub fn write_intent_md(session_id: &str, title: &str, role: &str, context: Option<&str>) -> std::io::Result<()> {
    ensure_workspace(session_id)?;
    let docs = session_docs_dir(session_id);
    let path = docs.join("intent.md");
    if path.exists() {
        return Ok(());
    }
    let mut md = String::new();
    md.push_str("# 产品意图\n\n");
    md.push_str(&format!("做一个与「{title}」相关的可交互静态 HTML 原型。\n\n"));
    md.push_str(&format!("目标用户视角：{role}\n"));
    if let Some(c) = context {
        if !c.trim().is_empty() {
            md.push_str(&format!("\n补充：{c}\n"));
        }
    }
    md.push_str("\n详细决策见 `docs/decisions.md`。代码只改 `prototype/`。\n");
    std::fs::write(path, md)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DecisionEntry, QuestionCategory};

    #[test]
    fn root_is_under_home_grill_work() {
        let root = grill_work_root();
        let s = root.to_string_lossy();
        assert!(s.ends_with(".grill-work") || s.contains("/.grill-work"));
    }

    #[test]
    fn workspace_layout() {
        let sid = "test-ws-layout";
        let ws = ensure_workspace(sid).unwrap();
        assert!(ws.join("prototype").is_dir());
        assert!(ws.join("docs").is_dir());
        let decisions = vec![DecisionEntry {
            question_id: "q1".into(),
            question: "目标用户？".into(),
            answer: "产品经理".into(),
            rationale: Some("主要使用场景".into()),
            category: QuestionCategory::Intent,
        }];
        let p = write_decisions_md(sid, "测试", "pm", Some("上下文"), &decisions, &[]).unwrap();
        let body = std::fs::read_to_string(p).unwrap();
        assert!(body.contains("目标用户"));
        assert!(body.contains("产品经理"));
        // cleanup
        let _ = std::fs::remove_dir_all(session_workspace_dir(sid));
    }
}
