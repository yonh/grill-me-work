//! Local coding-agent CLI bridge (ported from quick_agent).
//! Grill-Me only composes requirements/prompt; the agent writes the prototype files.

use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentToolStatus {
    pub id: String,
    pub available: bool,
    pub path: Option<String>,
}

/// GUI apps get a minimal PATH. Extend it so Homebrew / ~/.local/bin CLIs resolve.
pub fn gui_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let existing = std::env::var("PATH").unwrap_or_default();
    let extras = [
        format!("{home}/.local/bin"),
        format!("{home}/.cargo/bin"),
        format!("{home}/.npm-global/bin"),
        "/opt/homebrew/bin".into(),
        "/usr/local/bin".into(),
        "/usr/bin".into(),
        "/bin".into(),
        "/usr/sbin".into(),
        "/sbin".into(),
    ];
    let mut parts: Vec<String> = extras
        .into_iter()
        .filter(|p| std::path::Path::new(p).exists())
        .collect();
    for p in existing.split(':') {
        if !p.is_empty() && !parts.iter().any(|x| x == p) {
            parts.push(p.to_string());
        }
    }
    parts.join(":")
}

pub fn tool_binary(tool: &str) -> &str {
    match tool {
        "opencode" => "opencode",
        "codex" => "codex",
        "devin" => "devin",
        "agy" => "agy",
        other => other,
    }
}

pub fn which_tool(tool: &str) -> Option<String> {
    let path_env = gui_path();
    let bin = tool_binary(tool);
    for dir in path_env.split(':') {
        let candidate = PathBuf::from(dir).join(bin);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

pub fn list_tools() -> Vec<AgentToolStatus> {
    ["opencode", "codex", "devin", "agy"]
        .into_iter()
        .map(|id| {
            let path = which_tool(id);
            AgentToolStatus {
                id: id.to_string(),
                available: path.is_some(),
                path,
            }
        })
        .collect()
}

fn normalize_effort(effort: &str) -> Option<String> {
    let e = effort.trim().to_ascii_lowercase();
    if e.is_empty() || e == "auto" || e == "default" {
        return None;
    }
    Some(e)
}

fn strip_devin_effort_suffix(model: &str) -> &str {
    const SUFFIXES: [&str; 10] = [
        "-low-fast",
        "-medium-fast",
        "-high-fast",
        "-xhigh-fast",
        "-max-fast",
        "-minimal",
        "-low",
        "-medium",
        "-high",
        "-xhigh",
    ];
    let lower = model.to_ascii_lowercase();
    for s in SUFFIXES {
        if lower.ends_with(s) && model.len() > s.len() {
            return &model[..model.len() - s.len()];
        }
    }
    if lower.ends_with("-max") && model.len() > 4 {
        return &model[..model.len() - 4];
    }
    model
}

fn resolve_devin_model(model: &str, effort: &str) -> String {
    let base = strip_devin_effort_suffix(model.trim());
    match normalize_effort(effort).as_deref() {
        None => base.to_string(),
        Some(e) => {
            let suffix = match e {
                "minimal" | "low" => "low",
                "medium" | "med" => "medium",
                "high" => "high",
                "xhigh" => "xhigh",
                "max" | "ultra" => "max",
                other => other,
            };
            format!("{base}-{suffix}")
        }
    }
}

/// Build a non-interactive coding-agent command that edits `workdir`.
pub fn build_agent_command(
    tool: &str,
    model: &str,
    prompt: &str,
    workdir: &str,
    auto_approve: bool,
    effort: &str,
) -> Result<Command, String> {
    let bin = which_tool(tool).ok_or_else(|| {
        format!(
            "agent CLI `{tool}` 不在 PATH 中。已尝试 ~/.local/bin、~/.cargo/bin、Homebrew 等路径。请先安装，或到设置里换其他工具。"
        )
    })?;

    let mut cmd = Command::new(bin);
    cmd.env("PATH", gui_path());
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.current_dir(workdir);

    let model = model.trim();
    let effort_norm = normalize_effort(effort);

    match tool {
        "opencode" => {
            cmd.arg("run");
            if !model.is_empty() {
                cmd.arg("-m").arg(model);
            }
            cmd.arg("--dir").arg(workdir);
            if auto_approve {
                cmd.arg("--auto");
            }
            if let Some(v) = effort_norm {
                cmd.arg("--variant").arg(v);
            }
            cmd.arg("--").arg(prompt);
        }
        "codex" => {
            cmd.arg("exec");
            cmd.arg("-C").arg(workdir);
            if !model.is_empty() {
                cmd.arg("-m").arg(model);
            }
            if auto_approve {
                cmd.arg("--approve-for-me");
            } else {
                cmd.arg("--sandbox").arg("workspace-write");
            }
            if let Some(v) = effort_norm {
                cmd.arg("-c").arg(format!("model_reasoning_effort=\"{v}\""));
            }
            cmd.arg(prompt);
        }
        "devin" => {
            let devin_model = if model.is_empty() {
                String::new()
            } else {
                resolve_devin_model(model, effort)
            };
            cmd.arg("-p").arg(prompt);
            if !devin_model.is_empty() {
                cmd.arg("--model").arg(devin_model);
            }
            if auto_approve {
                cmd.arg("--permission-mode").arg("dangerous");
            }
        }
        "agy" => {
            cmd.arg("-p").arg("--print-timeout").arg("10m");
            if !model.is_empty() {
                cmd.arg("--model").arg(model);
            }
            if auto_approve {
                cmd.arg("--dangerously-skip-permissions");
            }
            if let Some(v) = effort_norm {
                let v = match v.as_str() {
                    "minimal" => "low",
                    "xhigh" | "max" | "ultra" => "high",
                    "med" => "medium",
                    other => other,
                };
                cmd.arg("--effort").arg(v);
            }
            cmd.arg(prompt);
        }
        other => return Err(format!("unsupported tool: {other}")),
    }

    Ok(cmd)
}

/// Strip ANSI/VT escape sequences and other control chars so UI log stays readable.
fn sanitize_agent_line(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // CSI: ESC [ ... final
            if chars.peek() == Some(&'[') {
                chars.next();
                for n in chars.by_ref() {
                    if n.is_ascii_alphabetic() || n == '~' {
                        break;
                    }
                }
                continue;
            }
            // OSC: ESC ] ... BEL or ESC \
            if chars.peek() == Some(&']') {
                chars.next();
                while let Some(n) = chars.next() {
                    if n == '\u{7}' {
                        break;
                    }
                    if n == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
                continue;
            }
            // Other ESC sequences: skip next char
            let _ = chars.next();
            continue;
        }
        if c == '\r' {
            continue;
        }
        // Drop remaining C0 controls except tab
        if (c as u32) < 0x20 && c != '\t' {
            continue;
        }
        out.push(c);
    }
    out.trim().to_string()
}

// ==================== run cancellation + log buffer ====================

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Error string returned by [`run_agent_to_completion`] when cancelled.
pub const ERR_CANCELLED: &str = "__cancelled__";

fn cancel_flags() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    static C: OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a fresh cancel flag for a run about to start (one per session).
pub fn new_cancel_flag(session_id: &str) -> Arc<AtomicBool> {
    let f = Arc::new(AtomicBool::new(false));
    cancel_flags()
        .lock()
        .unwrap()
        .insert(session_id.to_string(), f.clone());
    f
}

/// Signal the running agent for `session_id` to stop. false if nothing running.
pub fn request_cancel(session_id: &str) -> bool {
    if let Some(f) = cancel_flags().lock().unwrap().get(session_id) {
        f.store(true, Ordering::Relaxed);
        return true;
    }
    false
}

/// Drop the flag once a run finishes (success/fail/cancel alike).
pub fn clear_cancel_flag(session_id: &str) {
    cancel_flags().lock().unwrap().remove(session_id);
}

const AGENT_LOG_CAP: usize = 400;

/// Ring buffer of recent agent output lines per session (for log polling).
fn agent_logs() -> &'static Mutex<HashMap<String, VecDeque<String>>> {
    static C: OnceLock<Mutex<HashMap<String, VecDeque<String>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn push_agent_log(session_id: &str, stream: &str, text: &str) {
    let mut g = agent_logs().lock().unwrap();
    let buf = g.entry(session_id.to_string()).or_default();
    buf.push_back(format!("[{stream}] {text}"));
    while buf.len() > AGENT_LOG_CAP {
        buf.pop_front();
    }
}

pub fn clear_agent_log(session_id: &str) {
    agent_logs().lock().unwrap().remove(session_id);
}

/// Recent agent output lines; `tail` limits to the last N.
pub fn get_agent_log(session_id: &str, tail: Option<usize>) -> Vec<String> {
    let g = agent_logs().lock().unwrap();
    let buf = match g.get(session_id) {
        Some(b) => b,
        None => return vec![],
    };
    match tail {
        Some(n) => buf
            .iter()
            .skip(buf.len().saturating_sub(n))
            .cloned()
            .collect(),
        None => buf.iter().cloned().collect(),
    }
}

/// Run agent to completion, invoking `on_line(stream, text)` for each output line.
/// `cancel`: when the flag flips true the child is killed and Err(ERR_CANCELLED) returned.
/// Returns Ok(exit_code) or Err(message).
pub fn run_agent_to_completion(
    tool: &str,
    model: &str,
    prompt: &str,
    workdir: &str,
    auto_approve: bool,
    effort: &str,
    cancel: Option<Arc<AtomicBool>>,
    mut on_line: impl FnMut(&str, &str) + Send,
) -> Result<i32, String> {
    use std::io::BufRead;
    use std::sync::mpsc;

    /// Read line-by-line so multi-byte UTF-8 (Chinese) is never split mid-character.
    fn pump(
        reader: impl std::io::Read + Send + 'static,
        kind: &'static str,
        tx: mpsc::Sender<(String, String)>,
    ) {
        std::thread::spawn(move || {
            let mut lines = std::io::BufReader::new(reader).lines();
            while let Some(Ok(line)) = lines.next() {
                let text = sanitize_agent_line(&line);
                if !text.is_empty() {
                    let _ = tx.send((kind.to_string(), text));
                }
            }
        });
    }

    let mut cmd = build_agent_command(tool, model, prompt, workdir, auto_approve, effort)?;
    let mut child = cmd.spawn().map_err(|e| format!("spawn {tool}: {e}"))?;

    let (tx, rx) = mpsc::channel::<(String, String)>();
    if let Some(out) = child.stdout.take() {
        pump(out, "stdout", tx.clone());
    }
    if let Some(err) = child.stderr.take() {
        pump(err, "stderr", tx);
    }

    let status = loop {
        while let Ok((stream, text)) = rx.try_recv() {
            on_line(&stream, &text);
        }
        if cancel
            .as_ref()
            .map(|f| f.load(Ordering::Relaxed))
            .unwrap_or(false)
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ERR_CANCELLED.into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(80)),
            Err(e) => return Err(format!("wait failed: {e}")),
        }
    };

    // Drain remaining buffered lines after exit
    std::thread::sleep(std::time::Duration::from_millis(80));
    while let Ok((stream, text)) = rx.try_recv() {
        on_line(&stream, &text);
    }

    Ok(status.code().unwrap_or(-1))
}

// ==================== internal app log (debug observability via MCP) ====================

const APP_LOG_CAP: usize = 500;

fn app_log_buf() -> &'static Mutex<VecDeque<String>> {
    static C: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Internal event trace — readable via `get_app_log` for debugging async flows
/// (outline/spec/ticket generation) that don't surface through agent logs.
pub fn push_app_log(tag: &str, msg: &str) {
    let mut g = app_log_buf().lock().unwrap();
    g.push_back(format!("{} [{tag}] {msg}", chrono::Utc::now().format("%H:%M:%S%.3f")));
    while g.len() > APP_LOG_CAP {
        g.pop_front();
    }
    log::info!("[{tag}] {msg}");
}

pub fn get_app_log(tail: Option<usize>) -> Vec<String> {
    let g = app_log_buf().lock().unwrap();
    match tail {
        Some(n) => g.iter().skip(g.len().saturating_sub(n)).cloned().collect(),
        None => g.iter().cloned().collect(),
    }
}
