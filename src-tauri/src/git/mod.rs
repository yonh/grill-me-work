//! Thin system-git bridge for prototype decision-tree / branch history.
//! Uses the system `git` binary with an extended GUI PATH (same strategy as agent CLI).

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineCommit {
    pub sha: String,
    pub short_sha: String,
    pub subject: String,
    pub body: Option<String>,
    pub parent_shas: Vec<String>,
    pub branch_tips: Vec<String>,
    pub author_time: String,
    pub version: Option<i32>,
    pub is_head: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub available: bool,
    pub initialized: bool,
    pub branch: Option<String>,
    pub dirty: bool,
    pub head: Option<String>,
    /// Absolute path of the prototype workspace (git root).
    pub path: String,
    /// Whether prototype files already exist on disk.
    pub has_files: bool,
}

fn git_cmd(dir: &Path) -> Command {
    let mut c = Command::new("git");
    c.current_dir(dir);
    c.env("PATH", crate::agent::gui_path());
    c.env("GIT_TERMINAL_PROMPT", "0");
    c.env("GIT_CONFIG_NOSYSTEM", "1");
    // Local identity so GUI env without git config still commits
    c.env("GIT_AUTHOR_NAME", "Grill-Me");
    c.env("GIT_AUTHOR_EMAIL", "grill-me@local");
    c.env("GIT_COMMITTER_NAME", "Grill-Me");
    c.env("GIT_COMMITTER_EMAIL", "grill-me@local");
    c
}

fn run_git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = git_cmd(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git {args:?}: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("exit {:?}", out.status.code())
        };
        return Err(format!("git {args:?}: {detail}"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn git_available() -> bool {
    crate::agent::which_tool("git").is_some()
        || Command::new("git")
            .arg("--version")
            .env("PATH", crate::agent::gui_path())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

fn is_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Ensure a local git repo exists in `dir` and has at least one commit if there are files.
pub fn ensure_repo(dir: &Path) -> Result<(), String> {
    if !git_available() {
        return Err("系统未安装 git".into());
    }
    if !is_repo(dir) {
        run_git(dir, &["init", "-b", "main"])?;
        // Minimal local config (in case env identity is ignored by some git versions)
        let _ = run_git(dir, &["config", "user.name", "Grill-Me"]);
        let _ = run_git(dir, &["config", "user.email", "grill-me@local"]);
        let _ = run_git(dir, &["config", "commit.gpgsign", "false"]);
    }
    Ok(())
}

fn has_head(dir: &Path) -> bool {
    run_git(dir, &["rev-parse", "--verify", "HEAD"]).is_ok()
}

fn working_tree_dirty(dir: &Path) -> bool {
    match run_git(dir, &["status", "--porcelain"]) {
        Ok(s) => !s.trim().is_empty(),
        Err(_) => false,
    }
}

/// Human-readable summary of dirty files, e.g. "3 files (+120/-30): a.html, b.css".
pub fn summarize_dirty(dir: &Path) -> Option<String> {
    let porcelain = run_git(dir, &["status", "--porcelain"]).ok()?;
    let mut paths: Vec<String> = Vec::new();
    for line in porcelain.lines() {
        if line.trim().is_empty() {
            continue;
        }
        // "XY path" or "XY old -> new"
        let path = line
            .get(3..)
            .unwrap_or(line)
            .split(" -> ")
            .last()
            .unwrap_or("")
            .trim()
            .trim_matches('"')
            .to_string();
        if path.is_empty() || path.starts_with(".git/") {
            continue;
        }
        // Keep prototype-relative interest; still show docs/
        paths.push(path);
    }
    if paths.is_empty() {
        return None;
    }
    paths.sort();
    paths.dedup();

    // Line stats against HEAD (staged+unstaged combined is messy; use numstat after add is too late)
    // Use `git diff --numstat HEAD` when HEAD exists.
    let (added, removed) = if has_head(dir) {
        match run_git(dir, &["diff", "--numstat", "HEAD"]) {
            Ok(out) => {
                let mut a = 0i64;
                let mut r = 0i64;
                for line in out.lines() {
                    let mut parts = line.split('\t');
                    let add: i64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                    let del: i64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                    a += add;
                    r += del;
                }
                (a, r)
            }
            Err(_) => (0, 0),
        }
    } else {
        (0, 0)
    };

    let n = paths.len();
    let preview: Vec<&str> = paths.iter().take(4).map(|s| s.as_str()).collect();
    let more = if n > 4 { format!(" 等 {n} 个文件") } else { String::new() };
    Some(format!(
        "{} 个文件(+{added}/-{removed}): {}{more}",
        n,
        preview.join(", ")
    ))
}

/// Stage everything and commit with optional multi-line body.
/// Returns short sha.
pub fn commit_all_with_body(
    dir: &Path,
    subject: &str,
    body: Option<&str>,
) -> Result<String, String> {
    ensure_repo(dir)?;
    let dirty_note = summarize_dirty(dir);
    run_git(dir, &["add", "-A"])?;
    if !working_tree_dirty(dir) && has_head(dir) {
        let sha = run_git(dir, &["rev-parse", "--short", "HEAD"])?
            .trim()
            .to_string();
        return Ok(sha);
    }

    let mut message = String::new();
    message.push_str(subject.trim());
    message.push('\n');
    if let Some(b) = body {
        let b = b.trim();
        if !b.is_empty() {
            message.push('\n');
            message.push_str(b);
            message.push('\n');
        }
    }
    if let Some(d) = dirty_note {
        if !message.ends_with('\n') {
            message.push('\n');
        }
        message.push('\n');
        message.push_str(&format!("改动范围: {d}\n"));
    }

    if has_head(dir) {
        run_git(dir, &["commit", "-m", &message])?;
    } else {
        run_git(dir, &["commit", "--allow-empty", "-m", &message])?;
    }
    Ok(run_git(dir, &["rev-parse", "--short", "HEAD"])?.trim().to_string())
}

/// Stage everything tracked-ish and commit. Returns short sha on success.
/// If nothing changed, returns the current HEAD short sha without creating a commit.
pub fn commit_all(dir: &Path, message: &str) -> Result<String, String> {
    commit_all_with_body(dir, message, None)
}

pub fn current_branch(dir: &Path) -> Option<String> {
    if !is_repo(dir) {
        return None;
    }
    let b = run_git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .ok()?
        .trim()
        .to_string();
    if b.is_empty() || b == "HEAD" {
        None
    } else {
        Some(b)
    }
}

pub fn head_sha(dir: &Path) -> Option<String> {
    if !is_repo(dir) {
        return None;
    }
    run_git(dir, &["rev-parse", "--short", "HEAD"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Create (or reuse) a branch at `at_sha` (or HEAD) and switch to it.
pub fn checkout_new_branch(dir: &Path, name: &str, at_sha: Option<&str>) -> Result<(), String> {
    ensure_repo(dir)?;
    if working_tree_dirty(dir) {
        // Commit dirty work first so checkout is clean
        commit_all(dir, "wip: before branch switch")?;
    }
    let exists = run_git(dir, &["show-ref", "--verify", "--quiet", &format!("refs/heads/{name}")]).is_ok();
    if exists {
        run_git(dir, &["checkout", name])?;
    } else {
        match at_sha {
            Some(sha) if !sha.is_empty() => run_git(dir, &["checkout", "-b", name, sha])?,
            _ => run_git(dir, &["checkout", "-b", name])?,
        };
    }
    Ok(())
}

/// Switch to an existing branch or commit. Dirty tree is committed as wip first.
pub fn checkout(dir: &Path, ref_name: &str) -> Result<(), String> {
    ensure_repo(dir)?;
    if working_tree_dirty(dir) {
        commit_all(dir, "wip: before checkout")?;
    }
    run_git(dir, &["checkout", ref_name])?;
    Ok(())
}

pub fn list_branches(dir: &Path) -> Result<Vec<String>, String> {
    ensure_repo(dir)?;
    let out = run_git(dir, &["for-each-ref", "--format=%(refname:short)", "refs/heads"])?;
    Ok(out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

fn parse_version_from_subject(subject: &str) -> Option<i32> {
    // Prefer explicit "v12" token
    for token in subject.split_whitespace() {
        let t = token.trim_matches(|c: char| !c.is_alphanumeric() && c != 'v');
        if let Some(rest) = t.strip_prefix('v') {
            if let Ok(n) = rest.parse::<i32>() {
                return Some(n);
            }
        }
    }
    None
}

/// Read linear-friendly graph log for the timeline UI.
/// Uses a custom pretty format so we can recover parents and branch tips.
pub fn log_graph(dir: &Path, limit: usize) -> Result<Vec<TimelineCommit>, String> {
    ensure_repo(dir)?;
    if !has_head(dir) {
        return Ok(vec![]);
    }

    let sep = "\x1f";
    let rec = "\x1e";
    let pretty = format!(
        "%H%x1f%h%x1f%P%x1f%s%x1f%b%x1f%aI%x1e"
    );
    let out = run_git(
        dir,
        &[
            "log",
            "--all",
            &format!("--max-count={limit}"),
            &format!("--pretty=format:{pretty}"),
        ],
    )?;

    // Branch tips: "sha branch" lines
    let mut tips: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    if let Ok(refs) = run_git(
        dir,
        &[
            "for-each-ref",
            "--format=%(objectname:short) %(refname:short)",
            "refs/heads",
        ],
    ) {
        for line in refs.lines() {
            let mut parts = line.splitn(2, ' ');
            if let (Some(sha), Some(name)) = (parts.next(), parts.next()) {
                tips.entry(sha.trim().to_string())
                    .or_default()
                    .push(name.trim().to_string());
            }
        }
    }

    let head = head_sha(dir);

    let mut commits = Vec::new();
    for chunk in out.split(rec) {
        let chunk = chunk.trim_matches('\n');
        if chunk.is_empty() {
            continue;
        }
        let fields: Vec<&str> = chunk.split(sep).collect();
        if fields.len() < 6 {
            continue;
        }
        let sha = fields[0].to_string();
        let short_sha = fields[1].to_string();
        let parent_shas = fields[2]
            .split_whitespace()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let subject = fields[3].to_string();
        let body = {
            let b = fields[4].trim();
            if b.is_empty() {
                None
            } else {
                Some(b.to_string())
            }
        };
        let author_time = fields[5].to_string();
        let version = parse_version_from_subject(&subject);
        let is_head = head.as_deref() == Some(short_sha.as_str()) || head.as_deref() == Some(sha.as_str());
        let branch_tips = tips.get(&short_sha).cloned().unwrap_or_default();

        commits.push(TimelineCommit {
            sha,
            short_sha,
            subject,
            body,
            parent_shas,
            branch_tips,
            author_time,
            version,
            is_head,
        });
    }

    Ok(commits)
}

pub fn status(dir: &Path) -> GitStatus {
    let path = dir.to_string_lossy().to_string();
    // Workspace layout: files live under prototype/
    let proto = dir.join("prototype");
    let has_files = proto.join("index.html").exists()
        || proto.join("css").exists()
        || proto.join("pages").exists()
        || dir.join("index.html").exists()
        || dir.join("css").exists();
    let available = git_available();
    if !available {
        return GitStatus {
            available: false,
            initialized: false,
            branch: None,
            dirty: false,
            head: None,
            path,
            has_files,
        };
    }
    let initialized = is_repo(dir);
    if !initialized {
        return GitStatus {
            available: true,
            initialized: false,
            branch: None,
            dirty: false,
            head: None,
            path,
            has_files,
        };
    }
    GitStatus {
        available: true,
        initialized: true,
        branch: current_branch(dir),
        dirty: working_tree_dirty(dir),
        head: head_sha(dir),
        path,
        has_files,
    }
}

/// Sanitize a branch name fragment.
pub fn sanitize_branch_name(raw: &str) -> String {
    let mut s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_string();
    let candidate = if s.is_empty() {
        String::new()
    } else {
        s.chars().take(48).collect()
    };
    // Belt: let git itself judge — covers rules we'd otherwise have to
    // reimplement (no '..', no leading '-'/'.', no '.lock' suffix, no '@{' …).
    if candidate.is_empty() || !is_valid_branch_name(&candidate) {
        format!("timeline-{}", chrono::Utc::now().timestamp())
    } else {
        candidate
    }
}

fn is_valid_branch_name(name: &str) -> bool {
    std::process::Command::new("git")
        .args(["check-ref-format", "--branch", name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|st| st.success())
        .unwrap_or(false)
}
