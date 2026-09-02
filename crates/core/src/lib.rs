use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

pub fn compare_repo_with_origin(repo_dir: &Path) -> Result<Vec<FileComparison>, String> {
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

    for entry in
        fs::read_dir(dir).map_err(|e| format!("Failed to read directory {}: {e}", dir.display()))?
    {
        let entry = entry
            .map_err(|e| format!("Failed to read directory entry in {}: {e}", dir.display()))?;
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if name == ".git" || name == ".cat" || name == ".DS_Store" {
            continue;
        }

        if path.is_dir() {
            scan_for_repositories(&path, repositories, visited)?;
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

    if text.contains("??")
        || text.contains(" M")
        || text.contains("M ")
        || text.contains("A ")
        || text.contains("D ")
        || text.contains("R ")
    {
        return Ok(RepoSyncState::Modified);
    }

    if text.contains("No commits yet") {
        return Ok(RepoSyncState::Modified);
    }

    Ok(RepoSyncState::UpToDate)
}

fn directory_size(path: &Path) -> Result<u64, std::io::Error> {
    fn walk(dir: &Path) -> Result<u64, std::io::Error> {
        let mut total = 0u64;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                total += walk(&path)?;
            } else {
                total += metadata.len();
            }
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

    if let Some(token) = token {
        let encoded = base64::engine::general_purpose::STANDARD.encode(format!("x-access-token:{token}"));
        full_args.push("-c".to_string());
        full_args.push(format!("http.extraHeader=Authorization: Basic {encoded}"));
    }

    full_args.extend(args.iter().map(|s| s.to_string()));

    let output = Command::new(git)
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
        Err("Portable Git is missing from the Cat drive (.cat/tools/git)".into())
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

pub fn clone_repository(url: &str, target_dir: &Path) -> Result<(), String> {
    let git = git_program(target_dir.parent().unwrap_or(target_dir))?;
    let output = Command::new(git)
        .arg("clone")
        .arg(url)
        .arg(target_dir)
        .output()
        .map_err(|e| format!("Unable to clone {}: {e}", url))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
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
    git_output(repo_dir, &["config", "user.name", author_name])?;
    git_output(repo_dir, &["config", "user.email", author_email])?;
    let output = git_output(repo_dir, &["commit", "-m", message])?;
    Ok(output.trim().to_string())
}

pub fn open_in_editor(repo_dir: &Path, editor: &str) -> Result<(), String> {
    Command::new(editor)
        .arg(repo_dir)
        .spawn()
        .map_err(|e| format!("Failed to open {} in {}: {e}", repo_dir.display(), editor))?;
    Ok(())
}

pub fn clone_from_drive(source_repo_path: &Path, target_dir: &Path) -> Result<(), String> {
    let source_path = source_repo_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve source path: {e}"))?
        .to_string_lossy()
        .to_string();

    let git = git_program(source_repo_path)?;
    let output = Command::new(git)
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
        discover_repositories, get_commit_log, get_file_status, stage_all_changes,
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
}
