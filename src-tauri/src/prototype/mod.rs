use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

/// One file in the multi-file prototype.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrototypeFile {
    pub path: String,
    pub content: String,
}

/// Full snapshot of a prototype directory (used for LLM context + version history).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrototypeSnapshot {
    pub files: Vec<PrototypeFile>,
    pub changelog: Option<String>,
}

/// LLM response format for (re)generating the multi-file prototype.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrototypeGenPayload {
    pub files: Vec<PrototypeFile>,
    #[serde(default)]
    pub changelog: Option<String>,
    #[serde(default)]
    pub delete_paths: Vec<String>,
}

/// Root directory that holds all session prototypes.
/// Layout is delegated to `crate::workspace` (`~/.grill-work/<sid>/prototype`).
fn session_prototype_dir_impl(session_id: &str) -> PathBuf {
    crate::workspace::session_prototype_dir(session_id)
}

pub fn session_prototype_dir(session_id: &str) -> PathBuf {
    session_prototype_dir_impl(session_id)
}

/// Workspace root (docs/ + prototype/ + .git).
pub fn session_workspace_dir(session_id: &str) -> PathBuf {
    crate::workspace::session_workspace_dir(session_id)
}

/// Iteration blueprint lives at workspace root (not inside the static site).
pub fn graph_path(session_id: &str) -> PathBuf {
    crate::workspace::iteration_graph_path(session_id)
}

pub fn load_iteration_graph(session_id: &str) -> Option<String> {
    std::fs::read_to_string(graph_path(session_id)).ok()
}

pub fn save_iteration_graph(session_id: &str, json: &str) -> std::io::Result<()> {
    crate::workspace::ensure_workspace(session_id)?;
    std::fs::write(graph_path(session_id), json)
}

pub fn ensure_session_dir(session_id: &str) -> std::io::Result<PathBuf> {
    let ws = crate::workspace::ensure_workspace(session_id)?;
    Ok(ws.join("prototype"))
}

/// Resolve a relative path inside the session dir, rejecting path traversal.
fn safe_join(base: &Path, rel: &str) -> Option<PathBuf> {
    let rel = rel.trim().trim_start_matches('/');
    if rel.is_empty() || rel.contains("..") {
        return None;
    }
    let joined = base.join(rel);
    // Ensure resolved path stays under base
    let base_canon = base.canonicalize().ok()?;
    let joined_canon = if joined.exists() {
        joined.canonicalize().ok()?
    } else {
        // For new files, canonicalize parent
        let parent = joined.parent()?;
        let parent_canon = parent.canonicalize().ok()?;
        parent_canon.join(joined.file_name()?)
    };
    if joined_canon.starts_with(&base_canon) {
        Some(joined)
    } else {
        None
    }
}

fn is_texty(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".html")
        || lower.ends_with(".htm")
        || lower.ends_with(".css")
        || lower.ends_with(".js")
        || lower.ends_with(".mjs")
        || lower.ends_with(".json")
        || lower.ends_with(".svg")
        || lower.ends_with(".txt")
        || lower.ends_with(".md")
}

fn walk_files(dir: &Path, base: &Path, out: &mut Vec<PrototypeFile>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // skip version-history / hidden dirs
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }
            walk_files(&path, base, out);
        } else if path.is_file() {
            let rel = path
                .strip_prefix(base)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if !is_texty(&rel) {
                continue;
            }
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.starts_with('.') || fname == "TASK.md" {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                out.push(PrototypeFile {
                    path: rel,
                    content,
                });
            }
        }
    }
}

/// Read all text files under the session prototype directory.
pub fn read_snapshot(session_id: &str) -> PrototypeSnapshot {
    let dir = session_prototype_dir(session_id);
    let mut files = Vec::new();
    if dir.exists() {
        walk_files(&dir, &dir, &mut files);
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    PrototypeSnapshot {
        files,
        changelog: None,
    }
}

pub fn has_prototype(session_id: &str) -> bool {
    session_prototype_dir(session_id).join("index.html").exists()
}

/// Write a minimal static-site scaffold so the coding agent always has a starting tree.
pub fn seed_scaffold(session_id: &str) -> std::io::Result<()> {
    let dir = ensure_session_dir(session_id)?;
    let index = dir.join("index.html");
    if index.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dir.join("css"))?;
    std::fs::create_dir_all(dir.join("js"))?;
    std::fs::create_dir_all(dir.join("pages"))?;
    std::fs::write(
        &index,
        r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>原型</title>
  <link rel="stylesheet" href="css/styles.css" />
</head>
<body>
  <main class="app">
    <h1>原型生成中</h1>
    <p>回答访谈问题后，coding agent 会在这里实现界面。</p>
  </main>
  <script src="js/app.js"></script>
</body>
</html>
"#,
    )?;
    std::fs::write(
        dir.join("css").join("styles.css"),
        "/* styles */\n:root{font-family:system-ui,sans-serif}\nbody{margin:0;padding:24px}\n",
    )?;
    std::fs::write(dir.join("js").join("app.js"), "// app\n")?;
    Ok(())
}

/// Apply a generation payload: write files, delete listed paths.
pub fn apply_payload(session_id: &str, payload: &PrototypeGenPayload) -> std::io::Result<usize> {
    let dir = ensure_session_dir(session_id)?;

    for del in &payload.delete_paths {
        if let Some(p) = safe_join(&dir, del) {
            if p.exists() {
                std::fs::remove_file(&p)?;
            }
        }
    }

    let mut written = 0usize;
    for f in &payload.files {
        let Some(path) = safe_join(&dir, &f.path) else {
            log::warn!("[prototype] rejected unsafe path: {}", f.path);
            continue;
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &f.content)?;
        written += 1;
    }

    // Guarantee a minimal index.html so preview never crashes
    let index = dir.join("index.html");
    if !index.exists() {
        std::fs::write(
            &index,
            r#"<!DOCTYPE html>
<html lang="zh-CN">
<head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Prototype</title>
<link rel="stylesheet" href="css/styles.css"></head>
<body><main class="app"><h1>原型尚未生成</h1><p>回答访谈问题后会自动更新。</p></main>
<script src="js/app.js"></script></body></html>"#,
        )?;
        written += 1;
    }
    let css = dir.join("css").join("styles.css");
    if !css.exists() {
        std::fs::create_dir_all(css.parent().unwrap())?;
        std::fs::write(&css, "/* styles */\n")?;
    }
    let js = dir.join("js").join("app.js");
    if !js.exists() {
        std::fs::create_dir_all(js.parent().unwrap())?;
        std::fs::write(&js, "// app\n")?;
    }

    Ok(written)
}

pub fn read_file(session_id: &str, rel_path: &str) -> Option<String> {
    let dir = session_prototype_dir(session_id);
    let path = safe_join(&dir, rel_path)?;
    std::fs::read_to_string(path).ok()
}

// ---------------------------------------------------------------------------
// Local static preview server (one root at a time)
// ---------------------------------------------------------------------------

struct PreviewState {
    root: PathBuf,
    port: u16,
}

static PREVIEW: OnceLock<Mutex<Option<PreviewState>>> = OnceLock::new();

fn preview_state() -> &'static Mutex<Option<PreviewState>> {
    PREVIEW.get_or_init(|| Mutex::new(None))
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Start (or retarget) the local preview server. Returns base URL like `http://127.0.0.1:PORT`.
pub fn ensure_preview_server(session_id: &str) -> std::io::Result<String> {
    let dir = ensure_session_dir(session_id)?;
    let mut guard = preview_state().lock().unwrap();

    if let Some(state) = guard.as_ref() {
        if state.root == dir {
            return Ok(format!("http://127.0.0.1:{}", state.port));
        }
    }

    // Bind an ephemeral port
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let root = dir.clone();

    // tiny_http wants std::net::TcpListener — rebind via tiny_http
    drop(listener);
    let server = tiny_http::Server::http(("127.0.0.1", port)).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("preview bind failed: {e}"))
    })?;

    let root_for_thread = root.clone();
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let url = request.url().to_string();
            let rel = url.split('?').next().unwrap_or("/").trim_start_matches('/');
            let rel = if rel.is_empty() { "index.html" } else { rel };

            // Reject traversal
            let path_buf = PathBuf::from(&rel);
            if rel.contains("..") {
                let resp = tiny_http::Response::from_string("forbidden").with_status_code(403);
                let _ = request.respond(resp);
                continue;
            }

            let mut file_path = root_for_thread.join(&path_buf);
            if file_path.is_dir() {
                file_path = file_path.join("index.html");
            }

            match std::fs::read(&file_path) {
                Ok(bytes) => {
                    let ct = content_type(&file_path);
                    let header = tiny_http::Header::from_bytes(&b"Content-Type"[..], ct.as_bytes())
                        .unwrap();
                    let resp = tiny_http::Response::from_data(bytes).with_header(header);
                    let _ = request.respond(resp);
                }
                Err(_) => {
                    // SPA-ish fallback to index.html for missing pages
                    let index = root_for_thread.join("index.html");
                    if let Ok(bytes) = std::fs::read(&index) {
                        let header =
                            tiny_http::Header::from_bytes(&b"Content-Type"[..], b"text/html; charset=utf-8")
                                .unwrap();
                        let resp = tiny_http::Response::from_data(bytes)
                            .with_header(header)
                            .with_status_code(404);
                        let _ = request.respond(resp);
                    } else {
                        let resp = tiny_http::Response::from_string("not found").with_status_code(404);
                        let _ = request.respond(resp);
                    }
                }
            }
        }
    });

    let url = format!("http://127.0.0.1:{port}");
    *guard = Some(PreviewState {
        root: dir,
        port,
    });
    log::info!("[prototype] preview server at {} for session {}", url, session_id);
    Ok(url)
}

/// Shared map for debounce bookkeeping if needed later.
pub type PendingMap = Arc<Mutex<HashMap<String, bool>>>;
