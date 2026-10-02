//! Scrollable terminal buffer with per-line color levels.

/// Color level assigned to each terminal line for readability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Normal stdout output.
    Normal,
    /// Warnings (`warning:`, dirty-tree notices).
    Warn,
    /// Errors / failures (`error:`, `fatal:`).
    Error,
    /// Success markers (`[ok]`, `--- done (0)`).
    Ok,
    /// Cancelled / informational dim lines.
    Muted,
}

impl Level {
    fn color(self, ui: &egui::Ui) -> egui::Color32 {
        match self {
            Level::Normal => ui.visuals().text_color(),
            Level::Warn => egui::Color32::from_rgb(0xE0, 0xA0, 0x30),
            Level::Error => egui::Color32::from_rgb(0xD0, 0x50, 0x50),
            Level::Ok => egui::Color32::from_rgb(0x50, 0xC0, 0x70),
            Level::Muted => ui.visuals().weak_text_color(),
        }
    }
}

/// Classify a raw captured line (ANSI stripped) into a visual level.
pub fn classify(line: &str) -> Level {
    let l = line.to_ascii_lowercase();
    if l.starts_with("[err]")
        || l.starts_with("error:")
        || l.contains("fatal:")
        || l.contains("failed to")
    {
        Level::Error
    } else if l.contains("warning:")
        || l.starts_with("[warn]")
        || l.contains(" is dirty")
        || l.contains("may need sync")
    {
        Level::Warn
    } else if l.starts_with("[ok]")
        || l.starts_with("[done]")
        || l.contains("--- done (0)")
        || l.starts_with("+++ done")
        || l.contains("done. the new configuration")
    {
        Level::Ok
    } else if l.starts_with("[cancelled]") || l.starts_with("---") || l.is_empty() {
        Level::Muted
    } else {
        Level::Normal
    }
}

/// A single captured line with its classified color level.
#[derive(Debug, Clone)]
pub struct Line {
    pub level: Level,
    pub text: String,
}

/// An append-only, scrollable in-memory log buffer for the terminal area,
/// carrying a per-line color level.
pub struct Terminal {
    pub lines: Vec<Line>,
    pub stick_to_bottom: bool,
}

impl Default for Terminal {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            stick_to_bottom: true,
        }
    }
}

impl Terminal {
    pub fn push_line(&mut self, line: String) {
        let stripped = crate::text::strip_ansi(line.strip_suffix('\n').unwrap_or(&line));
        let level = classify(&stripped);
        self.lines.push(Line {
            level,
            text: stripped,
        });
    }

    pub fn push_str(&mut self, text: &str) {
        for l in text.split('\n') {
            self.push_line(l.to_string());
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Render the buffer in a bottom-sticking, monospace scroll area with
    /// per-line colors. `verbose` toggles extra activation logs on apply.
    pub fn show(&mut self, ui: &mut egui::Ui, verbose: &mut bool) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.checkbox(&mut self.stick_to_bottom, "follow logs");
            ui.separator();
            let label = if *verbose {
                "verbose logs: on"
            } else {
                "verbose logs: off"
            };
            if ui
                .button(label)
                .on_hover_text(
                    "Show extra activation logs on switch/boot/test. \
                     Off by default for a quieter, simpler view.",
                )
                .clicked()
            {
                *verbose = !*verbose;
            }
        });
        ui.separator();
        egui::Frame::none()
            .fill(ui.visuals().extreme_bg_color)
            .inner_margin(egui::Margin::symmetric(8.0, 8.0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(self.stick_to_bottom)
                    .show(ui, |ui| {
                        for line in &self.lines {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&line.text)
                                        .monospace()
                                        .size(16.0)
                                        .color(line.level.color(ui)),
                                )
                                .wrap_mode(egui::TextWrapMode::Wrap),
                            );
                        }
                    });
            });
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_errors_red() {
        assert_eq!(classify("error: build failed"), Level::Error);
        assert_eq!(classify("fatal: not a git repository"), Level::Error);
        assert_eq!(classify("failed to fetch input"), Level::Error);
    }

    #[test]
    fn classifies_warnings_amber() {
        assert_eq!(classify("warning: Git tree '/x' is dirty"), Level::Warn);
        assert_eq!(classify("profile may need sync"), Level::Warn);
    }

    #[test]
    fn classifies_success_and_muted() {
        assert_eq!(classify("[ok] applying..."), Level::Ok);
        assert_eq!(classify("--- done (0) ---"), Level::Ok); // success marker -> green
        assert_eq!(classify("--- exited with code 1 ---"), Level::Muted);
        assert_eq!(classify("[cancelled] nothing applied"), Level::Muted);
    }

    #[test]
    fn informational_stderr_is_normal() {
        assert_eq!(classify("> Building NixOS configuration"), Level::Normal);
        assert_eq!(classify("Activating configuration"), Level::Normal);
        assert_eq!(
            classify("not applying UID change of user 'gdm-greeter-2'"),
            Level::Normal
        );
    }

    #[test]
    fn strips_ansi() {
        assert_eq!(crate::text::strip_ansi("\u{1b}[33mpeek\u{1b}[0m"), "peek");
    }
}
