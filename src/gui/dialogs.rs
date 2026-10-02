//! Modal dialogs: settings, flake-input picker, apply confirmation, and help.

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
///
/// `ignore_dismiss` suppresses the outside-click/Escape close on the very frame
/// the modal was opened, so the click that opened it can't immediately close it.
pub fn settings_modal(
    ctx: &egui::Context,
    open: &mut bool,
    cfg: &mut AppConfig,
    ignore_dismiss: bool,
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

    let resp = egui::Window::new("Settings")
        .open(open)
        .resizable(true)
        .show(ctx, |ui| {
            ui.set_min_width(360.0);
            ui.label("Nix config dir");
            ui.horizontal(|ui| {
                if ui.text_edit_singleline(&mut config_dir_buf).changed() {
                    edit.config_dir = PathBuf::from(config_dir_buf.trim());
                }
                if ui.button("Browse...").on_hover_text("Pick the Nix config directory (repo root).").clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        config_dir_buf = p.to_string_lossy().into_owned();
                        edit.config_dir = p;
                    }
                }
            });
            ui.label("Log file");
            ui.horizontal(|ui| {
                if ui.text_edit_singleline(&mut log_file_buf).changed() {
                    edit.log_file = PathBuf::from(log_file_buf.trim());
                }
                if ui.button("Browse...").on_hover_text("Pick where to write the log file.").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .set_file_name("tuneshon.log")
                        .save_file()
                    {
                        log_file_buf = p.to_string_lossy().into_owned();
                        edit.log_file = p;
                    }
                }
            });
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
                if ui.button("Save").on_hover_text("Save settings and close.").clicked() {
                    saved = true;
                }
                if ui.button("Reset defaults").on_hover_text("Restore default settings and close.").clicked() {
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
    if let Some(inner) = resp {
        if !ignore_dismiss && dismiss_requested(ctx, &inner.response.rect) {
            *open = false;
        }
    }
}

/// True when the modal should close: Escape was pressed or a click landed
/// outside the given window rect.
fn dismiss_requested(ctx: &egui::Context, win_rect: &egui::Rect) -> bool {
    let mut escape = false;
    let mut outside = false;
    ctx.input(|i| {
        escape = i.key_pressed(egui::Key::Escape);
        outside = i.pointer.any_click()
            && i
                .pointer
                .interact_pos()
                .is_some_and(|p| !win_rect.contains(p));
    });
    escape || outside
}

/// Modal for picking which flake inputs to update.
pub fn flake_prompt(
    ctx: &egui::Context,
    prompt: &mut FlakePrompt,
    ignore_dismiss: bool,
) {
    let mut open = prompt.open;
    let mut filter = prompt.filter.clone();
    let mut inputs: Vec<FlakeInput> = prompt.inputs.clone();
    let mut submit = false;

    let resp = egui::Window::new("Select flake inputs to update")
        .open(&mut open)
        .resizable(true)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            ui.horizontal(|ui| {
                ui.label("Filter:");
                ui.text_edit_singleline(&mut filter)
                    .on_hover_text("Type to narrow the flake input list by name.");
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
            let upd = ui.add_enabled(
                has_sel,
                egui::Button::new("Update"),
            ).on_hover_text(if has_sel {
                "Run `nix flake update` on the selected inputs."
            } else {
                "Select at least one input to update."
            });
            if upd.clicked() {
                submit = true;
            }
        });

    if let Some(inner) = resp {
        if !ignore_dismiss && dismiss_requested(ctx, &inner.response.rect) {
            open = false;
        }
    }

    prompt.open = open;
    prompt.filter = filter;
    prompt.inputs = inputs;
    if submit {
        prompt.submit_requested = true;
    }
}

/// Confirmation gate between the non-elevated preview and the elevated apply.
pub struct ConfirmPrompt {
    pub open: bool,
    pub summary: String,
    pub apply_requested: bool,
    /// True when the user dismissed/cancelled without applying.
    pub cancelled: bool,
}

impl Default for ConfirmPrompt {
    fn default() -> Self {
        Self {
            open: false,
            summary: String::new(),
            apply_requested: false,
            cancelled: false,
        }
    }
}

/// Modal asking whether to apply the just-built configuration.
pub fn confirm_modal(ctx: &egui::Context, prompt: &mut ConfirmPrompt) {
    let mut open = prompt.open;
    let summary = prompt.summary.clone();

    let resp = egui::Window::new("Apply configuration?")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_min_width(360.0);
            ui.label(egui::RichText::new(&summary).monospace().size(13.0));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if ui
                    .button("Apply")
                    .on_hover_text(
                        "Run the elevated switch/boot/test now (prompts for your password via polkit).",
                    )
                    .clicked()
                {
                    prompt.apply_requested = true;
                }
                if ui
                    .button("Cancel")
                    .on_hover_text("Abort. Do not apply; returns to idle.")
                    .clicked()
                {
                    prompt.cancelled = true;
                }
            });
        });

    if let Some(inner) = resp {
        if dismiss_requested(ctx, &inner.response.rect) {
            open = false;
            prompt.cancelled = true;
        }
    }
    if !open || prompt.apply_requested {
        prompt.open = false;
    }
}

pub fn help_modal(ctx: &egui::Context, open: &mut bool, ignore_dismiss: bool) {
    let mut close = false;
    let resp = egui::Window::new("Help").open(open).collapsible(false).show(ctx, |ui| {
        ui.spacing_mut().item_spacing = egui::Vec2::new(8.0, 6.0);
        let body = [
            "tuneshon - lightweight NixOS update tool",
            "",
            "Actions (left grid):",
            "  full update",
            "      git pull, then build + switch + boot.",
            "  update",
            "      git add ., then build + switch + boot.",
            "  test",
            "      git add ., build, then test-activate (not the boot default).",
            "  update after restart",
            "      git add ., build, then boot (applies on next reboot).",
            "  specify update? / specify update & update",
            "      pick flake inputs to update (optionally followed by switch).",
            "",
            "Update inputs (left grid): quick non-elevated `nix flake update`",
            "for packages / addons / kernel without a full build.",
            "",
            "Each action first runs a non-elevated `nh os build` so you can",
            "watch the terminal and confirm the config builds. Only after it",
            "succeeds are you prompted (Apply/Cancel) to run the elevated",
            "switch/boot/test through pkexec.",
            "",
            "The right panel lists system generations, newest first, with the",
            "current one highlighted in green. The center pane streams colored",
            "command output (stdout/stderr).",
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
    if let Some(inner) = resp {
        if !ignore_dismiss && dismiss_requested(ctx, &inner.response.rect) {
            *open = false;
        }
    }
}