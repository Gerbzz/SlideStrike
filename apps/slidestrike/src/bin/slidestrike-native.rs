use std::path::PathBuf;
use std::process::ExitCode;

use eframe::egui;
use slidestrike::native_ui::NativeEditor;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(path) = args.next().map(PathBuf::from) else {
        eprintln!("Usage: slidestrike-native FILE.sstrike [--capture FILE.png]");
        return ExitCode::FAILURE;
    };
    let capture_path = match (args.next(), args.next(), args.next()) {
        (None, None, None) => None,
        (Some(flag), Some(output), None) if flag == "--capture" => {
            let output = PathBuf::from(output);
            if output.extension().is_none_or(|ext| ext != "png") {
                eprintln!("Capture path must end in .png");
                return ExitCode::FAILURE;
            }
            Some(output)
        }
        _ => {
            eprintln!("Usage: slidestrike-native FILE.sstrike [--capture FILE.png]");
            return ExitCode::FAILURE;
        }
    };
    let mut editor = match NativeEditor::open_or_create(path) {
        Ok(editor) => editor,
        Err(error) => {
            eprintln!("SlideStrike: {error}");
            return ExitCode::FAILURE;
        }
    };
    let capture_result = capture_path.map(|path| editor.capture_once(path));
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
        Ok(()) => {
            if let Some(result) = capture_result {
                match result.lock().unwrap().take() {
                    Some(Ok(())) => ExitCode::SUCCESS,
                    Some(Err(error)) => {
                        eprintln!("SlideStrike capture: {error}");
                        ExitCode::FAILURE
                    }
                    None => {
                        eprintln!("SlideStrike capture: window closed before screenshot completed");
                        ExitCode::FAILURE
                    }
                }
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("SlideStrike window: {error}");
            ExitCode::FAILURE
        }
    }
}
