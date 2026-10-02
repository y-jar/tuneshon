/// An append-only, scrollable in-memory log buffer for the terminal area.
pub struct Terminal {
    pub lines: Vec<String>,
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
        let line = line.strip_suffix('\n').unwrap_or(&line).to_string();
        self.lines.push(line);
    }

    pub fn push_str(&mut self, text: &str) {
        for l in text.split('\n') {
            self.push_line(l.to_string());
        }
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Render the buffer in a bottom-sticking, monospace scroll area.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text = self.text();
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.checkbox(&mut self.stick_to_bottom, "follow logs");
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
                        ui.add(
                            egui::Label::new(egui::RichText::new(&text).monospace().small())
                                .wrap_mode(egui::TextWrapMode::Wrap),
                        );
                    });
            });
    }
}