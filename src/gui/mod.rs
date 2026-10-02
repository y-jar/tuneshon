pub mod actions;
pub mod dialogs;
pub mod terminal;

use crate::config::AppConfig;
use crate::runner::{self, Event, Sink};
use crossbeam_channel::{Receiver, Sender};
use dialogs::FlakePrompt;
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
    pub flake_prompt: FlakePrompt,
    pub pending: Option<(Action, Vec<String>)>,
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
            flake_prompt: FlakePrompt::default(),
            pending: None,
            logo_angle: 0.0,
            spinning: false,
        };
        app.terminal
            .push_str("tuneshon ready. click an action to begin.\n");
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
                Event::Done(code) => {
                    let line = match code {
                        Some(0) => "--- done (0) ---".to_string(),
                        Some(c) => format!("--- exited with code {c} ---"),
                        None => "--- exited (signal) ---".to_string(),
                    };
                    self.terminal.push_line(line);
                }
            }
        }
    }

    /// Spawn a command on the tokio runtime, capturing output for display.
    fn spawn(&self, cmd: &str) {
        let cwd = self.cfg.config_dir.clone();
        let cmd = cmd.to_string();
        let sink = self.sink();
        let rt = self.runtime.handle().clone();
        rt.spawn(async move {
            let _ = runner::run(&cmd, &cwd, sink).await;
        });
    }

    fn handle_action(&mut self, action: Action) {
        let has_flake = action.needs_flake_inputs();
        if has_flake {
            self.flake_prompt.load(&self.cfg);
            self.flake_prompt.open = true;
            self.pending = Some((action, Vec::new()));
        } else {
            let cmd = actions::build_command(action, &self.cfg, None);
            self.spawn(&cmd);
        }
    }

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("⚙ settings").clicked() {
                    self.settings_open = true;
                }

                ui.centered_and_justified(|ui| {
                    let (rect, res) = ui.allocate_exact_size(
                        egui::vec2(36.0, 36.0),
                        egui::Sense::click(),
                    );
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
                    if ui.button("? help").clicked() {
                        self.help_open = true;
                    }
                });
            });
        });
    }

    fn left_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("actions")
            .resizable(false)
            .default_width(230.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.heading("Actions");
                ui.separator();
                for action in Action::ALL {
                    if ui
                        .add_sized([ui.available_width(), 44.0], egui::Button::new(action.label()))
                        .clicked()
                    {
                        self.handle_action(action);
                    }
                }
                ui.add_space(8.0);
                if ui.button("clear terminal").clicked() {
                    self.terminal.clear();
                }
            });
    }

    fn central(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Output");
            ui.separator();
            self.drain_events();
            self.terminal.show(ui);
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.settings_open {
            dialogs::settings_modal(ctx, &mut self.settings_open, &mut self.cfg);
        }
        if self.help_open {
            dialogs::help_modal(ctx, &mut self.help_open);
        }
        if self.flake_prompt.open {
            dialogs::flake_prompt(ctx, &mut self.flake_prompt);
            if self.flake_prompt.submit_requested {
                self.flake_prompt.submit_requested = false;
                self.flake_prompt.open = false;
                let joined = self.flake_prompt.selected_names().join(" ");
                if let Some((action, _)) = self.pending.take() {
                    let cmd = actions::build_command(action, &self.cfg, Some(&joined));
                    self.spawn(&cmd);
                }
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
            Box::new(move |_cc| Ok(Box::new(App::new(cfg, runtime)))),
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(())
    }
}