use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

#[derive(Serialize)]
struct DriveState<'a> {
    version: &'a str,
    selected_os: Vec<&'a str>,
    active_profile: Option<&'a str>,
    profiles: Vec<Profile<'a>>,
    repositories: Vec<String>,
}

#[derive(Serialize)]
struct Profile<'a> {
    name: &'a str,
    user_name: &'a str,
    user_email: &'a str,
}

fn usage() {
    eprintln!("Usage: mewmew init [drive-path] [--mode format|update|append] [--cat-binary path]");
}

fn main() {
    if let Err(error) = run() {
        eprintln!("mewmew: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    if args.next().as_deref() != Some("init") {
        usage();
        return Err("expected the init command".into());
    }
    let mut drive = PathBuf::from(".");
    let mut mode = "append".to_string();
    let mut cat_binary = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mode" => {
                mode = args
                    .next()
                    .ok_or("--mode requires format, update, or append")?
            }
            "--cat-binary" => {
                cat_binary = Some(PathBuf::from(
                    args.next().ok_or("--cat-binary requires a path")?,
                ))
            }
            value if !value.starts_with('-') && drive == PathBuf::from(".") => {
                drive = PathBuf::from(value)
            }
            _ => {
                usage();
                return Err(format!("unknown argument: {arg}"));
            }
        }
    }
    if !matches!(mode.as_str(), "format" | "update" | "append") {
        return Err(format!("unsupported mode: {mode}"));
    }
    provision(&drive, &mode, cat_binary.as_deref())
}

fn provision(drive: &Path, mode: &str, cat_binary: Option<&Path>) -> Result<(), String> {
    fs::create_dir_all(drive)
        .map_err(|e| format!("cannot access drive {}: {e}", drive.display()))?;
    let repositories = drive.join("repositories");
    let profiles = drive.join("profiles");
    let metadata = drive.join(".cat");
    if mode == "format" {
        remove_if_present(&repositories)?;
        remove_if_present(&profiles)?;
        remove_if_present(&metadata)?;
    }
    fs::create_dir_all(&repositories).map_err(|e| format!("cannot create repositories: {e}"))?;
    fs::create_dir_all(&profiles).map_err(|e| format!("cannot create profiles: {e}"))?;
    fs::create_dir_all(&metadata).map_err(|e| format!("cannot create metadata: {e}"))?;
    install_git(&metadata)?;
    install_cat(drive, cat_binary.as_deref())?;
    install_icon(&metadata)?;
    let state = DriveState {
        version: env!("CARGO_PKG_VERSION"),
        selected_os: vec!["windows"],
        active_profile: None,
        profiles: Vec::new(),
        repositories: Vec::new(),
    };
    let state_json =
        serde_json::to_vec_pretty(&state).map_err(|e| format!("cannot encode state: {e}"))?;
    fs::write(metadata.join("state.json"), state_json)
        .map_err(|e| format!("cannot write state: {e}"))?;
    let autorun_path = drive.join("autorun.inf");
    fs::write(
        &autorun_path,
        "[autorun]\r\nlabel=Cat\r\nicon=.cat\\cat-icon.ico\r\n",
    )
    .map_err(|e| format!("cannot write autorun.inf: {e}"))?;

    brand_drive(drive, &autorun_path)?;

    println!("Cat drive prepared in {mode} mode: {}", drive.display());
    Ok(())
}

#[cfg(windows)]
fn brand_drive(drive: &Path, autorun_path: &Path) -> Result<(), String> {
    use std::process::Command;

    // Windows only reliably honors autorun.inf when it carries System+Hidden
    // attributes, matching how manufacturer USB sticks ship theirs. This is
    // cosmetic — a failure here shouldn't block provisioning.
    let _ = Command::new("attrib")
        .args(["+s", "+h", &autorun_path.to_string_lossy()])
        .status();

    // The real persistent volume label (what "This PC" shows) is separate
    // from autorun.inf's label= line, which only affects some legacy
    // Explorer contexts. `label` is a cmd builtin, not a standalone exe.
    let drive_letter = drive
        .canonicalize()
        .unwrap_or_else(|_| drive.to_path_buf())
        .to_string_lossy()
        .chars()
        .take(2)
        .collect::<String>();

    if !drive_letter.is_empty() {
        let _ = Command::new("cmd")
            .args(["/C", "label", &drive_letter, "Cat"])
            .status();
    }

    Ok(())
}

#[cfg(not(windows))]
fn brand_drive(_drive: &Path, _autorun_path: &Path) -> Result<(), String> {
    Ok(())
}

const CAT_ICON: &[u8] = include_bytes!("../../../cat-icon.ico");

fn install_icon(metadata: &Path) -> Result<(), String> {
    fs::write(metadata.join("cat-icon.ico"), CAT_ICON)
        .map_err(|e| format!("cannot write cat-icon.ico: {e}"))
}

fn install_cat(drive: &Path, requested: Option<&Path>) -> Result<(), String> {
    let source = requested
        .map(PathBuf::from)
        .or_else(|| env::var_os("CAT_BINARY").map(PathBuf::from))
        .or_else(|| {
            let exe = env::current_exe().ok()?;
            let candidate = exe
                .parent()?
                .join(if cfg!(windows) { "cat.exe" } else { "cat" });
            candidate.exists().then_some(candidate)
        });
    let Some(source) = source else {
        return Err("Rust Cat binary not found; pass --cat-binary or set CAT_BINARY".into());
    };
    if !source.exists() {
        return Err(format!("Cat binary does not exist: {}", source.display()));
    }
    let destination = drive.join(if cfg!(windows) { "Cat.exe" } else { "Cat" });
    fs::copy(&source, &destination).map_err(|e| format!("cannot place Cat application: {e}"))?;
    Ok(())
}

fn remove_if_present(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .map_err(|e| format!("cannot remove {}: {e}", path.display()))
}

fn install_git(metadata: &Path) -> Result<(), String> {
    let source = env::var_os("CAT_GIT_BINARY").map(PathBuf::from);
    let Some(source) = source else {
        return Ok(());
    };
    if !source.is_file() {
        return Err(format!(
            "portable Git binary does not exist: {}",
            source.display()
        ));
    }
    let tools = metadata.join("tools");
    fs::create_dir_all(&tools).map_err(|e| format!("cannot create Git tools directory: {e}"))?;
    let destination = tools.join(if cfg!(windows) { "git.exe" } else { "git" });
    fs::copy(source, destination).map_err(|e| format!("cannot place portable Git: {e}"))?;
    Ok(())
}
