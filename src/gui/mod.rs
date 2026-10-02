pub mod actions;
pub mod dialogs;
pub mod terminal;

use crate::config::AppConfig;
use crate::generations::Generation;
use crate::runner::{self, Event, Sink};
use crossbeam_channel::{Receiver, Sender};
use dialogs::{ConfirmPrompt, FlakePrompt};
use std::sync::Arc;
use tokio::sync::Mutex;
use terminal::Terminal;

use actions::Action;

pub struct App {
    pub cfg: AppConfig,
    pub terminal: Terminal,
    pub rx: Receiver<Event>,
    pub tx: Sender<Event>,
    pub log_file: Arc<Mutex<std::fs::File>>,
    pub runtime: tokio::runtime::Runtime,
    pub settings_open: bool,
    pub help_open: bool,
    /// True on the frame a modal was just toggled open, so its opening click
    /// doesn't immediately count as an outside-click dismissal.
    pub settings_opened_this_frame: bool,
    pub help_opened_this_frame: bool,
    pub flake_opened_this_frame: bool,
    pub flake_prompt: FlakePrompt,
    pub confirm: ConfirmPrompt,
    pub pending: Option<(Action, Vec<String>)>,
    /// The elevated apply command stashed after a successful preview, awaiting
    /// the confirmation gate. `None` means no apply is queued.
    pub pending_apply: Option<(Action, String)>,
    /// True while a command (preview or apply) is running; blocks re-clicking.
    pub running: bool,
    /// Recent `nh os info` generations for the right panel.
    pub generations: Vec<Generation>,
    /// True once the initial generation fetch has completed.
    pub gens_loaded: bool,
    pub gens_loading: bool,
    pub logo_angle: f32,
    pub spinning: bool,
}

impl App {
    pub fn new(cfg: AppConfig, runtime: tokio::runtime::Runtime) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let log = cfg.log_file.clone();
        if let Some(dir) = log.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log);
        let log_file = Arc::new(Mutex::new(
            file.unwrap_or_else(|_| std::fs::File::create(&log).expect("open log")),
        ));

        let mut app = Self {
            cfg,
            terminal: Terminal::default(),
            rx,
            tx,
            log_file,
            runtime,
            settings_open: false,
            help_open: false,
            settings_opened_this_frame: false,
            help_opened_this_frame: false,
            flake_opened_this_frame: false,
            flake_prompt: FlakePrompt::default(),
            confirm: ConfirmPrompt::default(),
            pending: None,
            pending_apply: None,
            running: false,
            generations: Vec::new(),
            gens_loaded: false,
            gens_loading: false,
            logo_angle: 0.0,
            spinning: false,
        };
        app.terminal
            .push_str("tuneshon ready. click an action to begin.\n");
        app.refresh_generations();
        app
    }

    pub fn sink(&self) -> Sink {
        Sink {
            tx: self.tx.clone(),
            log_file: self.log_file.clone(),
        }
    }

    /// Drain any queued command events into the terminal buffer.
    fn drain_events(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                Event::Out(l) => self.terminal.push_line(l),
                Event::Err(l) => self.terminal.push_line(format!("[err] {l}")),
                Event::Done(code) => self.on_command_done(code),
                Event::Generations(gens) => {
                    self.generations = gens;
                    self.gens_loaded = true;
                    self.gens_loading = false;
                }
            }
        }
    }

    /// A command finished. If it was the preview and it succeeded, offer the
    /// elevated apply via the confirmation gate; otherwise just report and reset.
    fn on_command_done(&mut self, code: Option<i32>) {
        let line = match code {
            Some(0) => "--- done (0) ---".to_string(),
            Some(c) => format!("--- exited with code {c} ---"),
            None => "--- exited (signal) ---".to_string(),
        };
        self.terminal.push_line(line);
        self.running = false;
        self.spinning = false;

        let ok = code.unwrap_or(1) == 0;
        if ok {
            if let Some((action, apply_cmd)) = self.pending_apply.as_ref() {
                self.confirm.summary = format!(
                    "Build OK for \"{}\" — apply it now?\n  {}",
                    action.label(),
                    apply_cmd
                );
                self.confirm.open = true;
            } else {
                // An apply (or input update) finished fine; refresh generations.
                self.refresh_generations();
            }
        } else {
            // Preview (or apply) failed: nothing to confirm, back to idle.
            self.pending_apply = None;
        }
    }

    /// Spawn a command on the tokio runtime, capturing output for display.
    fn spawn(&mut self, cmd: &str) {
        let cwd = self.cfg.config_dir.clone();
        let cmd = cmd.to_string();
        let sink = self.sink();
        let rt = self.runtime.handle().clone();
        self.running = true;
        rt.spawn(async move {
            let _ = runner::run(&cmd, &cwd, sink).await;
        });
    }

    /// Quick, non-elevated `nix flake update <inputs>`; no build/confirm gate.
    fn run_input_update(&mut self, inputs: &[&str]) {
        let cmd = actions::build_input_update(inputs);
        self.spawn(&cmd);
    }

    fn handle_action(&mut self, action: Action) {
        if self.running {
            return; // one command at a time
        }
        if action.needs_flake_inputs() {
            self.flake_prompt.load(&self.cfg);
            self.flake_prompt.open = true;
            self.flake_opened_this_frame = true;
            self.pending = Some((action, Vec::new()));
        } else {
            self.start_preview(action, None);
        }
    }

    /// Run the non-elevated preview for `action`. If it needs an apply step,
    /// stash the elevated command for the confirmation gate on success.
    fn start_preview(&mut self, action: Action, inputs: Option<&str>) {
        let preview = actions::build_preview(action, &self.cfg, inputs);
        self.pending_apply = actions::build_apply(action, &self.cfg)
            .map(|apply| (action, apply));
        self.spawn(&preview);
    }

    /// Run the elevated apply command after the user confirmed.
    fn start_apply(&mut self) {
        if let Some((action, apply)) = self.pending_apply.take() {
            self.confirm.open = false;
            self.terminal.push_line(format!(
                "[ok] applying \"{}\"...",
                action.label()
            ));
            self.spawn(&apply);
        }
    }

    /// Kick off an async `nh os info` fetch; results land in `self.generations`
    /// via the shared event channel (no thread-unsafe mutation).
    fn refresh_generations(&mut self) {
        if self.gens_loading {
            return;
        }
        let cwd = self.cfg.config_dir.clone();
        let tx = self.tx.clone();
        self.gens_loading = true;
        let rt = self.runtime.handle().clone();
        rt.spawn(async move {
            let gens = crate::generations::fetch(&cwd);
            let _ = tx.send(Event::Generations(gens));
        });
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .button("⚙ settings")
                    .on_hover_text("Open settings (Nix config dir, log file, boot loader).")
                    .clicked()
                {
                    self.settings_open = true;
                    self.settings_opened_this_frame = true;
                }

                ui.centered_and_justified(|ui| {
                    let (rect, res) = ui.allocate_exact_size(
                        egui::vec2(36.0, 36.0),
                        egui::Sense::click(),
                    );
                    let res = res.on_hover_text("click me!!");
                    if res.clicked() {
                        self.spinning = !self.spinning;
                    }
                    if self.spinning {
                        self.logo_angle += 0.08;
                        ctx.request_repaint_after(std::time::Duration::from_millis(16));
                    } else {
                        self.logo_angle %= std::f32::consts::TAU;
                    }
                    let painter = ui.painter_at(rect);
                    let c = rect.center();
                    let r = rect.width() / 2.0 - 5.0;
                    painter.circle_filled(c, r, ui.visuals().strong_text_color());
                    painter.circle_filled(c, r * 0.5, ui.visuals().window_fill());
                    // Rotating accent dot, positioned by angle (a cheap spin).
                    let (s, co) = self.logo_angle.sin_cos();
                    let px = c.x + co * r * 0.7;
                    let py = c.y + s * r * 0.7;
                    painter.circle_filled(egui::pos2(px, py), 3.0, ui.visuals().warn_fg_color);
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button("? help")
                        .on_hover_text("Show the help overview.")
                        .clicked()
                    {
                        self.help_open = true;
                        self.help_opened_this_frame = true;
                    }
                });
            });
        });
    }

    fn left_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("actions")
            .resizable(false)
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                let can_click = !self.running && !self.confirm.open;

                ui.heading("Actions");
                ui.separator();
                for action in Action::ALL {
                    if ui
                        .add_enabled(
                            can_click,
                            egui::Button::new(action.label())
                                .min_size(egui::vec2(ui.available_width(), 40.0)),
                        )
                        .on_hover_text(action.description())
                        .clicked()
                    {
                        self.handle_action(action);
                    }
                }

                ui.add_space(12.0);
                ui.heading("Update inputs");
                ui.separator();
                let input_buttons: &[(&str, &[&str], &str)] = &[
                    (
                        "packages",
                        &["nixpkgs"],
                        "quick `nix flake update nixpkgs`.",
                    ),
                    (
                        "addons",
                        &["wall-jar", "icon-jar", "mcskins-jar", "shelljar", "tuneshon"],
                        "quick `nix flake update` for your addons \
                         (icon-jar, wall-jar, mcskins-jar, shelljar, tuneshon).",
                    ),
                    (
                        "kernel",
                        &["nix-cachyos-kernel"],
                        "quick `nix flake update nix-cachyos-kernel`.",
                    ),
                ];
                for (name, targets, hint) in input_buttons {
                    if ui
                        .add_enabled(
                            can_click,
                            egui::Button::new(format!("update {name}"))
                                .min_size(egui::vec2(ui.available_width(), 34.0)),
                        )
                        .on_hover_text(*hint)
                        .clicked()
                    {
                        self.run_input_update(targets);
                    }
                }

                ui.add_space(12.0);
                if ui
                    .button("clear terminal")
                    .on_hover_text("Clear the output/terminal pane.")
                    .clicked()
                {
                    self.terminal.clear();
                }
            });
    }

    fn generations_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("generations")
            .default_width(300.0)
            .min_width(220.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading("Generations");
                ui.separator();
                if self.gens_loading {
                    ui.spinner();
                    ui.weak("loading…");
                } else if self.generations.is_empty() {
                    if self.gens_loaded {
                        ui.weak("no generations found");
                    } else {
                        ui.weak("loading…");
                    }
                } else {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for g in &self.generations {
                                let rich = egui::RichText::new(format!(
                                    "{{{}  {}\n  {}  {}  {}",
                                    g.id, g.date, g.nixos_version, g.kernel, g.size
                                ))
                                .monospace()
                                .size(13.0);
                                let rich = if g.current {
                                    rich
                                        .strong()
                                        .color(egui::Color32::from_rgb(0x50, 0xC0, 0x70))
                                } else {
                                    rich
                                        .color(ui.visuals().weak_text_color())
                                };
                                ui.add(egui::Label::new(rich).wrap_mode(egui::TextWrapMode::Wrap));
                                ui.add_space(4.0);
                            }
                        });
                }
            });
    }

    fn central(&mut self, ctx: &egui::Context) {
        self.generations_panel(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Output");
            ui.separator();
            self.drain_events();
            self.terminal.show(ui);
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.settings_open {
            let just_opened = self.settings_opened_this_frame;
            self.settings_opened_this_frame = false;
            dialogs::settings_modal(ctx, &mut self.settings_open, &mut self.cfg, just_opened);
        }
        if self.help_open {
            let just_opened = self.help_opened_this_frame;
            self.help_opened_this_frame = false;
            dialogs::help_modal(ctx, &mut self.help_open, just_opened);
        }
        if self.flake_prompt.open {
            let just_opened = self.flake_opened_this_frame;
            self.flake_opened_this_frame = false;
            dialogs::flake_prompt(ctx, &mut self.flake_prompt, just_opened);
            if self.flake_prompt.submit_requested {
                self.flake_prompt.submit_requested = false;
                self.flake_prompt.open = false;
                let joined = self.flake_prompt.selected_names().join(" ");
                if let Some((action, _)) = self.pending.take() {
                    self.start_preview(action, Some(&joined));
                }
            }
        }
        if self.confirm.open {
            dialogs::confirm_modal(ctx, &mut self.confirm);
            if self.confirm.apply_requested {
                self.confirm.apply_requested = false;
                self.start_apply();
            } else if self.confirm.cancelled {
                self.confirm.cancelled = false;
                self.confirm.open = false;
                self.terminal.push_line("[cancelled] nothing applied".to_string());
                self.pending_apply = None;
                self.spinning = false;
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.top_bar(ctx);
        self.left_panel(ctx);
        self.central(ctx);
        self.dialogs(ctx);
    }
}

/// Load a set of system fonts (best-effort) and register them (plus a
/// Unicode/CJK-capable font) as fallbacks so box-drawing/arrow/unicode glyphs
/// render instead of showing missing-glyph boxes. egui bundles only a few
/// fonts that lack many of the characters nh emits.
fn install_system_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let mut add_face = |name: &str, data: Vec<u8>, families: &[egui::FontFamily]| {
        fonts
            .font_data
            .insert(name.to_string(), egui::FontData::from_owned(data).into());
        for fam in families {
            fonts.families.entry(fam.clone()).or_default().push(name.to_string());
        }
    };

    // Enumerate system fonts via font-kit. font-kit's source loading is
    // synchronous and blocking here — acceptable at startup.
    let load = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", "monospace"])
        .output();
    if let Ok(o) = load {
        let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !path.is_empty() {
            if let Ok(data) = std::fs::read(&path) {
                add_face("system_mono", data, &[egui::FontFamily::Monospace]);
            }
        }
    }

    // A Unicode-heavy fallback (Noto Sans / DejaVu) covers arrows + box chars.
    let unicode = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", "sans-serif"])
        .output();
    if let Ok(o) = unicode {
        let path = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !path.is_empty() {
            if let Ok(data) = std::fs::read(&path) {
                add_face(
                    "system_unicode",
                    data,
                    &[
                        egui::FontFamily::Monospace,
                        egui::FontFamily::Proportional,
                    ],
                );
            }
        }
    }

    ctx.set_fonts(fonts);
}

/// Entry point that boots the eframe window.
pub struct Gui;

impl Gui {
    pub fn launch() -> anyhow::Result<()> {
        let cfg = crate::config::AppConfig::load()?;
        crate::config::ensure_log_dir(&cfg)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;

        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("tuneshon")
                .with_inner_size([960.0, 640.0])
                .with_min_inner_size([640.0, 400.0]),
            ..Default::default()
        };

        eframe::run_native(
            "tuneshon",
            native_options,
            Box::new(move |cc| {
                install_system_fonts(&cc.egui_ctx);
                Ok(Box::new(App::new(cfg, runtime)))
            }),
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }
}