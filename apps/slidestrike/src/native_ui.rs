//! Rust desktop editor for the checked deck. Export buttons call the donor bridge.
//! This source awaits the next build and desktop walkthrough.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use uuid::Uuid;

use crate::store::{
    abandon_pre_move_deck, pending_pre_move_deck, recover_missing_deck, resume_pre_move_deck,
};
use crate::{
    Command, Deck, DeckSnapshot, DrawKind, Layout, ObjectKind, Rect, SlideObject, compose_slide,
    export_pdf_bytes, export_png_bytes, open_deck, save_deck,
};

pub struct NativeEditor {
    path: PathBuf,
    deck: Deck,
    saved_data: DeckSnapshot,
    saved_assets: BTreeMap<String, Vec<u8>>,
    reopen_requested: bool,
    pending_recovery: Option<PathBuf>,
    slide: Uuid,
    object: Option<Uuid>,
    image_path: String,
    textures: BTreeMap<String, egui::TextureHandle>,
    drag_applied: egui::Vec2,
    presenting: bool,
    present_index: usize,
    status: String,
    capture: Option<CaptureOnce>,
}

struct CaptureOnce {
    path: PathBuf,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
    started: Instant,
    settle_frames: u8,
    requested: bool,
}

impl NativeEditor {
    pub fn open_or_create(path: PathBuf) -> Result<Self, String> {
        if path.extension().is_none_or(|ext| ext != "sstrike") {
            return Err("native deck path must end in .sstrike".into());
        }
        let (deck, status, pending_recovery) = if path.exists() {
            let deck = open_deck(&path)?;
            let (pending, status) = Self::inspect_recovery(&path);
            (deck, status, pending)
        } else if let Some((deck, backup)) = recover_missing_deck(&path)? {
            (
                deck,
                format!(
                    "Recovered the interrupted save from {}. Save restores the missing deck; the backup remains available.",
                    backup.display()
                ),
                None,
            )
        } else {
            (Deck::new("Untitled Deck"), "Ready".to_string(), None)
        };
        let slide = deck.slides()[0].id;
        let saved_data = deck.snapshot();
        let saved_assets = deck.assets().clone();
        Ok(Self {
            path,
            deck,
            saved_data,
            saved_assets,
            reopen_requested: false,
            pending_recovery,
            slide,
            object: None,
            image_path: String::new(),
            textures: BTreeMap::new(),
            drag_applied: egui::Vec2::ZERO,
            presenting: false,
            present_index: 0,
            status,
            capture: None,
        })
    }

    /// Capture one fully painted editor frame, then close the capture window.
    pub fn capture_once(&mut self, path: PathBuf) -> Arc<Mutex<Option<Result<(), String>>>> {
        let outcome = Arc::new(Mutex::new(None));
        self.capture = Some(CaptureOnce {
            path,
            outcome: Arc::clone(&outcome),
            started: Instant::now(),
            settle_frames: 0,
            requested: false,
        });
        outcome
    }

    fn write_capture_png(
        path: &std::path::Path,
        screenshot: &egui::ColorImage,
    ) -> Result<(), String> {
        let [width, height] = screenshot.size;
        let bytes = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or("Screenshot dimensions overflow")?;
        let mut pixels = Vec::with_capacity(bytes);
        for color in &screenshot.pixels {
            pixels.extend_from_slice(&color.to_array());
        }
        if pixels.len() != bytes {
            return Err("Screenshot pixel count does not match its dimensions".into());
        }
        let width = u32::try_from(width).map_err(|_| "Screenshot width exceeds PNG limits")?;
        let height = u32::try_from(height).map_err(|_| "Screenshot height exceeds PNG limits")?;
        let image = image::RgbaImage::from_raw(width, height, pixels)
            .ok_or("Could not construct screenshot image")?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| format!("Create screenshot {}: {error}", path.display()))?;
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut file, image::ImageFormat::Png)
            .map_err(|error| format!("Encode screenshot: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("Sync screenshot: {error}"))?;
        Ok(())
    }

    fn finish_capture(&mut self, ctx: &egui::Context, result: Result<(), String>) {
        if let Some(capture) = self.capture.take() {
            *capture.outcome.lock().unwrap() = Some(result);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn capture_after_frame(&mut self, ctx: &egui::Context) {
        let Some(capture) = self.capture.as_ref() else {
            return;
        };
        if capture.started.elapsed() > Duration::from_secs(15) {
            self.finish_capture(ctx, Err("Timed out waiting for a native screenshot".into()));
            return;
        }
        if capture.requested {
            let image = ctx.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                    _ => None,
                })
            });
            if let Some(image) = image {
                let result = Self::write_capture_png(&capture.path, &image);
                self.finish_capture(ctx, result);
            } else {
                ctx.request_repaint();
            }
            return;
        }
        let capture = self.capture.as_mut().unwrap();
        if capture.settle_frames < 2 {
            capture.settle_frames += 1;
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            capture.requested = true;
        }
        ctx.request_repaint();
    }

    fn inspect_recovery(path: &PathBuf) -> (Option<PathBuf>, String) {
        match pending_pre_move_deck(path) {
            Ok(Some(staged)) => (
                Some(staged.clone()),
                format!(
                    "Interrupted save: original remains at {} and staged edits at {}. Choose Resume or Abandon.",
                    path.display(),
                    staged.display()
                ),
            ),
            Ok(None) => (None, "Ready".into()),
            Err(error) => (None, error),
        }
    }

    fn apply(&mut self, command: Command) {
        self.status = match self.deck.apply(command) {
            Ok(()) => {
                self.reconcile_selection();
                "Edited; save the deck to keep this change.".into()
            }
            Err(error) => error,
        };
    }

    fn reconcile_selection(&mut self) {
        if !self
            .deck
            .slides()
            .iter()
            .any(|slide| slide.id == self.slide)
        {
            self.slide = self.deck.slides()[0].id;
            self.object = None;
        }
        if let Some(object) = self.object {
            if !self.deck.slides().iter().any(|slide| {
                slide.id == self.slide && slide.objects.iter().any(|item| item.id == object)
            }) {
                self.object = None;
            }
        }
    }

    fn has_unsaved_edits(&self) -> bool {
        self.deck.snapshot() != self.saved_data || self.deck.assets() != &self.saved_assets
    }

    fn undo(&mut self) {
        self.status = self
            .deck
            .undo()
            .map(|_| "Undid edit".into())
            .unwrap_or_else(|e| e);
        self.reconcile_selection();
    }

    fn redo(&mut self) {
        self.status = self
            .deck
            .redo()
            .map(|_| "Redid edit".into())
            .unwrap_or_else(|e| e);
        self.reconcile_selection();
    }

    fn save(&mut self) {
        self.status = match save_deck(&mut self.deck, &self.path) {
            Ok(()) => {
                self.saved_data = self.deck.snapshot();
                self.saved_assets = self.deck.assets().clone();
                self.pending_recovery = None;
                format!("Saved {}", self.path.display())
            }
            Err(error) => {
                self.pending_recovery = pending_pre_move_deck(&self.path).ok().flatten();
                error
            }
        };
    }

    fn resume_interrupted_save(&mut self) {
        if self.has_unsaved_edits() {
            self.status =
                "Unsaved edits are present. Save a separate copy before resuming the staged deck."
                    .into();
            return;
        }
        self.status = match resume_pre_move_deck(&self.path)
            .and_then(|staged| open_deck(&self.path).map(|deck| (staged, deck)))
        {
            Ok((staged, deck)) => {
                self.deck = deck;
                self.saved_data = self.deck.snapshot();
                self.saved_assets = self.deck.assets().clone();
                self.pending_recovery = None;
                self.textures.clear();
                self.reconcile_selection();
                format!(
                    "Resumed staged deck from {}. The original remains in the recovery backup.",
                    staged.display()
                )
            }
            Err(error) => error,
        };
    }

    fn abandon_interrupted_save(&mut self) {
        self.status = match abandon_pre_move_deck(&self.path) {
            Ok(staged) => {
                self.pending_recovery = None;
                format!(
                    "Abandoned interrupted save. Original remains open; staged edits remain at {}. Save is available.",
                    staged.display()
                )
            }
            Err(error) => error,
        };
    }

    fn save_copy(&mut self) {
        let parent = self
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let stem = self
            .path
            .file_stem()
            .and_then(|part| part.to_str())
            .unwrap_or("slides");
        let path = parent.join(format!("{stem}-copy-{}.sstrike", Uuid::new_v4()));
        let mut copy = self.deck.clone();
        copy.disk_baseline = None;
        copy.recovery_backup = None;
        self.status = match save_deck(&mut copy, &path) {
            Ok(()) => {
                self.path = path;
                self.deck = copy;
                self.saved_data = self.deck.snapshot();
                self.saved_assets = self.deck.assets().clone();
                self.pending_recovery = None;
                format!("Saved editable copy at {}", self.path.display())
            }
            Err(error) => error,
        };
    }

    fn request_reopen(&mut self) {
        if self.has_unsaved_edits() {
            self.reopen_requested = true;
            self.status =
                "Unsaved edits are present. Confirm discard to reopen the disk version.".into();
        } else {
            self.confirm_reopen();
        }
    }

    fn confirm_reopen(&mut self) {
        let opened = if self.path.exists() {
            open_deck(&self.path).map(|deck| (deck, None))
        } else {
            recover_missing_deck(&self.path).and_then(|result| {
                result
                    .map(|(deck, backup)| (deck, Some(backup)))
                    .ok_or_else(|| "No saved deck exists at this path.".into())
            })
        };
        self.status = match opened {
            Ok((deck, backup)) => {
                self.saved_data = deck.snapshot();
                self.saved_assets = deck.assets().clone();
                self.textures.clear();
                self.deck = deck;
                self.reopen_requested = false;
                self.reconcile_selection();
                if let Some(backup) = backup {
                    self.pending_recovery = None;
                    format!(
                        "Recovered interrupted save from {}; Save restores the missing deck.",
                        backup.display()
                    )
                } else {
                    let (pending, status) = Self::inspect_recovery(&self.path);
                    self.pending_recovery = pending;
                    status
                }
            }
            Err(error) => error,
        };
    }

    fn export_pdf(&mut self) {
        let path = self.path.with_extension("pdf");
        self.status = match export_pdf_bytes(&self.deck).and_then(|report| {
            write_new(&path, &report.bytes)?;
            Ok(format!(
                "Exported {}. {}",
                path.display(),
                report.warnings.join(" ")
            ))
        }) {
            Ok(message) => message,
            Err(error) => error,
        };
    }

    fn export_pngs(&mut self) {
        self.status = match export_png_bytes(&self.deck).and_then(|reports| {
            let count = reports.len();
            let path = write_png_batch(&self.path, &reports, write_new)?;
            Ok(format!(
                "Exported {count} slide PNGs to {}.",
                path.display()
            ))
        }) {
            Ok(message) => message,
            Err(error) => error,
        };
    }

    fn add_image(&mut self) {
        let path = PathBuf::from(self.image_path.trim());
        let result = (|| -> Result<(), String> {
            let size = fs::metadata(&path).map_err(|e| e.to_string())?.len();
            if size > 10 * 1024 * 1024 {
                return Err("image exceeds 10 MiB".into());
            }
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            let mut next = self.deck.clone();
            let hash = next.add_png_or_jpeg(&bytes)?;
            let alt = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("Image");
            let object = SlideObject::image(hash, alt, Rect::new(110.0, 130.0, 320.0, 180.0)?);
            next.apply(Command::AddObject {
                slide: self.slide,
                object,
            })?;
            self.deck = next;
            Ok(())
        })();
        self.status = result
            .map(|_| "Image added; save the deck to keep this change.".into())
            .unwrap_or_else(|error| error);
    }

    fn texture(&mut self, ui: &egui::Ui, hash: &str) -> Option<egui::TextureId> {
        if !self.textures.contains_key(hash) {
            let bytes = self.deck.assets().get(hash)?;
            let (width, height) = image::ImageReader::new(std::io::Cursor::new(bytes))
                .with_guessed_format()
                .ok()?
                .into_dimensions()
                .ok()?;
            if width == 0
                || height == 0
                || width > 8192
                || height > 8192
                || u64::from(width) * u64::from(height) > 20_000_000
            {
                return None;
            }
            let rgba = image::load_from_memory(bytes).ok()?.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            let texture = ui
                .ctx()
                .load_texture(hash, image, egui::TextureOptions::LINEAR);
            self.textures.insert(hash.into(), texture);
        }
        self.textures.get(hash).map(egui::TextureHandle::id)
    }

    fn canvas(&mut self, ui: &mut egui::Ui, slide: Uuid, interactive: bool) {
        let Ok(scene) = compose_slide(&self.deck, slide) else {
            ui.label("This slide cannot be composed; check its objects and assets.");
            return;
        };
        let scale = (ui.available_width() / scene.width as f32)
            .min(ui.available_height() / scene.height as f32)
            .clamp(0.15, 1.5);
        let size = egui::vec2(scene.width as f32 * scale, scene.height as f32 * scale);
        let (frame, response) = ui.allocate_exact_size(
            size,
            if interactive {
                egui::Sense::click_and_drag()
            } else {
                egui::Sense::hover()
            },
        );
        let painter = ui.painter_at(frame);
        painter.rect_filled(frame, egui::CornerRadius::ZERO, egui::Color32::WHITE);
        for draw in &scene.draws {
            let b = draw.bounds;
            let rect = egui::Rect::from_min_size(
                frame.min + egui::vec2(b.x as f32 * scale, b.y as f32 * scale),
                egui::vec2(b.width as f32 * scale, b.height as f32 * scale),
            );
            match &draw.kind {
                DrawKind::Text {
                    text,
                    size_pt,
                    color,
                } => {
                    painter.text(
                        rect.min,
                        egui::Align2::LEFT_TOP,
                        text,
                        egui::FontId::proportional(*size_pt as f32 * scale),
                        egui::Color32::from_rgb(color[0], color[1], color[2]),
                    );
                }
                DrawKind::Rectangle { fill } => {
                    painter.rect_filled(
                        rect,
                        egui::CornerRadius::ZERO,
                        egui::Color32::from_rgb(fill[0], fill[1], fill[2]),
                    );
                }
                DrawKind::Image { hash, .. } => {
                    if let Some(texture) = self.texture(ui, hash) {
                        painter.image(
                            texture,
                            rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    } else {
                        painter.rect_filled(
                            rect,
                            egui::CornerRadius::ZERO,
                            egui::Color32::LIGHT_RED,
                        );
                    }
                }
            }
        }
        if !interactive {
            return;
        }
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                self.object = scene
                    .draws
                    .iter()
                    .rev()
                    .find(|draw| {
                        let b = draw.bounds;
                        let x = f64::from((pos.x - frame.min.x) / scale);
                        let y = f64::from((pos.y - frame.min.y) / scale);
                        x >= b.x && x <= b.x + b.width && y >= b.y && y <= b.y + b.height
                    })
                    .map(|draw| draw.source_id);
            }
        }
        if response.dragged() {
            if let Some(object) = self.object {
                let delta = response.drag_delta() - self.drag_applied;
                self.drag_applied = response.drag_delta();
                if let Some(item) = self
                    .deck
                    .slides()
                    .iter()
                    .find(|s| s.id == slide)
                    .and_then(|s| s.objects.iter().find(|o| o.id == object))
                {
                    let mut bounds = item.bounds;
                    bounds.x += f64::from(delta.x / scale);
                    bounds.y += f64::from(delta.y / scale);
                    self.apply(Command::SetBounds {
                        slide,
                        object,
                        bounds,
                    });
                }
            }
        } else {
            self.drag_applied = egui::Vec2::ZERO;
        }
    }
}

fn write_png_batch(
    deck_path: &std::path::Path,
    reports: &[crate::ExportBytes],
    write: impl Fn(&std::path::Path, &[u8]) -> Result<(), String>,
) -> Result<PathBuf, String> {
    let parent = deck_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let stem = deck_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("slides");
    let token = Uuid::new_v4();
    let staged = parent.join(format!(".{stem}-png-{token}.staged"));
    let output = parent.join(format!("{stem}-png-{token}"));
    fs::create_dir(&staged).map_err(|e| format!("cannot stage PNG batch: {e}"))?;
    for (index, report) in reports.iter().enumerate() {
        let path = staged.join(format!("{:02}.png", index + 1));
        write(&path, &report.bytes).map_err(|error| {
            format!(
                "PNG batch failed on slide {}: {error}; staged files retained at {}",
                index + 1,
                staged.display()
            )
        })?;
    }
    if output.exists() {
        return Err(format!(
            "PNG output path appeared during export; staged files retained at {}",
            staged.display()
        ));
    }
    fs::rename(&staged, &output).map_err(|error| {
        format!(
            "PNG batch could not be published: {error}; staged files retained at {}",
            staged.display()
        )
    })?;
    Ok(output)
}

fn write_new(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(path);
        return Err(format!("{}: {error}", path.display()));
    }
    Ok(())
}

impl eframe::App for NativeEditor {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root_ui.ctx().clone();
        if self.presenting {
            if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.presenting = false;
            }
            if ctx.input(|input| input.key_pressed(egui::Key::ArrowRight)) {
                self.present_index = (self.present_index + 1).min(self.deck.slides().len() - 1);
            }
            if ctx.input(|input| input.key_pressed(egui::Key::ArrowLeft)) {
                self.present_index = self.present_index.saturating_sub(1);
            }
            egui::CentralPanel::default().show(root_ui, |ui| {
                if ui.button("Exit presentation (Esc)").clicked() {
                    self.presenting = false;
                }
                let slide = self.deck.slides()[self.present_index].id;
                self.canvas(ui, slide, false);
            });
            return;
        }
        egui::Panel::top("toolbar").show(root_ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Save").clicked() {
                    self.save();
                }
                if ui.button("Save copy").clicked() {
                    self.save_copy();
                }
                if self.pending_recovery.is_some() {
                    if ui.button("Resume interrupted save").clicked() {
                        self.resume_interrupted_save();
                    }
                    if ui.button("Abandon staged save").clicked() {
                        self.abandon_interrupted_save();
                    }
                }
                if ui.button("Reopen saved").clicked() {
                    self.request_reopen();
                }
                if ui.button("Undo").clicked() {
                    self.undo();
                }
                if ui.button("Redo").clicked() {
                    self.redo();
                }
                if ui.button("PDF").clicked() {
                    self.export_pdf();
                }
                if ui.button("PNGs").clicked() {
                    self.export_pngs();
                }
                if ui.button("Present").clicked() {
                    self.present_index = self
                        .deck
                        .slides()
                        .iter()
                        .position(|s| s.id == self.slide)
                        .unwrap_or(0);
                    self.presenting = true;
                }
            });
            ui.label(&self.status);
        });
        egui::Panel::left("slides")
            .resizable(true)
            .default_size(175.0)
            .show(root_ui, |ui| {
                ui.heading("Slides");
                let slides: Vec<_> = self
                    .deck
                    .slides()
                    .iter()
                    .map(|s| (s.id, s.objects.len()))
                    .collect();
                for (index, (id, objects)) in slides.iter().enumerate() {
                    if ui
                        .selectable_label(
                            self.slide == *id,
                            format!("{:02}  ·  {objects} objects", index + 1),
                        )
                        .clicked()
                    {
                        self.slide = *id;
                        self.object = None;
                    }
                }
                if ui.button("Add slide").clicked() {
                    self.apply(Command::AddSlide {
                        layout: Layout::TitleContent,
                    });
                    self.slide = self
                        .deck
                        .slides()
                        .last()
                        .map(|s| s.id)
                        .unwrap_or(self.slide);
                }
                if ui.button("Duplicate").clicked() {
                    self.apply(Command::DuplicateSlide { slide: self.slide });
                }
                if ui.button("Delete").clicked() {
                    self.apply(Command::DeleteSlide { slide: self.slide });
                    self.slide = self.deck.slides()[0].id;
                    self.object = None;
                }
                if let Some(index) = self.deck.slides().iter().position(|s| s.id == self.slide) {
                    if index > 0 && ui.button("Move up").clicked() {
                        self.apply(Command::MoveSlide {
                            slide: self.slide,
                            to: index - 1,
                        });
                    }
                    if index + 1 < self.deck.slides().len() && ui.button("Move down").clicked() {
                        self.apply(Command::MoveSlide {
                            slide: self.slide,
                            to: index + 1,
                        });
                    }
                }
            });
        egui::Panel::right("inspector")
            .resizable(true)
            .default_size(270.0)
            .show(root_ui, |ui| {
                ui.heading("Inspector");
                if ui.button("Add text").clicked() {
                    self.apply(Command::AddObject {
                        slide: self.slide,
                        object: SlideObject::text(
                            "Text",
                            Rect {
                                x: 80.0,
                                y: 80.0,
                                width: 500.0,
                                height: 90.0,
                            },
                            32.0,
                        ),
                    });
                }
                if ui.button("Add rectangle").clicked() {
                    self.apply(Command::AddObject {
                        slide: self.slide,
                        object: SlideObject {
                            id: Uuid::new_v4(),
                            bounds: Rect {
                                x: 100.0,
                                y: 160.0,
                                width: 260.0,
                                height: 130.0,
                            },
                            kind: ObjectKind::Rectangle {
                                fill: [68, 111, 198],
                            },
                        },
                    });
                }
                ui.label("Local PNG or JPEG path");
                ui.text_edit_singleline(&mut self.image_path);
                if ui.button("Add image").clicked() {
                    self.add_image();
                }
                let items = self
                    .deck
                    .slides()
                    .iter()
                    .find(|s| s.id == self.slide)
                    .map(|s| s.objects.clone())
                    .unwrap_or_default();
                ui.separator();
                for (index, item) in items.iter().enumerate() {
                    let label = match &item.kind {
                        ObjectKind::Text { text, .. } => format!("{} Text: {}", index + 1, text),
                        ObjectKind::Rectangle { .. } => format!("{} Rectangle", index + 1),
                        ObjectKind::Image(image) => format!("{} Image: {}", index + 1, image.alt),
                    };
                    if ui
                        .selectable_label(self.object == Some(item.id), label)
                        .clicked()
                    {
                        self.object = Some(item.id);
                    }
                }
                if let Some(item) = items.iter().find(|o| Some(o.id) == self.object) {
                    let mut b = item.bounds;
                    ui.horizontal(|ui| {
                        ui.label("X / Y");
                        ui.add(egui::DragValue::new(&mut b.x));
                        ui.add(egui::DragValue::new(&mut b.y));
                    });
                    ui.horizontal(|ui| {
                        ui.label("W / H");
                        ui.add(egui::DragValue::new(&mut b.width));
                        ui.add(egui::DragValue::new(&mut b.height));
                    });
                    if b != item.bounds {
                        self.apply(Command::SetBounds {
                            slide: self.slide,
                            object: item.id,
                            bounds: b,
                        });
                    }
                    if let ObjectKind::Text { text, .. } = &item.kind {
                        let mut draft = text.clone();
                        if ui.text_edit_multiline(&mut draft).changed() {
                            self.apply(Command::SetText {
                                slide: self.slide,
                                object: item.id,
                                text: draft,
                            });
                        }
                    }
                }
                ui.separator();
                ui.label("Speaker notes (private)");
                if let Some(notes) = self
                    .deck
                    .slides()
                    .iter()
                    .find(|s| s.id == self.slide)
                    .map(|s| s.notes.clone())
                {
                    let mut draft = notes.clone();
                    if ui.text_edit_multiline(&mut draft).changed() {
                        self.apply(Command::SetNotes {
                            slide: self.slide,
                            notes: draft,
                        });
                    }
                }
            });
        egui::CentralPanel::default().show(root_ui, |ui| {
            ui.heading("Slide canvas");
            self.canvas(ui, self.slide, true);
        });
        if self.reopen_requested {
            egui::Window::new("Discard unsaved edits?")
                .collapsible(false)
                .show(&ctx, |ui| {
                    ui.label(
                        "Reopening reads the disk deck and discards unsaved edits in this window.",
                    );
                    if ui.button("Discard edits and reopen").clicked() {
                        self.confirm_reopen();
                    }
                    if ui.button("Keep editing").clicked() {
                        self.reopen_requested = false;
                    }
                });
        }
        self.capture_after_frame(&ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn pending_save(path: &std::path::Path) -> (PathBuf, PathBuf) {
        let backup = path.with_extension("sstrike.previous");
        let staged = path.with_extension("sstrike.test.tmp");
        let mut candidate = open_deck(path).unwrap();
        candidate.disk_baseline = None;
        candidate
            .apply(Command::SetTitle {
                title: "Staged edit".into(),
            })
            .unwrap();
        save_deck(&mut candidate, &staged).unwrap();
        let original_digest: [u8; 32] = Sha256::digest(fs::read(path).unwrap()).into();
        let published_digest: [u8; 32] = Sha256::digest(fs::read(&staged).unwrap()).into();
        let record = serde_json::json!({
            "version": 2,
            "backup": backup.file_name().unwrap().to_string_lossy(),
            "staged": staged.file_name().unwrap().to_string_lossy(),
            "original_digest": original_digest,
            "published_digest": published_digest,
        });
        fs::write(
            path.with_extension("sstrike.recovery"),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        (backup, staged)
    }

    #[test]
    fn undo_and_reopen_reconcile_selection_without_discarding_silently() {
        let root = std::env::temp_dir().join(format!("slidestrike-editor-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("brief.sstrike");
        let mut app = NativeEditor::open_or_create(path).unwrap();
        let original = app.slide;
        app.save();
        app.apply(Command::AddSlide {
            layout: Layout::TitleContent,
        });
        app.slide = app.deck.slides().last().unwrap().id;
        app.request_reopen();
        assert!(app.reopen_requested);
        assert_eq!(app.deck.slides().len(), 2);
        app.reopen_requested = false;
        app.undo();
        assert_eq!(app.slide, original);
        assert!(app.object.is_none());
        app.redo();
        assert!(app.deck.slides().iter().any(|slide| slide.id == app.slide));
        app.request_reopen();
        assert!(app.reopen_requested);
        app.confirm_reopen();
        assert_eq!(app.deck.slides().len(), 1);
        assert_eq!(app.slide, original);
    }

    #[test]
    fn failed_png_batch_keeps_staging_and_retry_publishes_complete_directory() {
        let root = std::env::temp_dir().join(format!("slidestrike-png-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let deck_path = root.join("brief.sstrike");
        let reports = [
            crate::ExportBytes {
                bytes: b"first".to_vec(),
                warnings: Vec::new(),
            },
            crate::ExportBytes {
                bytes: b"second".to_vec(),
                warnings: Vec::new(),
            },
        ];
        let error = write_png_batch(&deck_path, &reports, |path, bytes| {
            if path.file_name().and_then(|name| name.to_str()) == Some("02.png") {
                Err("simulated second write failure".into())
            } else {
                write_new(path, bytes)
            }
        })
        .unwrap_err();
        assert!(error.contains("staged files retained"));
        let staged: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(staged.len(), 1);
        assert!(staged[0].join("01.png").exists());
        assert!(!staged[0].join("02.png").exists());
        let published = write_png_batch(&deck_path, &reports, write_new).unwrap();
        assert_eq!(fs::read(published.join("01.png")).unwrap(), b"first");
        assert_eq!(fs::read(published.join("02.png")).unwrap(), b"second");
    }

    #[test]
    fn save_copy_after_external_conflict_keeps_both_decks() {
        let root = std::env::temp_dir().join(format!("slidestrike-copy-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let original = root.join("brief.sstrike");
        let mut app = NativeEditor::open_or_create(original.clone()).unwrap();
        app.save();
        let mut external = open_deck(&original).unwrap();
        external
            .apply(Command::SetTitle {
                title: "External".into(),
            })
            .unwrap();
        save_deck(&mut external, &original).unwrap();
        app.apply(Command::SetTitle {
            title: "Local".into(),
        });
        app.save();
        assert!(app.status.contains("changed outside"));
        app.save_copy();
        assert_ne!(app.path, original);
        assert_eq!(open_deck(&original).unwrap().snapshot().title, "External");
        assert_eq!(open_deck(&app.path).unwrap().snapshot().title, "Local");
    }

    #[test]
    fn interrupted_save_opens_recorded_backup_and_restores_missing_path() {
        let root = std::env::temp_dir().join(format!("slidestrike-recovery-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("brief.sstrike");
        let mut app = NativeEditor::open_or_create(path.clone()).unwrap();
        app.apply(Command::SetTitle {
            title: "Retained original".into(),
        });
        app.save();
        let (backup, staged) = pending_save(&path);
        fs::rename(&path, &backup).unwrap();
        app.request_reopen();
        assert!(app.status.contains("Recovered interrupted save"));
        let mut restarted = NativeEditor::open_or_create(path.clone()).unwrap();
        assert!(restarted.status.contains("Recovered"));
        assert_eq!(restarted.deck.snapshot().title, "Retained original");
        restarted.save();
        assert_eq!(
            open_deck(&path).unwrap().snapshot().title,
            "Retained original"
        );
        assert!(backup.exists());
        assert_eq!(open_deck(&staged).unwrap().snapshot().title, "Staged edit");
        assert!(!path.with_extension("sstrike.recovery").exists());
    }

    #[test]
    fn pre_move_recovery_resumes_staged_edit_and_retains_original() {
        let root = std::env::temp_dir().join(format!("slidestrike-premoves-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("brief.sstrike");
        let mut first = NativeEditor::open_or_create(path.clone()).unwrap();
        first.apply(Command::SetTitle {
            title: "Original".into(),
        });
        first.save();
        let (backup, staged) = pending_save(&path);

        let mut restarted = NativeEditor::open_or_create(path.clone()).unwrap();
        assert_eq!(restarted.pending_recovery, Some(staged.clone()));
        assert_eq!(restarted.deck.snapshot().title, "Original");
        restarted.save();
        assert!(restarted.status.contains("Resume or Abandon"));
        restarted.resume_interrupted_save();
        assert_eq!(restarted.deck.snapshot().title, "Staged edit");
        assert_eq!(open_deck(&path).unwrap().snapshot().title, "Staged edit");
        assert_eq!(open_deck(&backup).unwrap().snapshot().title, "Original");
        assert!(staged.exists());
        assert!(!path.with_extension("sstrike.recovery").exists());
    }

    #[test]
    fn pre_move_abandon_preserves_staged_edit_and_allows_normal_save() {
        let root = std::env::temp_dir().join(format!("slidestrike-abandon-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("brief.sstrike");
        let mut first = NativeEditor::open_or_create(path.clone()).unwrap();
        first.apply(Command::SetTitle {
            title: "Original".into(),
        });
        first.save();
        let (_, staged) = pending_save(&path);

        let mut restarted = NativeEditor::open_or_create(path.clone()).unwrap();
        restarted.abandon_interrupted_save();
        assert!(restarted.status.contains("Save is available"));
        restarted.apply(Command::SetTitle {
            title: "New local edit".into(),
        });
        restarted.save();
        assert_eq!(open_deck(&path).unwrap().snapshot().title, "New local edit");
        assert_eq!(open_deck(&staged).unwrap().snapshot().title, "Staged edit");
    }

    #[test]
    fn pre_move_external_edit_blocks_recovery_without_losing_staged_copy() {
        let root =
            std::env::temp_dir().join(format!("slidestrike-external-recovery-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("brief.sstrike");
        let mut first = NativeEditor::open_or_create(path.clone()).unwrap();
        first.apply(Command::SetTitle {
            title: "Original".into(),
        });
        first.save();
        let (backup, staged) = pending_save(&path);
        let external_path = root.join("external.sstrike");
        let mut external = Deck::new("External");
        save_deck(&mut external, &external_path).unwrap();
        fs::copy(&external_path, &path).unwrap();

        let mut restarted = NativeEditor::open_or_create(path.clone()).unwrap();
        assert!(restarted.status.contains("changed outside"));
        restarted.save();
        assert!(restarted.status.contains("changed outside"));
        restarted.resume_interrupted_save();
        assert!(restarted.status.contains("changed outside"));
        restarted.abandon_interrupted_save();
        assert!(restarted.status.contains("changed outside"));
        assert_eq!(open_deck(&path).unwrap().snapshot().title, "External");
        assert_eq!(open_deck(&staged).unwrap().snapshot().title, "Staged edit");
        assert!(!backup.exists());
        assert!(path.with_extension("sstrike.recovery").exists());
    }

    #[test]
    fn post_publication_record_is_cleared_on_next_save() {
        let root = std::env::temp_dir().join(format!("slidestrike-postpublish-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("brief.sstrike");
        let mut first = NativeEditor::open_or_create(path.clone()).unwrap();
        first.apply(Command::SetTitle {
            title: "Original".into(),
        });
        first.save();
        let (backup, staged) = pending_save(&path);
        fs::rename(&path, &backup).unwrap();
        fs::hard_link(&staged, &path).unwrap();

        let mut restarted = NativeEditor::open_or_create(path.clone()).unwrap();
        assert_eq!(restarted.deck.snapshot().title, "Staged edit");
        restarted.save();
        assert!(restarted.status.starts_with("Saved"));
        assert!(!path.with_extension("sstrike.recovery").exists());
        assert_eq!(open_deck(&backup).unwrap().snapshot().title, "Original");
        assert!(staged.exists());
    }

    #[test]
    fn missing_deck_with_unrecorded_backup_does_not_open_blank() {
        let root = std::env::temp_dir().join(format!("slidestrike-backup-{}", Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("brief.sstrike");
        let mut app = NativeEditor::open_or_create(path.clone()).unwrap();
        app.save();
        let backup = path.with_extension("sstrike.previous");
        fs::rename(&path, &backup).unwrap();
        let error = NativeEditor::open_or_create(path).err().unwrap();
        assert!(error.contains("recovery copies exist"));
        assert!(error.contains(&backup.display().to_string()));
    }
}
