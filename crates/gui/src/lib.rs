use std::path::PathBuf;

use cat_core::{
    BranchInfo, CommitInfo, FileCompareStatus, FileComparison, GitProfile,
    RepositoryRecord, TreeNode, build_file_tree, clone_repository,
    compare_repo_with_origin, discover_repositories, fetch_repository,
    get_commit_log, get_token, list_branches, load_drive_state, open_in_editor, pull_repository,
    read_file_at_head, save_drive_state, set_token, DriveState,
};
use eframe::egui;

mod mascot;
use mascot::{CatMood, WebViewMascot};

pub fn app_name() -> &'static str {
    "Cat"
}

pub fn drive_root() -> PathBuf {
    let Some(exe) = std::env::current_exe().ok() else {
        return PathBuf::from(".");
    };

    let mut candidate = exe.parent();
    while let Some(path) = candidate {
        let metadata = path.join(".cat");
        let cat_binary = path.join(if cfg!(windows) { "Cat.exe" } else { "Cat" });
        if metadata.is_dir() && cat_binary.is_file() {
            return path.to_path_buf();
        }
        candidate = path.parent();
    }

    exe.parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

struct CatApp {
    drive_root: PathBuf,
    repositories: Vec<RepositoryRecord>,
    selected_index: usize,
    clone_url_input: String,
    status_message: String,
    status_is_error: bool,
    comparison: Option<Vec<FileComparison>>,
    file_tree: Option<Vec<TreeNode>>,
    selected_file: Option<String>,
    selected_file_content: Option<Result<String, String>>,
    branches: Vec<BranchInfo>,
    commit_log: Vec<CommitInfo>,
    view_mode: ViewMode,
    drive_state: DriveState,
    passphrase: String,
    show_profile_modal: bool,
    new_profile_name: String,
    new_profile_user_name: String,
    new_profile_user_email: String,
    new_profile_token: String,
    mascot: WebViewMascot,
}

#[derive(PartialEq, Default)]
enum ViewMode {
    #[default]
    Files,
    Commits,
}

impl CatApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(12, 14, 20);
        visuals.window_fill = egui::Color32::from_rgb(17, 20, 26);
        visuals.extreme_bg_color = egui::Color32::from_rgb(8, 10, 15);
        visuals.selection.bg_fill = egui::Color32::from_rgb(251, 146, 60);
        // Selected text must contrast against the orange fill — using the
        // same orange for both (the original bug) made selected labels like
        // the "Files" tab render invisible: orange text on orange background.
        visuals.selection.stroke.color = egui::Color32::from_rgb(26, 15, 8);
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(23, 27, 34);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(35, 41, 51);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(251, 146, 60);
        
        // Improve button styling with rounded corners
        visuals.widgets.noninteractive.weak_bg_fill = egui::Color32::from_rgb(31, 37, 45);
        
        cc.egui_ctx.set_visuals(visuals);

        let root = drive_root();
        let drive_state = load_drive_state(&root).unwrap_or_default();

        let mut app = Self {
            drive_root: root,
            repositories: Vec::new(),
            selected_index: 0,
            clone_url_input: String::new(),
            status_message: String::new(),
            status_is_error: false,
            comparison: None,
            file_tree: None,
            selected_file: None,
            selected_file_content: None,
            branches: Vec::new(),
            commit_log: Vec::new(),
            view_mode: ViewMode::default(),
            drive_state,
            passphrase: String::new(),
            show_profile_modal: false,
            new_profile_name: String::new(),
            new_profile_user_name: String::new(),
            new_profile_user_email: String::new(),
            new_profile_token: String::new(),
            mascot: WebViewMascot::new(),
        };
        app.refresh();
        app
    }

    fn active_profile(&self) -> Option<&GitProfile> {
        let name = self.drive_state.active_profile.as_ref()?;
        self.drive_state.profiles.iter().find(|p| &p.name == name)
    }

    fn current_token(&self) -> Option<String> {
        let profile = self.active_profile()?;
        if self.passphrase.is_empty() {
            return None;
        }
        get_token(&self.drive_root, &profile.name, &self.passphrase).ok().flatten()
    }

    fn handle_create_profile(&mut self) {
        let name = self.new_profile_name.trim().to_string();
        let user_name = self.new_profile_user_name.trim().to_string();
        let user_email = self.new_profile_user_email.trim().to_string();
        let token = self.new_profile_token.trim().to_string();

        if name.is_empty() || user_name.is_empty() || user_email.is_empty() || self.passphrase.is_empty() {
            self.set_status("Profile name, identity, and a passphrase are all required.", true);
            return;
        }

        self.drive_state.add_profile(GitProfile {
            name: name.clone(),
            user_name,
            user_email,
        });

        if !token.is_empty() {
            if let Err(err) = set_token(&self.drive_root, &name, &token, &self.passphrase) {
                self.set_status(format!("Saved profile, but couldn't store token: {err}"), true);
            }
        }

        if let Err(err) = save_drive_state(&self.drive_root, &self.drive_state) {
            self.set_status(format!("Couldn't save drive state: {err}"), true);
            return;
        }

        self.show_profile_modal = false;
        self.new_profile_name.clear();
        self.new_profile_user_name.clear();
        self.new_profile_user_email.clear();
        self.new_profile_token.clear();
        self.set_status(format!("Created profile {name}."), false);
    }

    fn handle_fetch(&mut self, repo_path: &str) {
        let token = self.current_token();
        match fetch_repository(std::path::Path::new(repo_path), "origin", token.as_deref()) {
            Ok(()) => {
                self.set_status("Fetched from origin.", false);
                self.load_tree_for_selected();
            }
            Err(err) => self.set_status(format!("Fetch failed: {err}"), true),
        }
    }

    fn handle_pull(&mut self, repo_path: &str) {
        let branch = self
            .branches
            .iter()
            .find(|b| b.is_current)
            .map(|b| b.name.clone())
            .unwrap_or_else(|| "main".to_string());
        let token = self.current_token();
        match pull_repository(std::path::Path::new(repo_path), "origin", &branch, token.as_deref()) {
            Ok(()) => {
                self.set_status(format!("Pulled {branch}."), false);
                self.mascot.set_mood(CatMood::Happy);
                self.load_tree_for_selected();
            }
            Err(err) => {
                self.set_status(format!("Pull failed: {err}"), true);
                self.mascot.set_mood(CatMood::Sad);
            }
        }
    }

    fn refresh(&mut self) {
        let root = self.drive_root.clone();
        match discover_repositories(&root) {
            Ok(repositories) => {
                self.repositories = repositories;
                self.set_status("Repository list refreshed.", false);
            }
            Err(err) => {
                self.set_status(format!("Couldn't scan {}: {err}", root.display()), true);
                return;
            }
        }
        if self.selected_index >= self.repositories.len() {
            self.selected_index = self.repositories.len().saturating_sub(1);
        }
        self.drive_state.repositories = self.repositories.clone();
        let _ = save_drive_state(&self.drive_root, &self.drive_state);
        self.load_tree_for_selected();
    }

    fn load_tree_for_selected(&mut self) {
        let repo_path = self.selected_repo().map(|r| r.local_path.clone());

        self.file_tree = repo_path
            .as_ref()
            .and_then(|p| build_file_tree(std::path::Path::new(p)).ok());
        self.branches = repo_path
            .as_ref()
            .and_then(|p| list_branches(std::path::Path::new(p)).ok())
            .unwrap_or_default();
        self.commit_log = repo_path
            .as_ref()
            .and_then(|p| get_commit_log(std::path::Path::new(p), 50).ok())
            .unwrap_or_default();
        self.selected_file = None;
        self.selected_file_content = None;
        self.comparison = None;
    }

    fn open_file(&mut self, file_path: String) {
        if let Some(repo) = self.selected_repo() {
            let repo_path = repo.local_path.clone();
            let content = read_file_at_head(std::path::Path::new(&repo_path), &file_path);
            self.selected_file = Some(file_path);
            self.selected_file_content = Some(content);
        }
    }

    fn selected_repo(&self) -> Option<&RepositoryRecord> {
        self.repositories.get(self.selected_index)
    }

    fn set_status(&mut self, message: impl Into<String>, is_error: bool) {
        self.status_message = message.into();
        self.status_is_error = is_error;
    }

    fn handle_clone(&mut self) {
        let url = self.clone_url_input.trim().to_string();
        if url.is_empty() {
            self.set_status("Enter a repository URL first.", true);
            return;
        }

        let repo_name = url
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("repo")
            .trim_end_matches(".git")
            .to_string();

        let repos_root = self.drive_root.join("repositories");
        let target_dir = repos_root.join(&repo_name);

        if target_dir.exists() {
            self.set_status(format!("{repo_name} already exists on this drive."), true);
            return;
        }

        if let Err(err) = std::fs::create_dir_all(&repos_root) {
            self.set_status(format!("Could not prepare repositories folder: {err}"), true);
            return;
        }

        self.mascot.set_mood(CatMood::Working);
        match clone_repository(&url, &target_dir, self.current_token().as_deref()) {
            Ok(()) => {
                self.set_status(format!("Cloned {repo_name}."), false);
                self.mascot.set_mood(CatMood::Happy);
                self.clone_url_input.clear();
                self.refresh();
            }
            Err(err) => {
                self.set_status(format!("Clone failed: {err}"), true);
                self.mascot.set_mood(CatMood::Sad);
            }
        }
    }
}

impl eframe::App for CatApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {

        let dt = ui.ctx().input(|i| i.stable_dt);
        self.mascot.tick(dt);

        egui::Panel::top("cat_header")
            .show(ui, |ui| {
                ui.set_height(240.0);
                ui.vertical(|ui| {
                    ui.add_space(12.0);
                    // Header row with mascot and title
                    ui.horizontal(|ui| {
                        self.mascot.show(ui);
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("Cat")
                                    .size(24.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(251, 146, 60)),
                            );
                            ui.label(egui::RichText::new("portable repository vault").size(11.0).color(egui::Color32::from_rgb(156, 165, 180)));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("🔄").on_hover_text("Refresh repository list").clicked() {
                                self.refresh();
                            }
                            if ui.button("+ Profile").clicked() {
                                self.show_profile_modal = true;
                            }
                            let profile_label = self
                                .active_profile()
                                .map(|p| p.name.clone())
                                .unwrap_or_else(|| "no profile".to_string());
                            egui::ComboBox::from_id_salt("profile_selector")
                                .selected_text(&profile_label)
                                .show_ui(ui, |ui| {
                                    let profiles = self.drive_state.profiles.clone();
                                    for profile in profiles {
                                        let is_active = self.drive_state.active_profile.as_deref()
                                            == Some(profile.name.as_str());
                                        if ui.selectable_label(is_active, &profile.name).clicked() {
                                            self.drive_state.active_profile = Some(profile.name);
                                            let _ = save_drive_state(&self.drive_root, &self.drive_state);
                                        }
                                    }
                                });
                            let has_profiles = !self.drive_state.profiles.is_empty();
                            ui.add_enabled(
                                has_profiles,
                                egui::TextEdit::singleline(&mut self.passphrase)
                                    .password(true)
                                    .hint_text("passphrase")
                                    .desired_width(110.0),
                            )
                            .on_hover_text(if has_profiles {
                                "Unlocks the active profile's stored GitHub token"
                            } else {
                                "Create a profile first to unlock credentials"
                            });
                        });
                    });

                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(12.0);
                    
                    // Clone section
                    ui.horizontal(|ui| {
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.clone_url_input)
                                .hint_text("git remote URL")
                                .desired_width(400.0),
                        );
                        let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let clone_btn = egui::Button::new(
                            egui::RichText::new("Clone").strong().color(egui::Color32::from_rgb(26, 15, 8)),
                        )
                        .fill(egui::Color32::from_rgb(251, 146, 60));

                        if ui.add(clone_btn).clicked() || submitted {
                            self.handle_clone();
                        }
                    });

                    // Status message with improved styling
                    if !self.status_message.is_empty() {
                        ui.add_space(10.0);
                        let color = if self.status_is_error {
                            egui::Color32::from_rgb(248, 113, 113)
                        } else {
                            egui::Color32::from_rgb(134, 194, 172)
                        };
                        let bg_color = if self.status_is_error {
                            egui::Color32::from_rgb(30, 16, 16)
                        } else {
                            egui::Color32::from_rgb(16, 30, 26)
                        };
                        let frame = egui::Frame::default()
                            .fill(bg_color)
                            .inner_margin(egui::Margin::symmetric(10, 8));
                        frame.show(ui, |ui| {
                            ui.label(egui::RichText::new(&self.status_message).size(10.0).color(color));
                        });
                    }
                    ui.add_space(8.0);
                });
            });

        if self.show_profile_modal {
            egui::Window::new("New Profile")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Profile Details").strong().size(12.0));
                    ui.add_space(8.0);
                    
                    ui.add(egui::TextEdit::singleline(&mut self.new_profile_name).hint_text("Profile name").desired_width(f32::INFINITY));
                    ui.add(egui::TextEdit::singleline(&mut self.new_profile_user_name).hint_text("Git user.name").desired_width(f32::INFINITY));
                    ui.add(egui::TextEdit::singleline(&mut self.new_profile_user_email).hint_text("Git user.email").desired_width(f32::INFINITY));
                    
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Credentials").strong().size(12.0));
                    ui.add_space(6.0);
                    
                    ui.add(egui::TextEdit::singleline(&mut self.new_profile_token).password(true).hint_text("GitHub token (optional)").desired_width(f32::INFINITY));
                    ui.add(egui::TextEdit::singleline(&mut self.passphrase).password(true).hint_text("Passphrase (required)").desired_width(f32::INFINITY));
                    
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        let create_btn = egui::Button::new(
                            egui::RichText::new("Create").strong().color(egui::Color32::from_rgb(26, 15, 8)),
                        )
                        .fill(egui::Color32::from_rgb(251, 146, 60));
                        
                        if ui.add(create_btn).clicked() {
                            self.handle_create_profile();
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_profile_modal = false;
                        }
                    });
                });
        }

        egui::Panel::left("repos")
            .default_size(300.0)
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Repositories").size(16.0).strong());
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                if self.repositories.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(32.0);
                        ui.label(egui::RichText::new("🐾").size(32.0));
                        ui.label(egui::RichText::new("No repositories").size(14.0).strong());
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Clone a repository above, or place repos in the repositories/ folder.").size(10.0).color(egui::Color32::from_rgb(156, 165, 180)));
                    });
                    return;
                }

                let mut clicked_index: Option<usize> = None;

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (idx, repo) in self.repositories.iter().enumerate() {
                        let selected = idx == self.selected_index;
                        let bg_color = if selected {
                            egui::Color32::from_rgb(35, 41, 51)
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        
                        let frame = egui::Frame::default()
                            .fill(bg_color)
                            .inner_margin(egui::Margin::symmetric(8, 8));
                        
                        frame.show(ui, |ui| {
                            ui.vertical(|ui| {
                                let repo_text = format!("{} {}", if selected { "▶" } else { "•" }, repo.name);
                                if ui.selectable_label(false, egui::RichText::new(repo_text).strong().color(if selected { egui::Color32::from_rgb(251, 146, 60) } else { egui::Color32::WHITE })).clicked() {
                                    clicked_index = Some(idx);
                                }
                                ui.label(egui::RichText::new(&repo.remote_url).size(9.0).color(egui::Color32::from_rgb(156, 165, 180)));
                            });
                        });
                        
                        if idx < self.repositories.len() - 1 {
                            ui.add_space(2.0);
                        }
                    }
                });

                if let Some(idx) = clicked_index {
                    self.selected_index = idx;
                    self.load_tree_for_selected();
                }
            });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(4.0);
            if let Some(file_path) = self.selected_file.clone() {
                ui.horizontal(|ui| {
                    if ui.button("← Back").clicked() {
                        self.selected_file = None;
                        self.selected_file_content = None;
                    }
                    ui.label(egui::RichText::new(&file_path).strong().size(13.0));
                    ui.label(
                        egui::RichText::new("(read-only)")
                            .size(10.0)
                            .color(egui::Color32::from_rgb(156, 165, 180)),
                    );
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                match &self.selected_file_content {
                    Some(Ok(content)) => {
                        let mut content_copy = content.clone();
                        let mut layouter = |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
                            let mut layout_job = highlight_code(buf.as_str());
                            layout_job.wrap.max_width = wrap_width;
                            ui.fonts_mut(|f| f.layout_job(layout_job))
                        };
                        egui::ScrollArea::both().show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut content_copy)
                                    .code_editor()
                                    .interactive(false)
                                    .desired_width(f32::INFINITY)
                                    .layouter(&mut layouter),
                            );
                        });
                    }
                    Some(Err(err)) => {
                        ui.colored_label(egui::Color32::from_rgb(248, 113, 113), format!("Couldn't read file: {err}"));
                    }
                    None => {}
                }
                return;
            }

            let Some(repo) = self.selected_repo() else {
                ui.add_space((ui.available_height() / 3.0).max(0.0));
                ui.vertical_centered(|ui| {
                    if self.repositories.is_empty() {
                        ui.label(egui::RichText::new("🐾").size(40.0));
                        ui.label(egui::RichText::new("No repositories yet").size(18.0).strong());
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Clone a repository above to get started.")
                                .color(egui::Color32::from_rgb(156, 165, 180))
                                .size(12.0),
                        );
                    } else {
                        ui.label(egui::RichText::new("Select a repository from the list.").color(egui::Color32::from_rgb(156, 165, 180)).size(12.0));
                    }
                });
                return;
            };

            let repo_name = repo.name.clone();
            let repo_remote = repo.remote_url.clone();
            let repo_state = format!("{:?}", repo.sync_state);
            let repo_path = repo.local_path.clone();

            // Repository header
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&repo_name).size(18.0).strong().color(egui::Color32::from_rgb(251, 146, 60)));
                ui.label(egui::RichText::new(&repo_state).size(10.0).color(egui::Color32::from_rgb(156, 165, 180)));
            });
            ui.label(egui::RichText::new(&repo_remote).size(11.0).color(egui::Color32::from_rgb(156, 165, 180)));
            ui.add_space(12.0);

            // Action buttons
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if ui.button("Open in VS Code").clicked() {
                    match open_in_editor(std::path::Path::new(&repo_path), "code") {
                        Ok(()) => self.set_status(format!("Opening {repo_name} in VS Code..."), false),
                        Err(err) => self.set_status(
                            format!("Couldn't launch VS Code — is it on PATH? ({err})"),
                            true,
                        ),
                    }
                }
                if ui.button("Compare with GitHub").clicked() {
                    let token = self.current_token();
                    match compare_repo_with_origin(std::path::Path::new(&repo_path), token.as_deref()) {
                        Ok(comparison) => {
                            self.comparison = Some(comparison);
                            self.set_status("Comparison updated.", false);
                        }
                        Err(err) => {
                            self.set_status(format!("Comparison failed: {err}"), true);
                        }
                    }
                }
                if ui.button("Fetch").clicked() {
                    self.handle_fetch(&repo_path);
                }
                if ui.button("Pull").clicked() {
                    self.handle_pull(&repo_path);
                }
            });

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(10.0);

            // View mode tabs
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let files_selected = self.view_mode == ViewMode::Files;
                let commits_selected = self.view_mode == ViewMode::Commits;
                
                let files_frame = egui::Frame::default()
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .fill(if files_selected { egui::Color32::from_rgb(35, 41, 51) } else { egui::Color32::TRANSPARENT });
                
                files_frame.show(ui, |ui| {
                    if ui.selectable_label(files_selected, "Files").clicked() {
                        self.view_mode = ViewMode::Files;
                    }
                });

                let commits_frame = egui::Frame::default()
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .fill(if commits_selected { egui::Color32::from_rgb(35, 41, 51) } else { egui::Color32::TRANSPARENT });
                
                commits_frame.show(ui, |ui| {
                    if ui.selectable_label(commits_selected, "Commits").clicked() {
                        self.view_mode = ViewMode::Commits;
                    }
                });
            });
            
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            match self.view_mode {
                ViewMode::Files => {
                    if let Some(tree) = self.file_tree.clone() {
                        egui::ScrollArea::vertical().id_salt("files_view").max_height(400.0).show(ui, |ui| {
                            render_tree(ui, &tree, self);
                        });
                    } else {
                        ui.label("Select a repository to browse its files.");
                    }
                }
                ViewMode::Commits => {
                    egui::ScrollArea::vertical().id_salt("commits_view").max_height(400.0).show(ui, |ui| {
                        for commit in &self.commit_log {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&commit.sha[..7.min(commit.sha.len())])
                                        .monospace()
                                        .color(egui::Color32::from_rgb(251, 146, 60)),
                                );
                                ui.label(&commit.message);
                            });
                            ui.label(
                                egui::RichText::new(format!("{} · {}", commit.author, commit.date))
                                    .color(egui::Color32::from_rgb(107, 114, 128)),
                            );
                            ui.add_space(4.0);
                            ui.separator();
                        }
                        if self.commit_log.is_empty() {
                            ui.label("No commits yet.");
                        }
                    });
                }
            }

            // Comparison section
            if let Some(comparison) = &self.comparison {
                ui.add_space(12.0);
                ui.separator();
                ui.label(egui::RichText::new("Comparison with GitHub").strong());
                ui.add_space(6.0);
                
                egui::ScrollArea::vertical().id_salt("comparison_view").max_height(240.0).show(ui, |ui| {
                    for file in comparison {
                        let (icon, color) = match file.status {
                            FileCompareStatus::Same => ("✓", egui::Color32::from_rgb(134, 194, 172)),
                            FileCompareStatus::Modified => ("⚠", egui::Color32::from_rgb(251, 191, 36)),
                            FileCompareStatus::LocalOnly => ("🟢", egui::Color32::from_rgb(134, 194, 172)),
                            FileCompareStatus::RemoteOnly => ("🔵", egui::Color32::from_rgb(96, 165, 250)),
                        };
                        ui.horizontal(|ui| {
                            ui.colored_label(color, icon);
                            ui.label(egui::RichText::new(&file.path));
                        });
                    }
                });
            }
        });
    }
}

fn highlight_code(text: &str) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    use egui::{Color32, FontId};
    use std::collections::HashSet;

    const KEYWORDS: &[&str] = &[
        "fn", "let", "mut", "pub", "struct", "enum", "impl", "use", "mod", "return", "if", "else",
        "match", "for", "while", "loop", "break", "continue", "const", "static", "trait", "async",
        "await", "move", "ref", "self", "Self", "true", "false", "null", "None", "Some", "Ok", "Err",
        "function", "var", "const", "import", "export", "class", "extends", "new", "this", "def",
        "from", "as", "in", "is", "not", "and", "or", "type", "interface", "void", "int", "string",
    ];

    let base = TextFormat::simple(FontId::monospace(12.5), Color32::from_rgb(225, 225, 225));
    let comment_fmt = TextFormat::simple(FontId::monospace(12.5), Color32::from_rgb(107, 114, 128));
    let string_fmt = TextFormat::simple(FontId::monospace(12.5), Color32::from_rgb(134, 224, 145));
    let keyword_fmt = TextFormat::simple(FontId::monospace(12.5), Color32::from_rgb(251, 146, 60));
    let number_fmt = TextFormat::simple(FontId::monospace(12.5), Color32::from_rgb(129, 178, 255));

    // Build keyword lookup set once — O(k), amortised over all lines.
    let keyword_set: HashSet<&str> = KEYWORDS.iter().copied().collect();

    let mut job = LayoutJob::default();

    for line in text.split_inclusive('\n') {
        // ── Comment detection (whole-line, runs first) ────────────────
        let hash_comment = if line.trim_start().starts_with('#') {
            line.find('#')
        } else {
            None
        };
        if let Some(comment_start) = line.find("//").or(hash_comment) {
            job.append(&line[..comment_start], 0.0, base.clone());
            job.append(&line[comment_start..], 0.0, comment_fmt.clone());
            continue;
        }

        // ── Token walk via char_indices (O(n) per line) ───────────────
        let len = line.len();
        let mut i = 0usize;

        while i < len {
            let c = line[i..].chars().next().unwrap();
            let c_len = c.len_utf8();

            if c == '"' || c == '\'' {
                // String / character literal — scan to matching closing quote.
                let quote = c;
                let after = &line[i + c_len..];
                let close = after.find(quote)
                    .map(|j| i + c_len + j + quote.len_utf8())
                    .unwrap_or(len);
                job.append(&line[i..close], 0.0, string_fmt.clone());
                i = close;
                continue;
            }

            if c.is_ascii_alphanumeric() || c == '_' {
                // Identifier or number token — collect the full run.
                let start = i;
                i += c_len;
                while i < len {
                    let nc = line[i..].chars().next().unwrap();
                    if nc.is_ascii_alphanumeric() || nc == '_' || (nc == '.' && line.as_bytes()[start].is_ascii_digit()) {
                        i += nc.len_utf8();
                    } else {
                        break;
                    }
                }
                let token = &line[start..i];

                // Number token: starts with an ASCII digit
                if token.as_bytes()[0].is_ascii_digit() {
                    job.append(token, 0.0, number_fmt.clone());
                } else if keyword_set.contains(token) {
                    job.append(token, 0.0, keyword_fmt.clone());
                } else {
                    job.append(token, 0.0, base.clone());
                }
                continue;
            }

            // Any other character (punctuation, space, etc.)
            job.append(&line[i..i + c_len], 0.0, base.clone());
            i += c_len;
        }
    }

    job
}

fn render_tree(ui: &mut egui::Ui, nodes: &[TreeNode], app: &mut CatApp) {
    for node in nodes {
        match node {
            TreeNode::Directory { name, children, .. } => {
                egui::CollapsingHeader::new(
                    egui::RichText::new(format!("📁 {name}"))
                        .size(11.0)
                )
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.add_space(2.0);
                        render_tree(ui, children, app);
                        ui.add_space(2.0);
                    });
            }
            TreeNode::File { name, path } => {
                let response = ui.selectable_label(false, 
                    egui::RichText::new(format!("📄 {name}"))
                        .size(10.5)
                        .color(egui::Color32::from_rgb(210, 210, 210))
                );
                if response.clicked() {
                    app.open_file(path.clone());
                }
                if response.hovered() {
                    response.on_hover_text("Click to view");
                }
            }
        }
    }
}

#[cfg(windows)]
pub fn run() -> Result<(), String> {
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Cat",
        native_options,
        Box::new(|cc| Ok(Box::new(CatApp::new(cc)))),
    )
    .map_err(|err| err.to_string())
}

#[cfg(not(windows))]
pub fn run() -> Result<(), String> {
    Err("Cat's native GUI is currently implemented for Windows".into())
}

#[cfg(test)]
mod tests {
    use super::{app_name, drive_root};
    use eframe::egui;
    #[test]
    fn identity_is_cat() {
        assert_eq!(app_name(), "Cat");
    }
    #[test]
    fn has_drive_root() {
        assert!(!drive_root().as_os_str().is_empty());
    }

    // -------------------------------------------------------------------------
    // Task 4 — Bug A exploration test
    // -------------------------------------------------------------------------

    /// Task 4 — Bug A exploration: highlight_code must highlight ALL keyword tokens
    ///
    /// Input "mut fn" contains two keywords. The unfixed O(n²) loop finds "fn" at
    /// position 4, emits "mut " as plain base text (missing the "mut" keyword), then
    /// highlights "fn". So "mut" is never keyword-coloured.
    ///
    /// EXPECTED OUTCOME on unfixed code: FAILS
    ///   counterexample: "mut" span has base color, not keyword color
    ///
    /// Validates: Requirements 2.1, 2.2, 2.3
    #[test]
    fn highlight_code_keywords_in_prefix_are_highlighted() {
        let job = super::highlight_code("mut fn");
        let keyword_color = egui::Color32::from_rgb(251, 146, 60);

        // Collect all text spans that have keyword color
        let keyword_spans: Vec<&str> = job.sections.iter()
            .filter(|s| s.format.color == keyword_color)
            .map(|s| &job.text[s.byte_range.start.0..s.byte_range.end.0])
            .collect();

        assert!(
            keyword_spans.contains(&"mut"),
            "Bug A: 'mut' must be highlighted as a keyword on line 'mut fn'.\n\
             Counterexample: keyword-coloured spans are {:?} — 'mut' is missing",
            keyword_spans
        );
        assert!(
            keyword_spans.contains(&"fn"),
            "'fn' must also be highlighted as a keyword on line 'mut fn'.\n\
             keyword-coloured spans are {:?}",
            keyword_spans
        );
    }

    // -------------------------------------------------------------------------
    // Task 5 — Bug A preservation tests
    // -------------------------------------------------------------------------

    /// Task 5 — Preservation: comment lines still render entirely in comment color
    #[test]
    fn highlight_code_preserves_comment_lines() {
        let job = super::highlight_code("// this is a comment");
        let comment_color = egui::Color32::from_rgb(107, 114, 128);
        // Every non-empty span must be comment-colored
        let non_comment: Vec<&str> = job.sections.iter()
            .filter(|s| {
                let text = &job.text[s.byte_range.start.0..s.byte_range.end.0];
                s.format.color != comment_color && !text.trim().is_empty()
            })
            .map(|s| &job.text[s.byte_range.start.0..s.byte_range.end.0])
            .collect();
        assert!(
            non_comment.is_empty(),
            "All text in a comment line must be comment-colored. Non-comment spans: {:?}",
            non_comment
        );
    }

    /// Task 5 — Preservation: string literals still render in string color
    #[test]
    fn highlight_code_preserves_string_literals() {
        let job = super::highlight_code("\"hello\"");
        let string_color = egui::Color32::from_rgb(134, 224, 145);
        let string_spans: Vec<&str> = job.sections.iter()
            .filter(|s| s.format.color == string_color)
            .map(|s| &job.text[s.byte_range.start.0..s.byte_range.end.0])
            .collect();
        assert!(
            string_spans.iter().any(|s| s.contains("hello")),
            "String content must be string-colored. String spans: {:?}",
            string_spans
        );
    }

    /// Task 5 — Preservation: number literals still render in number color
    #[test]
    fn highlight_code_preserves_number_literals() {
        let job = super::highlight_code("123");
        let number_color = egui::Color32::from_rgb(129, 178, 255);
        let number_spans: Vec<&str> = job.sections.iter()
            .filter(|s| s.format.color == number_color)
            .map(|s| &job.text[s.byte_range.start.0..s.byte_range.end.0])
            .collect();
        assert!(
            number_spans.iter().any(|s| s.contains("123")),
            "Number literal must be number-colored. Number spans: {:?}",
            number_spans
        );
    }

    // -------------------------------------------------------------------------
    // Task 9 — Bug 8 exploration test
    // -------------------------------------------------------------------------

    /// Task 9 — Bug 8 exploration: interior `#` must NOT trigger comment colouring
    ///
    /// The unfixed code uses `line.find('#')` which fires on any `#` position.
    /// For "let x = map[#key];", the '#' at offset 12 causes everything from
    /// '#key];' to be rendered in comment colour.
    ///
    /// EXPECTED OUTCOME on unfixed code: FAILS
    ///   counterexample: span containing "#key" has comment_color, not base color
    ///
    /// Validates: Requirements 1.5, 1.6
    #[test]
    fn highlight_code_interior_hash_not_comment() {
        let job = super::highlight_code("let x = map[#key];");
        let comment_color = egui::Color32::from_rgb(107, 114, 128);

        // Collect all spans that are comment-colored
        let comment_spans: Vec<&str> = job.sections.iter()
            .filter(|s| s.format.color == comment_color)
            .map(|s| &job.text[s.byte_range.start.0..s.byte_range.end.0])
            .collect();

        assert!(
            comment_spans.is_empty(),
            "Bug 8: Interior '#' must NOT produce comment-coloured spans.\n\
             Counterexample: comment-coloured spans are {:?}",
            comment_spans
        );
    }

}