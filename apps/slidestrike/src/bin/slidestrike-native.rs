use std::path::PathBuf;
use std::process::ExitCode;

use eframe::egui;
use slidestrike::native_ui::NativeEditor;

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("Usage: slidestrike-native FILE.sstrike");
        return ExitCode::FAILURE;
    };
    let editor = match NativeEditor::open_or_create(path) {
        Ok(editor) => editor,
        Err(error) => {
            eprintln!("SlideStrike: {error}");
            return ExitCode::FAILURE;
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SlideStrike")
            .with_inner_size([1450.0, 850.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    match eframe::run_native(
        "SlideStrike",
        options,
        Box::new(move |_cc| Ok(Box::new(editor))),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("SlideStrike window: {error}");
            ExitCode::FAILURE
        }
    }
}
