use crate::config::AppConfig;
use crate::flakelock;
use std::path::PathBuf;

/// A flake input entry shown in the picker dialog.
#[derive(Debug, Clone, Default)]
pub struct FlakeInput {
    pub name: String,
    pub selected: bool,
}

/// State for the "flake lock update" chooser modal.
#[derive(Default)]
pub struct FlakePrompt {
    pub open: bool,
    pub filter: String,
    pub inputs: Vec<FlakeInput>,
    pub submit_requested: bool,
}

impl FlakePrompt {
    pub fn load(&mut self, cfg: &AppConfig) {
        self.inputs.clear();
        if let Ok(map) = flakelock::parse_inputs(&cfg.flake_lock_path()) {
            self.inputs = map
                .into_keys()
                .map(|name| FlakeInput {
                    name,
                    selected: false,
                })
                .collect();
            self.inputs.sort_by(|a, b| a.name.cmp(&b.name));
        }
    }

    pub fn selected_names(&self) -> Vec<String> {
        self.inputs
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.name.clone())
            .collect()
    }
}

/// Modal for editing persistent settings.
pub fn settings_modal(
    ctx: &egui::Context,
    open: &mut bool,
    cfg: &mut AppConfig,
) {
    let mut edit = cfg.clone();
    let mut config_dir_buf = edit.config_dir.to_string_lossy().to_string();
    let mut log_file_buf = edit.log_file.to_string_lossy().to_string();
    let mut boot_changed = false;
    let options = ["systemd-boot", "grub", "efi"];
    let mut sel = options
        .iter()
        .position(|o| *o == edit.boot_loader)
        .unwrap_or(0);
    let mut saved = false;
    let mut reset = false;
    let mut save_cfg: Option<AppConfig> = None;

    egui::Window::new("Settings")
        .open(open)
        .resizable(true)
        .show(ctx, |ui| {
            ui.set_min_width(360.0);
            ui.label("Nix config dir");
            if ui.text_edit_singleline(&mut config_dir_buf).changed() {
                edit.config_dir = PathBuf::from(config_dir_buf.trim());
            }
            ui.label("Log file");
            if ui.text_edit_singleline(&mut log_file_buf).changed() {
                edit.log_file = PathBuf::from(log_file_buf.trim());
            }
            ui.label("Boot loader");
            egui::ComboBox::from_id_salt("boot_loader")
                .selected_text(options[sel])
                .show_ui(ui, |ui| {
                    for (idx, opt) in options.iter().enumerate() {
                        if ui.selectable_value(&mut sel, idx, *opt).changed() {
                            boot_changed = true;
                        }
                    }
                });
            if boot_changed {
                edit.boot_loader = options[sel].to_string();
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    saved = true;
                }
                if ui.button("Reset defaults").clicked() {
                    reset = true;
                }
            });
            if saved {
                save_cfg = Some(edit.clone());
            }
        });

    if reset {
        *cfg = AppConfig::default();
        *open = false;
    }
    if let Some(c) = save_cfg {
        *cfg = c;
        let _ = cfg.save();
        *open = false;
    }
    let _ = &mut edit;
    let _ = &mut boot_changed;
}

/// Modal for picking which flake inputs to update.
pub fn flake_prompt(ctx: &egui::Context, prompt: &mut FlakePrompt) {
    let mut open = prompt.open;
    let mut filter = prompt.filter.clone();
    let mut inputs: Vec<FlakeInput> = prompt.inputs.clone();
    let mut submit = false;

    egui::Window::new("Select flake inputs to update")
        .open(&mut open)
        .resizable(true)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            ui.horizontal(|ui| {
                ui.label("Filter:");
                ui.text_edit_singleline(&mut filter);
            });
            ui.separator();

            let q = filter.to_lowercase();
            let filtered_idx: Vec<usize> = (0..inputs.len())
                .filter(|&i| q.is_empty() || inputs[i].name.to_lowercase().contains(&q))
                .collect();
            let mut scroll = egui::ScrollArea::vertical()
                .id_salt("flake_input_scroll")
                .auto_shrink([false, true]);
            scroll = scroll.max_height(ctx.screen_rect().height() * 0.5);
            scroll.show(ui, |ui| {
                for item_idx in filtered_idx {
                    let name = inputs[item_idx].name.clone();
                    let mut sel = inputs[item_idx].selected;
                    ui.checkbox(&mut sel, &name);
                    inputs[item_idx].selected = sel;
                }
                if inputs.is_empty() {
                    ui.weak("No inputs parsed (could not read flake.lock?)");
                }
            });

            ui.separator();
            let has_sel = inputs.iter().any(|i| i.selected);
            if ui.add_enabled(has_sel, egui::Button::new("Update")).clicked() {
                submit = true;
            }
        });

    prompt.open = open;
    prompt.filter = filter;
    prompt.inputs = inputs;
    if submit {
        prompt.submit_requested = true;
    }
}

pub fn help_modal(ctx: &egui::Context, open: &mut bool) {
    let mut close = false;
    egui::Window::new("Help").open(open).collapsible(false).show(ctx, |ui| {
        ui.spacing_mut().item_spacing = egui::Vec2::new(8.0, 6.0);
        let body = [
            "tuneshon - lightweight NixOS update tool",
            "",
            "GUI actions (left grid):",
            "  pull + switch + boot",
            "      git pull, then nixos-rebuild switch and boot.",
            "  git add {repodir} + switch + boot",
            "      stage repo changes, then switch and boot.",
            "  git add {repodir} + switch",
            "      stage repo changes, then switch.",
            "  git add {repodir} + boot",
            "      stage repo changes, then boot.",
            "  flake lock update [ + switch + boot ]",
            "      pick flake inputs to update in the dialog.",
            "",
            "The right pane streams command output (stdout/stderr).",
            "Output is also appended to the configured log file.",
            "",
            "Settings (gear icon) lets you set the Nix config dir",
            "and log path. Clicking the logo spins it.",
        ];
        for line in body {
            ui.label(egui::RichText::new(line).monospace().size(12.0));
        }
        ui.add_space(8.0);
        if ui.button("Close").clicked() {
            close = true;
        }
    });
    if close {
        *open = false;
    }
}