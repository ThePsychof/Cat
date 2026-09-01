use std::path::Path;
use std::process::Command;

use cat_core::{
    discover_repositories, get_current_branch, compare_with_remote, list_branches,
    switch_branch, create_branch, clone_repository, clone_from_drive, list_files,
    get_commit_log, get_file_status, DriveState, GitProfile, RepoSyncState, RepositoryRecord,
};

fn print_usage() {
    eprintln!(
        r#"Usage: cat-cli <command> [options]

Commands:
  init <path>                      initialize a Cat drive
  status                          print current drive state
  open <repo-path>                open repo in external editor
  branch <repo-path>              list branches in a repository
  branch-switch <repo-path> <name> switch to a branch
  branch-create <repo-path> <name> create a new branch
  clone <url> <target>            clone from GitHub to target directory
  clone-from-drive <source> <target> clone repository from drive to target location
  ls-files <repo-path>            list files in repository
  log <repo-path> [count]         show commit history (default 20)
  diff <repo-path>                show changed files (comparison)
"#
    );
}

fn command_init() {
    let mut state = DriveState::new();
    state.selected_os = vec!["windows".to_string(), "macos".to_string(), "linux".to_string()];
    state.add_profile(GitProfile {
        name: "default".to_string(),
        user_name: "Cat User".to_string(),
        user_email: "cat@example.com".to_string(),
    });
    state.add_repository(RepositoryRecord {
        name: "example-repo".to_string(),
        remote_url: "https://github.com/example/example-repo".to_string(),
        local_path: "/Volumes/Cat/repos/example-repo".to_string(),
        read_only: false,
        last_synced_at: Some("2026-09-01T00:00:00Z".to_string()),
        last_synced_sha: Some("abc123".to_string()),
        size_bytes: Some(4096),
        sync_state: RepoSyncState::UpToDate,
    });

    println!("Prepared Cat drive state for portable repository vault.");
    println!("Selected OS targets: {:?}", state.selected_os);
    println!("Profiles: {}", state.profiles.len());
    println!("Repositories: {}", state.repositories.len());
}

fn command_status() {
    let root = Path::new(".");
    let repos = discover_repositories(root).unwrap_or_default();
    let state = DriveState::new();

    println!("\n🐈 Cat drive status (v{})", state.version);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Profiles: {} | Repositories: {}\n", state.profiles.len(), repos.len());

    for repo in repos {
        let branch = get_current_branch(Path::new(&repo.local_path))
            .unwrap_or_else(|_| "unknown".to_string());

        let status_info = if !repo.remote_url.is_empty() {
            match compare_with_remote(Path::new(&repo.local_path), "origin", &branch) {
                Ok((ahead, behind)) => {
                    if ahead > 0 && behind > 0 {
                        format!("↕ diverged (+{}/{}) [{}]", ahead, behind, branch)
                    } else if ahead > 0 {
                        format!("↑ ahead +{} [{}]", ahead, branch)
                    } else if behind > 0 {
                        format!("↓ behind -{} [{}]", behind, branch)
                    } else {
                        format!("✓ up-to-date [{}]", branch)
                    }
                }
                Err(_) => format!("? unknown [{}]", branch),
            }
        } else {
            format!("● local only [{}]", branch)
        };

        println!("  {}", repo.name);
        println!("    {}", status_info);
        if !repo.remote_url.is_empty() {
            println!("    → {}", repo.remote_url);
        }
    }
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
}

fn open_in_editor(repo_path: &str, editor: &str) -> Result<(), String> {
    Command::new(editor)
        .arg(repo_path)
        .spawn()
        .map_err(|e| format!("Failed to open in {}: {e}", editor))?
        .wait()
        .map(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("Editor exited with status {}", status))
            }
        })
        .map_err(|e| e.to_string())?
}

fn command_open(repo_path: &str) {
    let editor = std::env::var("CAT_EDITOR").unwrap_or_else(|_| "code".to_string());
    match open_in_editor(repo_path, &editor) {
        Ok(_) => println!("🐈 Opening {} in {}...", repo_path, editor),
        Err(e) => eprintln!("Error: {e}"),
    }
}

fn command_branch(repo_path: &str) {
    match list_branches(Path::new(repo_path)) {
        Ok(branches) => {
            println!("🐈 Branches in {}:", repo_path);
            for branch in branches {
                let marker = if branch.is_current { "→" } else { "  " };
                println!("{} {}", marker, branch.name);
            }
        }
        Err(e) => eprintln!("Error: {e}"),
    }
}

fn command_switch_branch(repo_path: &str, branch_name: &str) {
    match switch_branch(Path::new(repo_path), branch_name) {
        Ok(_) => println!("🐈 Switched to branch '{}'", branch_name),
        Err(e) => eprintln!("Error: {e}"),
    }
}

fn command_create_branch(repo_path: &str, branch_name: &str) {
    match create_branch(Path::new(repo_path), branch_name) {
        Ok(_) => println!("🐈 Created and switched to branch '{}'", branch_name),
        Err(e) => eprintln!("Error: {e}"),
    }
}

fn command_clone(url: &str, target: &str) {
    match clone_repository(url, Path::new(target)) {
        Ok(_) => println!("🐈 Cloned {} to {}", url, target),
        Err(e) => eprintln!("Error: {e}"),
    }
}

fn command_clone_from_drive(source: &str, target: &str) {
    match clone_from_drive(Path::new(source), Path::new(target)) {
        Ok(_) => println!("🐈 Cloned from {} to {}", source, target),
        Err(e) => eprintln!("Error: {e}"),
    }
}

fn command_ls_files(repo_path: &str) {
    match list_files(Path::new(repo_path)) {
        Ok(files) => {
            for file in files {
                println!("{}", file);
            }
        }
        Err(e) => eprintln!("Error: {e}"),
    }
}

fn command_log(repo_path: &str, max_count: usize) {
    match get_commit_log(Path::new(repo_path), max_count) {
        Ok(commits) => {
            for (idx, commit) in commits.iter().enumerate() {
                if idx > 0 {
                    println!();
                }
                println!("{}", &commit.sha[..7.min(commit.sha.len())]);
                println!("{} <{}>", commit.author, commit.email);
                println!("{}", commit.date);
                println!("{}", commit.message);
            }
        }
        Err(e) => eprintln!("Error: {e}"),
    }
}

fn command_diff(repo_path: &str) {
    match get_file_status(Path::new(repo_path)) {
        Ok(files) => {
            if files.is_empty() {
                println!("✓ No changes — working directory is clean");
            } else {
                println!("🐈 File changes:");
                for file in files {
                    let icon = match file.status.as_str() {
                        "modified" => "⚙",
                        "added" => "🟢",
                        "deleted" => "🔴",
                        "renamed" => "➜",
                        "untracked" => "?",
                        _ => "•",
                    };
                    println!("  {} {} ({})", icon, file.path, file.status);
                }
            }
        }
        Err(e) => eprintln!("Error: {e}"),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("init") => command_init(),
        Some("status") => command_status(),
        Some("open") => {
            if let Some(path) = args.next() {
                command_open(&path);
            } else {
                eprintln!("Error: open requires a repository path");
                print_usage();
                std::process::exit(1);
            }
        }
        Some("branch") => {
            if let Some(path) = args.next() {
                command_branch(&path);
            } else {
                eprintln!("Error: branch requires a repository path");
                print_usage();
                std::process::exit(1);
            }
        }
        Some("branch-switch") => {
            if let (Some(path), Some(name)) = (args.next(), args.next()) {
                command_switch_branch(&path, &name);
            } else {
                eprintln!("Error: branch-switch requires path and branch name");
                print_usage();
                std::process::exit(1);
            }
        }
        Some("branch-create") => {
            if let (Some(path), Some(name)) = (args.next(), args.next()) {
                command_create_branch(&path, &name);
            } else {
                eprintln!("Error: branch-create requires path and branch name");
                print_usage();
                std::process::exit(1);
            }
        }
        Some("clone") => {
            if let (Some(url), Some(target)) = (args.next(), args.next()) {
                command_clone(&url, &target);
            } else {
                eprintln!("Error: clone requires url and target");
                print_usage();
                std::process::exit(1);
            }
        }
        Some("clone-from-drive") => {
            if let (Some(source), Some(target)) = (args.next(), args.next()) {
                command_clone_from_drive(&source, &target);
            } else {
                eprintln!("Error: clone-from-drive requires source and target");
                print_usage();
                std::process::exit(1);
            }
        }
        Some("ls-files") => {
            if let Some(path) = args.next() {
                command_ls_files(&path);
            } else {
                eprintln!("Error: ls-files requires a repository path");
                print_usage();
                std::process::exit(1);
            }
        }
        Some("log") => {
            if let Some(path) = args.next() {
                let max_count = args.next().and_then(|s| s.parse::<usize>().ok()).unwrap_or(20);
                command_log(&path, max_count);
            } else {
                eprintln!("Error: log requires a repository path");
                print_usage();
                std::process::exit(1);
            }
        }
        Some("diff") => {
            if let Some(path) = args.next() {
                command_diff(&path);
            } else {
                eprintln!("Error: diff requires a repository path");
                print_usage();
                std::process::exit(1);
            }
        }
        Some(_) | None => {
            print_usage();
            std::process::exit(1);
        }
    }
}
