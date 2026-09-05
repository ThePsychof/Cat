use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
}

use serde::{Deserialize, Serialize};

mod credentials;
pub use credentials::{get_token, read_credentials, set_token, write_credentials, CredentialStore};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileCompareStatus {
    Same,
    Modified,
    LocalOnly,
    RemoteOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileComparison {
    pub path: String,
    pub status: FileCompareStatus,
}

fn list_tree_files(repo_dir: &Path, treeish: &str) -> Result<HashSet<String>, String> {
    let output = git_output(repo_dir, &["ls-tree", "-r", "--name-only", treeish])?;
    Ok(output.lines().map(|line| line.to_string()).collect())
}

pub fn compare_files_with_remote(
    repo_dir: &Path,
    remote: &str,
    branch: &str,
) -> Result<Vec<FileComparison>, String> {
    let remote_ref = format!("{}/{}", remote, branch);

    let local_files = list_tree_files(repo_dir, "HEAD")?;
    let remote_files = match list_tree_files(repo_dir, &remote_ref) {
        Ok(files) => files,
        Err(_) => HashSet::new(), // no remote-tracking branch yet — everything reads as local-only
    };

    let diff_output = git_output(repo_dir, &["diff", "--name-status", &remote_ref, "HEAD"])
        .unwrap_or_default();

    let mut results = Vec::new();
    let mut seen = HashSet::new();

    for line in diff_output.lines() {
        let mut parts = line.splitn(2, '\t');
        let code = parts.next().unwrap_or("").trim();
        let path = parts.next().unwrap_or("").trim().to_string();
        if path.is_empty() {
            continue;
        }
        let status = match code.chars().next().unwrap_or(' ') {
            'A' => FileCompareStatus::LocalOnly,
            'D' => FileCompareStatus::RemoteOnly,
            _ => FileCompareStatus::Modified, // M (modified) and R (renamed) both read as "changed"
        };
        seen.insert(path.clone());
        results.push(FileComparison { path, status });
    }

    // Anything present in both trees but absent from the diff is unchanged.
    for path in local_files.intersection(&remote_files) {
        if !seen.contains(path) {
            results.push(FileComparison {
                path: path.clone(),
                status: FileCompareStatus::Same,
            });
        }
    }

    results.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(results)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TreeNode {
    File { name: String, path: String },
    Directory { name: String, path: String, children: Vec<TreeNode> },
}

pub fn build_file_tree(repo_dir: &Path) -> Result<Vec<TreeNode>, String> {
    let files = list_files(repo_dir)?;
    let mut root: Vec<TreeNode> = Vec::new();

    for file_path in files {
        insert_into_tree(&mut root, &file_path, &file_path);
    }

    Ok(root)
}

fn insert_into_tree(nodes: &mut Vec<TreeNode>, remaining: &str, full_path: &str) {
    let (segment, rest) = match remaining.split_once('/') {
        Some((first, rest)) => (first, Some(rest)),
        None => (remaining, None),
    };

    match rest {
        None => {
            nodes.push(TreeNode::File {
                name: segment.to_string(),
                path: full_path.to_string(),
            });
        }
        Some(rest) => {
            let existing = nodes.iter_mut().find(|n| matches!(n, TreeNode::Directory { name, .. } if name == segment));

            let dir_path = &full_path[..full_path.len() - rest.len() - 1];

            match existing {
                Some(TreeNode::Directory { children, .. }) => {
                    insert_into_tree(children, rest, full_path);
                }
                _ => {
                    let mut children = Vec::new();
                    insert_into_tree(&mut children, rest, full_path);
                    nodes.push(TreeNode::Directory {
                        name: segment.to_string(),
                        path: dir_path.to_string(),
                        children,
                    });
                }
            }
        }
    }
}

pub fn read_file_at_head(repo_dir: &Path, file_path: &str) -> Result<String, String> {
    git_output(repo_dir, &["show", &format!("HEAD:{}", file_path)])
}

pub fn compare_repo_with_origin(
    repo_dir: &Path,
    token: Option<&str>,
) -> Result<Vec<FileComparison>, String> {
    // Fetch first so the comparison reflects the actual remote state,
    // not whatever was last fetched locally.
    let _ = fetch_repository(repo_dir, "origin", token); // ignore fetch errors (e.g. offline)
    let branch = get_current_branch(repo_dir)?;
    compare_files_with_remote(repo_dir, "origin", &branch)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoSyncState {
    UpToDate,
    Ahead,
    Behind,
    Diverged,
    Modified,
    MissingFiles,
    LocalOnly,
    RemoteOnly,
    Conflicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitProfile {
    pub name: String,
    pub user_name: String,
    pub user_email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryRecord {
    pub name: String,
    pub remote_url: String,
    pub local_path: String,
    pub read_only: bool,
    pub last_synced_at: Option<String>,
    pub last_synced_sha: Option<String>,
    pub size_bytes: Option<u64>,
    pub sync_state: RepoSyncState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveState {
    pub version: String,
    pub selected_os: Vec<String>,
    pub active_profile: Option<String>,
    pub profiles: Vec<GitProfile>,
    pub repositories: Vec<RepositoryRecord>,
}

impl DriveState {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            selected_os: Vec::new(),
            active_profile: None,
            profiles: Vec::new(),
            repositories: Vec::new(),
        }
    }

    pub fn add_profile(&mut self, profile: GitProfile) {
        self.profiles.push(profile);
        if self.active_profile.is_none() {
            self.active_profile = Some(self.profiles[0].name.clone());
        }
    }

    pub fn add_repository(&mut self, repo: RepositoryRecord) {
        self.repositories.push(repo);
    }
}

impl Default for DriveState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn discover_repositories(root: &Path) -> Result<Vec<RepositoryRecord>, String> {
    let mut repositories = Vec::new();
    let mut visited = HashSet::new();
    scan_for_repositories(root, &mut repositories, &mut visited)?;
    Ok(repositories)
}

fn scan_for_repositories(
    dir: &Path,
    repositories: &mut Vec<RepositoryRecord>,
    visited: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }

    let canonical = dir
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize {}: {e}", dir.display()))?;
    if !visited.insert(canonical.clone()) {
        return Ok(());
    }

    let git_dir = dir.join(".git");
    if git_dir.exists() {
        let repo = inspect_repository(dir)?;
        if !repositories.iter().any(|r| r.local_path == repo.local_path) {
            repositories.push(repo);
        }
        return Ok(());
    }

    let Ok(read_dir) = fs::read_dir(dir) else {
        // Can't read this directory (permissions, a weird reparse point,
        // etc.) — skip it rather than aborting the entire scan, so one bad
        // entry anywhere under repositories/ doesn't hide every other repo.
        return Ok(());
    };

    for entry in read_dir {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if name == ".git" || name == ".cat" || name == ".DS_Store" {
            continue;
        }

        if path.is_dir() {
            let _ = scan_for_repositories(&path, repositories, visited);
        }
    }

    Ok(())
}

pub fn inspect_repository(repo_dir: &Path) -> Result<RepositoryRecord, String> {
    let name = repo_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let remote_url = git_output(repo_dir, &["remote", "get-url", "origin"])
        .unwrap_or_default()
        .trim()
        .to_string();

    let sync_state = evaluate_repository_state(repo_dir)?;
    let size_bytes = directory_size(repo_dir).ok();

    Ok(RepositoryRecord {
        name,
        remote_url,
        local_path: repo_dir.to_string_lossy().to_string(),
        read_only: false,
        last_synced_at: None,
        last_synced_sha: git_output(repo_dir, &["rev-parse", "HEAD"])
            .ok()
            .map(|s| s.trim().to_string()),
        size_bytes,
        sync_state,
    })
}

pub fn evaluate_repository_state(repo_dir: &Path) -> Result<RepoSyncState, String> {
    let output = git_output(repo_dir, &["status", "--porcelain=v1", "--branch"])
        .map_err(|e| format!("Failed to inspect repo {}: {e}", repo_dir.display()))?;
    let text = output.trim();

    if text.is_empty() {
        return Ok(RepoSyncState::UpToDate);
    }

    if text.contains("Unmerged")
        || text.contains("UU ")
        || text.contains("AA ")
        || text.contains("DD ")
    {
        return Ok(RepoSyncState::Conflicted);
    }

    if text.contains("ahead") && text.contains("behind") {
        return Ok(RepoSyncState::Diverged);
    }
    if text.contains("ahead") {
        return Ok(RepoSyncState::Ahead);
    }
    if text.contains("behind") {
        return Ok(RepoSyncState::Behind);
    }

    // Check "No commits yet" BEFORE the M/?? checks so a brand-new repo
    // is classified as LocalOnly regardless of what else is in the output.
    if text.contains("No commits yet") {
        return Ok(RepoSyncState::LocalOnly);
    }

    if text.contains("??")
        || text.contains(" M")
        || text.contains("M ")
        || text.contains("A ")
        || text.contains("D ")
        || text.contains("R ")
    {
        return Ok(RepoSyncState::Modified);
    }

    Ok(RepoSyncState::UpToDate)
}

fn directory_size(path: &Path) -> Result<u64, std::io::Error> {
    fn walk(dir: &Path) -> Result<u64, std::io::Error> {
        let mut total = 0u64;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                // Regular subdirectory — recurse without following symlinks/junctions
                total += walk(&entry.path())?;
            } else if file_type.is_file() {
                // Regular file — accumulate its size
                total += entry.metadata()?.len();
            }
            // Symlinks and junctions (file_type.is_symlink()) are implicitly skipped
        }
        Ok(total)
    }
    walk(path)
}

fn git_output(repo_dir: &Path, args: &[&str]) -> Result<String, String> {
    git_output_authed(repo_dir, None, args)
}

fn git_output_authed(repo_dir: &Path, token: Option<&str>, args: &[&str]) -> Result<String, String> {
    use base64::Engine;

    let git = git_program(repo_dir)?;
    let mut full_args: Vec<String> = Vec::new();

    full_args.push("-c".to_string());
    full_args.push(format!("safe.directory={}", repo_dir.display()));

    if let Some(token) = token {
        let encoded = base64::engine::general_purpose::STANDARD.encode(format!("x-access-token:{token}"));
        full_args.push("-c".to_string());
        full_args.push(format!("http.extraHeader=Authorization: Basic {encoded}"));
    }

    full_args.extend(args.iter().map(|s| s.to_string()));

    let mut command = Command::new(git);
    hide_console(&mut command);
    let output = command
        .current_dir(repo_dir)
        .args(&full_args)
        .output()
        .map_err(|e| format!("Unable to execute git {}: {e}", args.join(" ")))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_program(repo_dir: &Path) -> Result<PathBuf, String> {
    let mut current = Some(repo_dir);
    while let Some(path) = current {
        let candidate =
            path.join(".cat")
                .join("tools")
                .join(if cfg!(windows) { "git.exe" } else { "git" });
        if candidate.is_file() {
            return Ok(candidate);
        }
        current = path.parent();
    }
    #[cfg(test)]
    {
        return Ok(PathBuf::from("git"));
    }
    #[cfg(not(test))]
    {
        Ok(PathBuf::from(if cfg!(windows) { "git.exe" } else { "git" }))
    }
}

pub struct SyncProgress {
    pub phase: String,
    pub loaded: u64,
    pub total: Option<u64>,
}

pub fn fetch_repository(repo_dir: &Path, remote: &str, token: Option<&str>) -> Result<(), String> {
    git_output_authed(repo_dir, token, &["fetch", remote])?;
    Ok(())
}

pub fn pull_repository(repo_dir: &Path, remote: &str, branch: &str, token: Option<&str>) -> Result<(), String> {
    git_output_authed(repo_dir, token, &["pull", remote, branch])?;
    Ok(())
}

pub fn push_repository(repo_dir: &Path, remote: &str, branch: &str, token: Option<&str>) -> Result<(), String> {
    git_output_authed(repo_dir, token, &["push", remote, branch])?;
    Ok(())
}

pub fn clone_repository(url: &str, target_dir: &Path, token: Option<&str>) -> Result<(), String> {
    use base64::Engine;

    // Use target_dir's parent as the lookup root for git_program (the target
    // doesn't exist yet, so walk up from its parent).
    let lookup_dir = target_dir.parent().unwrap_or_else(|| std::path::Path::new("."));
    let git = git_program(lookup_dir)?;

    let mut args: Vec<String> = Vec::new();

    if let Some(t) = token {
        let encoded = base64::engine::general_purpose::STANDARD
            .encode(format!("x-access-token:{t}"));
        args.push("-c".to_string());
        args.push(format!("http.extraHeader=Authorization: Basic {encoded}"));
    }

    args.push("clone".to_string());
    args.push(url.to_string());
    args.push(target_dir.to_string_lossy().to_string());

    let mut command = Command::new(git);
    hide_console(&mut command);
    let output = command
        .args(&args)
        .output()
        .map_err(|e| format!("Unable to execute git clone {url}: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(())
}

pub fn get_current_branch(repo_dir: &Path) -> Result<String, String> {
    git_output(repo_dir, &["rev-parse", "--abbrev-ref", "HEAD"]).map(|s| s.trim().to_string())
}

pub fn get_remote_url(repo_dir: &Path, remote: &str) -> Result<String, String> {
    git_output(repo_dir, &["remote", "get-url", remote]).map(|s| s.trim().to_string())
}

pub fn compare_with_remote(
    repo_dir: &Path,
    remote: &str,
    branch: &str,
) -> Result<(u32, u32), String> {
    let local_head = git_output(repo_dir, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    let remote_ref = format!("{}/{}", remote, branch);

    let remote_head = match git_output(repo_dir, &["rev-parse", &remote_ref]) {
        Ok(s) => s.trim().to_string(),
        Err(_) => return Ok((0, 0)),
    };

    if local_head == remote_head {
        return Ok((0, 0));
    }

    let ahead_output = git_output(
        repo_dir,
        &["rev-list", "--count", &format!("{}..HEAD", remote_ref)],
    )?;
    let ahead: u32 = ahead_output.trim().parse().unwrap_or(0);

    let behind_output = git_output(
        repo_dir,
        &["rev-list", "--count", &format!("HEAD..{}", remote_ref)],
    )?;
    let behind: u32 = behind_output.trim().parse().unwrap_or(0);

    Ok((ahead, behind))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub remote_tracking: Option<String>,
}

pub fn list_branches(repo_dir: &Path) -> Result<Vec<BranchInfo>, String> {
    let output = git_output(repo_dir, &["branch", "-a"])?
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let is_current = trimmed.starts_with("*");
            let name = if is_current {
                trimmed[2..].trim().to_string()
            } else {
                trimmed.trim().to_string()
            };
            if name.starts_with("remotes/") {
                return None;
            }
            Some(BranchInfo {
                name,
                is_current,
                remote_tracking: None,
            })
        })
        .collect();
    Ok(output)
}

pub fn switch_branch(repo_dir: &Path, branch_name: &str) -> Result<(), String> {
    git_output(repo_dir, &["checkout", branch_name])?;
    Ok(())
}

pub fn create_branch(repo_dir: &Path, branch_name: &str) -> Result<(), String> {
    git_output(repo_dir, &["checkout", "-b", branch_name])?;
    Ok(())
}

pub fn stage_all_changes(repo_dir: &Path) -> Result<(), String> {
    git_output(repo_dir, &["add", "-A"])?;
    Ok(())
}

pub fn commit_changes(
    repo_dir: &Path,
    message: &str,
    author_name: &str,
    author_email: &str,
) -> Result<String, String> {
    let git = git_program(repo_dir)?;
    let name_config = format!("user.name={author_name}");
    let email_config = format!("user.email={author_email}");
    let mut command = Command::new(git);
    hide_console(&mut command);
    let output = command
        .current_dir(repo_dir)
        .args([
            "-c",
            &format!("safe.directory={}", repo_dir.display()),
            "-c",
            &name_config,
            "-c",
            &email_config,
            "commit",
            "-m",
            message,
        ])
        .output()
        .map_err(|e| format!("Unable to execute git commit: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn open_in_editor(repo_dir: &Path, editor: &str) -> Result<(), String> {
    // On Windows, many editors (including VS Code's `code`) are installed as
    // `.cmd` shell scripts rather than plain `.exe` files.  CreateProcess
    // (what Command uses) cannot launch `.cmd` files directly — they require
    // the shell (cmd.exe) as the host process.
    #[cfg(windows)]
    {
        let mut command = Command::new("cmd");
        hide_console(&mut command);
        command
            .args(["/C", editor])
            .arg(repo_dir)
            .spawn()
            .map_err(|e| format!("Failed to open {} in {}: {e}", repo_dir.display(), editor))?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let mut command = Command::new(editor);
        hide_console(&mut command);
        command
            .arg(repo_dir)
            .spawn()
            .map_err(|e| format!("Failed to open {} in {}: {e}", repo_dir.display(), editor))?;
        Ok(())
    }
}

pub fn clone_from_drive(source_repo_path: &Path, target_dir: &Path) -> Result<(), String> {
    let source_path = source_repo_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve source path: {e}"))?
        .to_string_lossy()
        .to_string();

    let git = git_program(source_repo_path)?;
    let mut command = Command::new(git);
    hide_console(&mut command);
    let output = command
        .arg("clone")
        .arg(&source_path)
        .arg(target_dir)
        .output()
        .map_err(|e| format!("Unable to clone from {}: {e}", source_repo_path.display()))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(())
}

pub fn list_files(repo_dir: &Path) -> Result<Vec<String>, String> {
    let output = git_output(repo_dir, &["ls-tree", "-r", "--name-only", "HEAD"])?;
    Ok(output.lines().map(|line| line.to_string()).collect())
}

pub fn get_commit_log(repo_dir: &Path, max_count: usize) -> Result<Vec<CommitInfo>, String> {
    let count_str = max_count.to_string();
    let output = git_output(
        repo_dir,
        &[
            "log",
            &format!("--max-count={}", count_str),
            "--pretty=format:%H%n%an%n%ae%n%ai%n%s%n---END---",
        ],
    )?;

    let mut commits = Vec::new();
    let mut current_commit = CommitInfo::default();
    let mut field_idx = 0;

    for line in output.lines() {
        if line == "---END---" {
            if !current_commit.sha.is_empty() {
                commits.push(current_commit.clone());
            }
            current_commit = CommitInfo::default();
            field_idx = 0;
        } else {
            match field_idx {
                0 => current_commit.sha = line.to_string(),
                1 => current_commit.author = line.to_string(),
                2 => current_commit.email = line.to_string(),
                3 => current_commit.date = line.to_string(),
                4 => current_commit.message = line.to_string(),
                _ => {}
            }
            field_idx += 1;
        }
    }

    Ok(commits)
}

pub fn get_file_status(repo_dir: &Path) -> Result<Vec<FileStatus>, String> {
    let output = git_output(repo_dir, &["status", "--porcelain"])?;

    let mut statuses = Vec::new();
    for line in output.lines() {
        if line.len() < 3 {
            continue;
        }

        let status_code = &line[0..2];
        let filepath = line[3..].to_string();

        let status = match status_code {
            "M " => "modified",
            "A " => "added",
            "D " => "deleted",
            "R " => "renamed",
            "C " => "copied",
            "U " => "updated",
            "??" => "untracked",
            " M" => "modified",
            "MM" => "modified",
            " A" => "added",
            " D" => "deleted",
            "!!" => "ignored",
            "UU" | "AA" | "DD" | "AU" | "UA" | "DU" | "UD" => "unmerged",
            _ => "unknown",
        };

        statuses.push(FileStatus {
            path: filepath,
            status: status.to_string(),
        });
    }

    Ok(statuses)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileStatus {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommitInfo {
    pub sha: String,
    pub author: String,
    pub email: String,
    pub date: String,
    pub message: String,
}

pub fn drive_state_path(drive_root: &Path) -> PathBuf {
    drive_root.join(".cat").join("state.json")
}

pub fn load_drive_state(drive_root: &Path) -> Result<DriveState, String> {
    let path = drive_state_path(drive_root);
    if !path.exists() {
        return Ok(DriveState::new());
    }
    let raw =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("Failed to parse {}: {e}", path.display()))
}

pub fn save_drive_state(drive_root: &Path, state: &DriveState) -> Result<(), String> {
    let path = drive_state_path(drive_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to encode drive state: {e}"))?;
    fs::write(&path, raw).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

pub fn compare_files(repo_dir: &Path) -> Result<Vec<FileStatus>, String> {
    get_file_status(repo_dir)
}

#[cfg(test)]
mod tests {
    use super::{
        DriveState, GitProfile, RepoSyncState, RepositoryRecord, commit_changes,
        directory_size, discover_repositories, get_commit_log, get_file_status,
        stage_all_changes, clone_repository, evaluate_repository_state,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cat-{prefix}-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn init_git_repo(base: &PathBuf, repo_name: &str) -> PathBuf {
        let repo = base.join(repo_name);
        fs::create_dir_all(&repo).unwrap();

        let status = Command::new("git")
            .arg("init")
            .arg(&repo)
            .status()
            .expect("git init should work");
        assert!(status.success());

        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("config")
            .arg("user.name")
            .arg("Cat Test")
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .arg("config")
            .arg("user.email")
            .arg("cat@example.com")
            .status()
            .unwrap();

        repo
    }

    #[test]
    fn new_drive_state_starts_empty() {
        let state = DriveState::new();
        assert!(state.selected_os.is_empty());
        assert!(state.profiles.is_empty());
        assert!(state.repositories.is_empty());
        assert!(state.active_profile.is_none());
    }

    #[test]
    fn can_add_profile_and_repository() {
        let mut state = DriveState::new();
        state.add_profile(GitProfile {
            name: "work".to_string(),
            user_name: "Test User".to_string(),
            user_email: "test@example.com".to_string(),
        });

        state.add_repository(RepositoryRecord {
            name: "cat".to_string(),
            remote_url: "https://github.com/ThePsychof/Cat".to_string(),
            local_path: "/Volumes/Cat/repos/cat".to_string(),
            read_only: false,
            last_synced_at: None,
            last_synced_sha: None,
            size_bytes: Some(2048),
            sync_state: RepoSyncState::UpToDate,
        });

        assert_eq!(state.profiles.len(), 1);
        assert_eq!(state.repositories.len(), 1);
        assert_eq!(state.active_profile.as_deref(), Some("work"));
        assert_eq!(state.repositories[0].sync_state, RepoSyncState::UpToDate);
    }

    #[test]
    fn discovers_git_repositories_on_a_drive() {
        let base = unique_temp_dir("drive");
        let repo = init_git_repo(&base, "demo-repo");

        let discovered = discover_repositories(&base).unwrap();
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].local_path, repo.to_string_lossy().to_string());
        assert_eq!(discovered[0].name, "demo-repo");
    }

    #[test]
    fn can_stage_and_commit_changes_for_a_repo() {
        let base = unique_temp_dir("stage-commit");
        let repo = init_git_repo(&base, "change-tracker");

        let readme = repo.join("README.md");
        std::fs::write(&readme, "hello from Cat\n").unwrap();

        stage_all_changes(&repo).unwrap();
        let commit_output =
            commit_changes(&repo, "chore: add README", "Cat Test", "cat@example.com").unwrap();

        assert!(commit_output.contains("[main"));
        assert!(get_file_status(&repo).unwrap().is_empty());
        let log = get_commit_log(&repo, 5).unwrap();
        assert_eq!(log[0].message, "chore: add README");
    }

    // ────────────────────────────────────────────────────────────
    // Bug condition exploration tests (task 1 — written BEFORE fixes)
    // These tests are intentionally written against the EXPECTED
    // (fixed) behaviour so that:
    //   • 1a FAILS on unfixed code  → counterexample documents bug 4
    //   • 1b does NOT COMPILE       → compile error documents bug 1
    //   • 1c PASSES on unfixed code → confirms the bug is in GUI, not core
    // DO NOT alter these tests or the implementation to make them pass
    // until task 3 is reached.
    // ────────────────────────────────────────────────────────────

    /// Task 1a — Bug 4 exploration
    ///
    /// A freshly `git init`-ed repository with no commits should be classified
    /// as `LocalOnly`.  On unfixed code the function returns `Modified` because
    /// the "No commits yet" check sits *after* the M/??/A/D/R block and still
    /// returns the wrong variant.
    ///
    /// EXPECTED OUTCOME on unfixed code: FAILS
    ///   counterexample: evaluate_repository_state(new_repo) → Ok(Modified)
    ///                   but expected Ok(LocalOnly)
    ///
    /// Validates: Requirements 4.1, 4.2
    #[test]
    fn evaluate_repository_state_new_repo_returns_local_only() {
        let base = unique_temp_dir("new-repo-local-only");

        // git init — no commits, no staged files
        let status = Command::new("git")
            .arg("init")
            .arg(&base)
            .status()
            .expect("git init failed");
        assert!(status.success(), "git init must succeed");

        let result = evaluate_repository_state(&base)
            .expect("evaluate_repository_state should not error on a new repo");

        assert_eq!(
            result,
            RepoSyncState::LocalOnly,
            "A new repo with no commits must be classified as LocalOnly, got {:?}",
            result
        );
    }

    /// Task 1b — Bug 1 exploration
    ///
    /// `clone_repository` should accept a third `token: Option<&str>` argument
    /// and perform the clone via Command rather than gix.  The current
    /// two-argument signature makes this test a **compile error**, which is the
    /// expected evidence that bug 1 exists.
    ///
    /// EXPECTED OUTCOME on unfixed code: COMPILE ERROR (wrong arity)
    ///   counterexample: `clone_repository(url, dir, None)` does not compile
    ///   because the current signature is `clone_repository(url, dir)`
    ///
    /// Validates: Requirements 1.2, 1.3, 1.4, 1.6
    #[test]
    fn clone_repository_creates_local_copy() {
        // Create a local bare repo that acts as the "remote"
        let bare_dir = unique_temp_dir("bare-remote");
        let status = Command::new("git")
            .arg("init")
            .arg("--bare")
            .arg(&bare_dir)
            .status()
            .expect("git init --bare failed");
        assert!(status.success(), "git init --bare must succeed");

        // Give the bare repo at least one commit so it can be cloned
        // (use a temporary work-tree clone approach)
        let work_dir = unique_temp_dir("bare-work");
        let clone_status = Command::new("git")
            .args(["clone", &bare_dir.to_string_lossy(), &work_dir.to_string_lossy()])
            .status()
            .expect("git clone of bare repo failed");
        assert!(clone_status.success(), "staging clone must succeed");

        // Config + empty commit in the work tree
        Command::new("git").args(["-C", &work_dir.to_string_lossy(), "config", "user.name", "Cat Test"]).status().unwrap();
        Command::new("git").args(["-C", &work_dir.to_string_lossy(), "config", "user.email", "cat@example.com"]).status().unwrap();
        let commit_status = Command::new("git")
            .args(["-C", &work_dir.to_string_lossy(), "commit", "--allow-empty", "-m", "init"])
            .status()
            .expect("empty commit failed");
        assert!(commit_status.success(), "empty commit must succeed");

        // Detect the current branch name (could be 'main' or 'master')
        let branch_output = Command::new("git")
            .args(["-C", &work_dir.to_string_lossy(), "rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .expect("rev-parse HEAD failed");
        let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

        // Push the commit back into the bare repo
        let push_status = Command::new("git")
            .args(["-C", &work_dir.to_string_lossy(), "push", "origin", &branch])
            .status()
            .expect("git push to bare repo failed");
        assert!(push_status.success(), "push to bare repo must succeed");

        // Now clone via the library function — NOTE: three-arg signature is the
        // EXPECTED (fixed) API.  On unfixed code this line is a compile error.
        // Temporarily use the two-arg call so the other tests can run; the
        // three-arg call below (commented out) is the authoritative test that
        // documents bug 1.  After task 3.2 fixes the signature, swap the
        // comments back.
        let target_dir = unique_temp_dir("clone-target");
        let bare_url = bare_dir.to_string_lossy().to_string();
        // Three-arg call — now valid after Bug 1 fix:
        let result = clone_repository(&bare_url, &target_dir, None);

        assert!(result.is_ok(), "clone_repository should succeed: {:?}", result);
        assert!(
            target_dir.join(".git").exists(),
            "cloned directory must contain a .git entry"
        );
    }

    // ────────────────────────────────────────────────────────────
    // Preservation property tests (task 2 — written BEFORE fixes)
    // These tests capture CORRECT existing behaviour for inputs that
    // do NOT satisfy any bug condition.  They must PASS on unfixed code
    // and continue to pass after every fix (regression guard).
    // ────────────────────────────────────────────────────────────

    /// Task 2a — Preservation: evaluate_repository_state for non-new repos
    ///
    /// For repositories that already have at least one commit the "No commits
    /// yet" path is never taken, so these cases are unaffected by bug 4.
    /// They document the three states that must survive unchanged after the fix:
    ///
    ///   • committed repo, clean working tree      → UpToDate
    ///   • committed repo + untracked file          → Modified  (?? in status)
    ///   • committed repo + modified tracked file   → Modified  ( M in status)
    ///
    /// EXPECTED OUTCOME on unfixed code: PASSES (no bug-condition inputs)
    /// EXPECTED OUTCOME after bug 4 fix:  PASSES (preservation satisfied)
    ///
    /// Validates: Requirements 4.1, 4.2
    #[test]
    fn evaluate_repository_state_preservation_non_new_repo() {
        // ── helper: init repo + one commit ──────────────────────────────
        fn init_with_commit(prefix: &str) -> std::path::PathBuf {
            let base = {
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                std::env::temp_dir().join(format!("cat-{prefix}-{nanos}"))
            };
            fs::create_dir_all(&base).unwrap();

            let s = Command::new("git").arg("init").arg(&base).status().unwrap();
            assert!(s.success(), "git init failed for {prefix}");
            Command::new("git").args(["-C", &base.to_string_lossy(), "config", "user.name", "Cat Test"]).status().unwrap();
            Command::new("git").args(["-C", &base.to_string_lossy(), "config", "user.email", "cat@example.com"]).status().unwrap();

            // one committed file so HEAD exists
            let f = base.join("seed.txt");
            fs::write(&f, "seed").unwrap();
            Command::new("git").args(["-C", &base.to_string_lossy(), "add", "seed.txt"]).status().unwrap();
            let s = Command::new("git")
                .args(["-C", &base.to_string_lossy(), "commit", "-m", "seed"])
                .status()
                .unwrap();
            assert!(s.success(), "seed commit failed for {prefix}");
            base
        }

        // ── Case 1: clean committed repo → UpToDate ─────────────────────
        {
            let repo = init_with_commit("pres-uptodate");
            let result = evaluate_repository_state(&repo)
                .expect("evaluate_repository_state should not error on clean repo");
            assert_eq!(
                result,
                RepoSyncState::UpToDate,
                "clean committed repo must be UpToDate, got {:?}",
                result
            );
        }

        // ── Case 2: untracked file (??) → Modified ───────────────────────
        {
            let repo = init_with_commit("pres-untracked");
            fs::write(repo.join("untracked.txt"), "new file").unwrap();
            let result = evaluate_repository_state(&repo)
                .expect("evaluate_repository_state should not error with untracked file");
            assert_eq!(
                result,
                RepoSyncState::Modified,
                "repo with untracked file must be Modified, got {:?}",
                result
            );
        }

        // ── Case 3: modified tracked file ( M) → Modified ────────────────
        {
            let repo = init_with_commit("pres-modified");
            // mutate the already-tracked seed.txt (not staged → " M" in porcelain)
            fs::write(repo.join("seed.txt"), "changed content").unwrap();
            let result = evaluate_repository_state(&repo)
                .expect("evaluate_repository_state should not error with modified tracked file");
            assert_eq!(
                result,
                RepoSyncState::Modified,
                "repo with modified tracked file must be Modified, got {:?}",
                result
            );
        }
    }

    // ────────────────────────────────────────────────────────────
    // Task 1 — Bug condition exploration tests
    // Written BEFORE fixes.  Each test encodes the EXPECTED (correct) behaviour.
    // • 1a FAILS on unfixed code  → documents Bug D
    // • 1b FAILS on unfixed code  → documents Bug F
    // • 1c FAILS on unfixed Unix  → documents Bug C  (skipped on Windows)
    // DO NOT alter the tests or implementation to make them pass until task 3.
    // ────────────────────────────────────────────────────────────

    /// Task 1a — Bug D exploration: commit_changes must NOT write [user] to .git/config
    ///
    /// EXPECTED OUTCOME on unfixed code: FAILS
    ///   counterexample: .git/config contains "[user]" section after commit_changes
    ///
    /// Validates: Requirements 4.1, 4.2
    #[test]
    fn commit_changes_does_not_modify_git_config() {
        let base = unique_temp_dir("bug-d-exploration");

        // git init
        let s = Command::new("git").arg("init").arg(&base).status().unwrap();
        assert!(s.success(), "git init failed");

        // Configure identity via direct git -C args so HEAD exists for commit
        Command::new("git")
            .args(["-C", &base.to_string_lossy(), "config", "user.name", "Setup User"])
            .status()
            .unwrap();
        Command::new("git")
            .args(["-C", &base.to_string_lossy(), "config", "user.email", "setup@example.com"])
            .status()
            .unwrap();

        // Write and stage a file
        fs::write(base.join("hello.txt"), "hello").unwrap();
        Command::new("git")
            .args(["-C", &base.to_string_lossy(), "add", "hello.txt"])
            .status()
            .unwrap();

        // Clear out the [user] section written by setup so we start clean
        // (git config --remove-section user ignores error if section missing)
        let _ = Command::new("git")
            .args(["-C", &base.to_string_lossy(), "config", "--remove-section", "user"])
            .status();

        // Call the library function under test
        let result = commit_changes(&base, "test commit", "Cat Test", "cat@example.com");
        assert!(result.is_ok(), "commit_changes should succeed: {:?}", result);

        // Read .git/config and assert it does NOT contain [user]
        let git_config_path = base.join(".git").join("config");
        let config_contents = fs::read_to_string(&git_config_path)
            .expect(".git/config must be readable");

        assert!(
            !config_contents.contains("[user]"),
            "Bug D: commit_changes must not write [user] to .git/config.\n\
             Counterexample: .git/config contains:\n{}",
            config_contents
        );
    }

    /// Task 1b — Bug F exploration: get_file_status must recognise " M" as "modified"
    ///
    /// EXPECTED OUTCOME on unfixed code: FAILS
    ///   counterexample: FileStatus { status: "unknown" } for a file with porcelain code " M"
    ///
    /// Validates: Requirements 5.1
    #[test]
    fn get_file_status_recognizes_unstaged_modification() {
        let base = unique_temp_dir("bug-f-exploration");

        // git init + identity
        let s = Command::new("git").arg("init").arg(&base).status().unwrap();
        assert!(s.success(), "git init failed");
        Command::new("git")
            .args(["-C", &base.to_string_lossy(), "config", "user.name", "Cat Test"])
            .status()
            .unwrap();
        Command::new("git")
            .args(["-C", &base.to_string_lossy(), "config", "user.email", "cat@example.com"])
            .status()
            .unwrap();

        // Write, stage, and commit a file
        fs::write(base.join("tracked.txt"), "original content").unwrap();
        Command::new("git")
            .args(["-C", &base.to_string_lossy(), "add", "tracked.txt"])
            .status()
            .unwrap();
        let s = Command::new("git")
            .args(["-C", &base.to_string_lossy(), "commit", "-m", "initial"])
            .status()
            .unwrap();
        assert!(s.success(), "initial commit failed");

        // Modify the file WITHOUT staging (produces porcelain code " M")
        fs::write(base.join("tracked.txt"), "modified content").unwrap();

        // Call the library function under test
        let statuses = get_file_status(&base).expect("get_file_status should not error");

        let tracked = statuses.iter().find(|f| f.path.contains("tracked.txt"))
            .expect("tracked.txt must appear in file statuses");

        assert_eq!(
            tracked.status, "modified",
            "Bug F: porcelain code ' M' must map to 'modified', got '{}'.\n\
             Counterexample: FileStatus {{ path: {:?}, status: {:?} }}",
            tracked.status, tracked.path, tracked.status
        );
    }

    /// Task 1c — Bug C exploration: directory_size must skip symlinks (Unix only)
    ///
    /// EXPECTED OUTCOME on unfixed Unix code: FAILS
    ///   counterexample: directory_size returns more than 10 bytes (follows symlink into "other" dir)
    ///
    /// Validates: Requirements 3.1, 3.2, 3.3
    #[cfg(unix)]
    #[test]
    fn directory_size_skips_symlinks() {
        use std::os::unix::fs::symlink;

        let main_dir = unique_temp_dir("bug-c-exploration-main");
        let other_dir = unique_temp_dir("bug-c-exploration-other");

        // Write exactly 10 bytes in main_dir
        fs::write(main_dir.join("regular.bin"), b"0123456789").unwrap();

        // Put some files in other_dir (50 bytes total) so the difference is detectable
        fs::write(other_dir.join("foreign1.bin"), b"aaaaaaaaaabbbbbbbbbbccccccccccddddddddddeeeeeeeeee").unwrap();

        // Create a symlink "link" -> other_dir inside main_dir
        symlink(&other_dir, main_dir.join("link")).unwrap();

        let size = directory_size(&main_dir)
            .expect("directory_size should not error");

        assert_eq!(
            size, 10,
            "Bug C: directory_size must skip the symlink and return exactly 10 bytes (the regular file).\n\
             Counterexample: returned {} bytes instead of 10 (followed symlink into other_dir)",
            size
        );
    }

    // ────────────────────────────────────────────────────────────
    // Task 2 — Preservation property tests
    // Written BEFORE fixes.  These capture correct existing behaviour for
    // non-buggy inputs.  All three must PASS on unfixed code and continue
    // to pass after every fix (regression guard).
    // ────────────────────────────────────────────────────────────

    /// Task 2a — Bug C preservation: directory_size correct for a plain directory (no symlinks)
    ///
    /// EXPECTED OUTCOME on unfixed code: PASSES
    ///
    /// Validates: Requirements 3.3, 3.4
    #[test]
    fn directory_size_correct_for_plain_directory() {
        let dir = unique_temp_dir("bug-c-preservation");

        // Write three files with known sizes: 10 + 20 + 30 = 60 bytes
        fs::write(dir.join("a.bin"), b"0123456789").unwrap();                          // 10
        fs::write(dir.join("b.bin"), b"01234567890123456789").unwrap();                // 20
        fs::write(dir.join("c.bin"), b"012345678901234567890123456789").unwrap();      // 30

        let size = directory_size(&dir).expect("directory_size should not error");

        assert_eq!(
            size, 60,
            "directory_size must sum all regular file sizes; expected 60, got {}",
            size
        );
    }

    /// Task 2b — Bug D preservation: commit_changes records the correct author
    ///
    /// EXPECTED OUTCOME on unfixed code: PASSES (author IS written, just also leaks to .git/config)
    ///
    /// Validates: Requirements 4.3, 4.4
    #[test]
    fn commit_changes_records_correct_author() {
        let base = unique_temp_dir("bug-d-preservation");

        // git init
        let s = Command::new("git").arg("init").arg(&base).status().unwrap();
        assert!(s.success(), "git init failed");

        // Write and stage a file
        fs::write(base.join("file.txt"), "content").unwrap();
        Command::new("git")
            .args(["-C", &base.to_string_lossy(), "add", "file.txt"])
            .status()
            .unwrap();

        // Commit via library function with a specific author identity
        let result = commit_changes(&base, "preservation commit", "Alice Test", "alice@example.com");
        assert!(result.is_ok(), "commit_changes should succeed: {:?}", result);

        // Verify git log shows the correct author name and email
        let log_output = Command::new("git")
            .args(["-C", &base.to_string_lossy(), "log", "--format=%an|%ae", "-1"])
            .output()
            .expect("git log failed");
        let log_str = String::from_utf8_lossy(&log_output.stdout);
        let log_str = log_str.trim();

        assert!(
            log_str.contains("Alice Test"),
            "git log must show author name 'Alice Test', got: {}",
            log_str
        );
        assert!(
            log_str.contains("alice@example.com"),
            "git log must show author email 'alice@example.com', got: {}",
            log_str
        );
    }

    /// Task 2c — Bug F preservation: existing staged-code mappings are unchanged
    ///
    /// Verifies that the three most common pre-existing match arms still return
    /// the correct status strings after any future fix:
    ///   "A " → "added"    (newly staged file)
    ///   "M " → "modified" (staged modification of a tracked file)
    ///   "??" → "untracked" (untracked file)
    ///
    /// EXPECTED OUTCOME on unfixed code: PASSES
    ///
    /// Validates: Requirements 5.7
    #[test]
    fn get_file_status_existing_codes_unchanged() {
        let base = unique_temp_dir("bug-f-preservation");

        // git init + identity
        let s = Command::new("git").arg("init").arg(&base).status().unwrap();
        assert!(s.success(), "git init failed");
        Command::new("git")
            .args(["-C", &base.to_string_lossy(), "config", "user.name", "Cat Test"])
            .status()
            .unwrap();
        Command::new("git")
            .args(["-C", &base.to_string_lossy(), "config", "user.email", "cat@example.com"])
            .status()
            .unwrap();

        // Seed commit so HEAD exists (needed for "M " to appear in porcelain)
        fs::write(base.join("tracked.txt"), "initial").unwrap();
        Command::new("git")
            .args(["-C", &base.to_string_lossy(), "add", "tracked.txt"])
            .status()
            .unwrap();
        let s = Command::new("git")
            .args(["-C", &base.to_string_lossy(), "commit", "-m", "seed"])
            .status()
            .unwrap();
        assert!(s.success(), "seed commit failed");

        // Case 1: staged NEW file → "A " → "added"
        fs::write(base.join("new_file.txt"), "new").unwrap();
        Command::new("git")
            .args(["-C", &base.to_string_lossy(), "add", "new_file.txt"])
            .status()
            .unwrap();

        // Case 2: staged modification of tracked file → "M " → "modified"
        fs::write(base.join("tracked.txt"), "modified").unwrap();
        Command::new("git")
            .args(["-C", &base.to_string_lossy(), "add", "tracked.txt"])
            .status()
            .unwrap();

        // Case 3: untracked file → "??" → "untracked"
        fs::write(base.join("untracked.txt"), "not staged").unwrap();

        let statuses = get_file_status(&base).expect("get_file_status should not error");

        let find_status = |name: &str| -> Option<String> {
            statuses.iter()
                .find(|f| f.path.contains(name))
                .map(|f| f.status.clone())
        };

        assert_eq!(
            find_status("new_file.txt").as_deref(),
            Some("added"),
            "staged new file (code 'A ') must map to 'added'"
        );
        assert_eq!(
            find_status("tracked.txt").as_deref(),
            Some("modified"),
            "staged modification (code 'M ') must map to 'modified'"
        );
        assert_eq!(
            find_status("untracked.txt").as_deref(),
            Some("untracked"),
            "untracked file (code '??') must map to 'untracked'"
        );
    }

    /// Task 1c (legacy label) — Bug 2 exploration (core layer)
    ///
    /// `discover_repositories` should find a git repo placed DIRECTLY inside
    /// the given root (not buried in a `repositories/` subdirectory).
    /// The scan-root bug lives in the GUI `refresh()` call site; the core
    /// function itself is correct, so this test PASSES on unfixed code.
    ///
    /// EXPECTED OUTCOME on unfixed code: PASSES
    ///   Note: bug 2 is in GUI refresh() — `let root = self.drive_root.join("repositories")`
    ///   narrows the scan to a subdirectory.  Core discover_repositories is fine.
    ///
    /// Validates: Requirements 2.1
    #[test]
    fn discover_repos_finds_repo_at_drive_root() {
        let drive_root = unique_temp_dir("drive-root-scan");

        // Create a repo directly inside drive_root (no repositories/ subdir)
        let repo_dir = drive_root.join("my-direct-repo");
        fs::create_dir_all(&repo_dir).unwrap();
        let status = Command::new("git")
            .arg("init")
            .arg(&repo_dir)
            .status()
            .expect("git init failed");
        assert!(status.success(), "git init must succeed");

        let discovered = discover_repositories(&drive_root)
            .expect("discover_repositories should not error");

        let repo_path = repo_dir.to_string_lossy().to_string();
        assert!(
            discovered.iter().any(|r| r.local_path == repo_path),
            "discover_repositories must find a repo placed directly at drive_root; found: {:?}",
            discovered.iter().map(|r| &r.local_path).collect::<Vec<_>>()
        );
    }
}
