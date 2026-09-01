use std::path::PathBuf;

use cat_core::{RepositoryRecord, clone_repository, discover_repositories, open_in_editor};
use eframe::egui;

pub fn app_name() -> &'static str {
    "Cat"
}

pub fn drive_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Default)]
struct CatApp {
    drive_root: PathBuf,
    repositories: Vec<RepositoryRecord>,
    selected_index: usize,
    clone_url_input: String,
    status_message: String,
    status_is_error: bool,
}

impl CatApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(17, 20, 26);
        visuals.window_fill = egui::Color32::from_rgb(12, 14, 20);
        visuals.extreme_bg_color = egui::Color32::from_rgb(8, 10, 15);
        visuals.selection.stroke.color = egui::Color32::from_rgb(251, 146, 60);
        visuals.selection.bg_fill = egui::Color32::from_rgb(251, 146, 60);
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(23, 27, 34);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(31, 37, 45);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(45, 52, 62);
        cc.egui_ctx.set_visuals(visuals);

        let mut app = Self {
            drive_root: drive_root(),
            selected_index: 0,
            ..Self::default()
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        let root = self.drive_root.join("repositories");
        self.repositories = discover_repositories(&root).unwrap_or_default();
        if self.selected_index >= self.repositories.len() {
            self.selected_index = self.repositories.len().saturating_sub(1);
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

        match clone_repository(&url, &target_dir) {
            Ok(()) => {
                self.set_status(format!("Cloned {repo_name}."), false);
                self.clone_url_input.clear();
                self.refresh();
            }
            Err(err) => {
                self.set_status(format!("Clone failed: {err}"), true);
            }
        }
    }
}

impl eframe::App for CatApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("cat_header")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("🐈").size(28.0));
                    ui.vertical(|ui| {
                        ui.heading("Cat");
                        ui.label(egui::RichText::new("portable repository vault").size(11.0).color(egui::Color32::from_rgb(148, 163, 184)));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("🔄️").clicked() {
                            self.refresh();
                        }
                    });
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.clone_url_input)
                            .hint_text("git remote URL")
                            .desired_width(400.0),
                    );
                    let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.button("Clone").clicked() || submitted {
                        self.handle_clone();
                    }
                });

                if !self.status_message.is_empty() {
                    let color = if self.status_is_error {
                        egui::Color32::from_rgb(251, 113, 60)
                    } else {
                        egui::Color32::from_rgb(148, 163, 184)
                    };
                    ui.label(egui::RichText::new(&self.status_message).size(11.0).color(color));
                }
            });

        egui::Panel::left("repos")
            .default_size(300.0)
            .show(ui, |ui| {
                ui.heading("Repositories");
                ui.separator();

                if self.repositories.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(32.0);
                        ui.label(egui::RichText::new("🐾 no repositories yet").size(18.0));
                        ui.label(egui::RichText::new("Drop repos on the drive to begin.").color(egui::Color32::from_rgb(148, 163, 184)));
                    });
                    return;
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (idx, repo) in self.repositories.iter().enumerate() {
                        let selected = idx == self.selected_index;
                        let repo_button = egui::Button::new(
                            egui::RichText::new(format!("{} {}", if selected { "▶" } else { "•" }, repo.name))
                                .strong()
                                .color(if selected {
                                    egui::Color32::from_rgb(255, 176, 103)
                                } else {
                                    egui::Color32::WHITE
                                }),
                        )
                        .fill(if selected {
                            egui::Color32::from_rgb(38, 44, 52)
                        } else {
                            egui::Color32::TRANSPARENT
                        });

                        if ui.add(repo_button).clicked() {
                            self.selected_index = idx;
                        }

                        ui.label(
                            egui::RichText::new(&repo.remote_url)
                                .size(10.0)
                                .color(egui::Color32::from_rgb(148, 163, 184)),
                        );
                        ui.separator();
                    }
                });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Selected repository");
            ui.separator();
            ui.add_space(8.0);

            match self.selected_repo() {
                Some(repo) => {
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.colored_label(egui::Color32::from_rgb(255, 176, 103), &repo.name);
                    });
                    ui.label(format!("Remote: {}", repo.remote_url));
                    ui.label(format!("State: {:?}", repo.sync_state));
                    ui.label(format!("Path: {}", repo.local_path));
                    ui.add_space(12.0);

                    let repo_path = repo.local_path.clone();
                    if ui.button("Open in VS Code").clicked() {
                        match open_in_editor(std::path::Path::new(&repo_path), "code") {
                            Ok(()) => self.set_status(format!("Opening {} in VS Code...", repo.name), false),
                            Err(err) => self.set_status(
                                format!("Couldn't launch VS Code — is it on PATH? ({err})"),
                                true,
                            ),
                        }
                    }

                    ui.add_space(12.0);
                    ui.label("The Cat drive is ready to inspect repositories from this portable workspace.");
                }
                None => {
                    ui.label("No repository selected");
                }
            }
        });
    }
}

#[cfg(windows)]
pub fn run() -> Result<(), String> {
    let native_options = eframe::NativeOptions {
        // wgpu's DX12/Vulkan backends can hard-crash (rather than fail
        // gracefully) on some GPU/driver combinations. glow (OpenGL) is
        // the more broadly compatible renderer — worth defaulting to it
        // unless a specific reason to prefer wgpu comes up later.
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
    #[test]
    fn identity_is_cat() {
        assert_eq!(app_name(), "Cat");
    }
    #[test]
    fn has_drive_root() {
        assert!(!drive_root().as_os_str().is_empty());
    }
}
