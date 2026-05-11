use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MarkRush")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([840.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "MarkRush",
        options,
        Box::new(|cc| Ok(Box::new(MarkRushApp::new(cc)))),
    )
}

struct MarkRushApp {
    message: String,
}

impl MarkRushApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            message: "Rust rebuild scaffold is ready.".to_owned(),
        }
    }
}

impl eframe::App for MarkRushApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("MarkRush");
            ui.label(&self.message);
        });
    }
}
