//! Editable slide model and a checked page contract for a future pinned DesignCraft bridge.
//! The page contract is source-only until the donor crates and exports pass a local build.

#[cfg(feature = "donor-export")]
mod designcraft_adapter;
#[cfg(feature = "native-ui")]
pub mod native_ui;
mod scene;
mod store;

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[cfg(feature = "donor-export")]
pub use designcraft_adapter::{
    DonorBuild, ExportBytes, donor_document, export_pdf_bytes, export_png_bytes,
};
pub use scene::{
    DonorFrameContract, DonorPageContract, Draw, DrawKind, Scene, compose_slide,
    donor_page_contract,
};
pub use store::{open_deck, save_deck};

pub const SLIDE_WIDTH_PT: f64 = 960.0;
pub const SLIDE_HEIGHT_PT: f64 = 540.0;
const MAX_ASSET_BYTES: usize = 10 * 1024 * 1024;
const MAX_SLIDES: usize = 200;
const MAX_OBJECTS_PER_SLIDE: usize = 1000;
const MAX_HISTORY: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Result<Self, String> {
        let rect = Self {
            x,
            y,
            width,
            height,
        };
        rect.validate()?;
        Ok(rect)
    }

    fn validate(self) -> Result<(), String> {
        if ![self.x, self.y, self.width, self.height]
            .into_iter()
            .all(f64::is_finite)
            || self.width <= 0.0
            || self.height <= 0.0
            || self.x.abs() > 10_000.0
            || self.y.abs() > 10_000.0
            || self.width > 10_000.0
            || self.height > 10_000.0
        {
            return Err(
                "object bounds must be finite, positive, and within the supported workspace".into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageRef {
    pub hash: String,
    pub alt: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ObjectKind {
    Text {
        text: String,
        size_pt: f64,
        color: [u8; 3],
    },
    Rectangle {
        fill: [u8; 3],
    },
    Image(ImageRef),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlideObject {
    pub id: Uuid,
    pub bounds: Rect,
    pub kind: ObjectKind,
}

impl SlideObject {
    pub fn text(text: &str, bounds: Rect, size_pt: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            bounds,
            kind: ObjectKind::Text {
                text: text.into(),
                size_pt,
                color: [34, 42, 58],
            },
        }
    }

    pub fn image(hash: String, alt: &str, bounds: Rect) -> Self {
        Self {
            id: Uuid::new_v4(),
            bounds,
            kind: ObjectKind::Image(ImageRef {
                hash,
                alt: alt.into(),
            }),
        }
    }

    pub fn image_ref(&self) -> Option<&ImageRef> {
        match &self.kind {
            ObjectKind::Image(image) => Some(image),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layout {
    Title,
    TitleContent,
    TwoColumn,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slide {
    pub id: Uuid,
    pub layout: Layout,
    pub objects: Vec<SlideObject>,
    pub notes: String,
}

impl Slide {
    pub fn new(layout: Layout) -> Self {
        Self {
            id: Uuid::new_v4(),
            layout,
            objects: Vec::new(),
            notes: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeckSnapshot {
    pub title: String,
    pub slides: Vec<Slide>,
}

#[derive(Clone, Debug)]
pub struct Deck {
    pub(crate) data: DeckSnapshot,
    pub(crate) assets: BTreeMap<String, Vec<u8>>,
    pub(crate) history: Vec<DeckSnapshot>,
    pub(crate) cursor: usize,
    pub(crate) disk_baseline: Option<(PathBuf, [u8; 32])>,
    pub(crate) recovery_backup: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub enum Command {
    SetTitle {
        title: String,
    },
    AddSlide {
        layout: Layout,
    },
    MoveSlide {
        slide: Uuid,
        to: usize,
    },
    DuplicateSlide {
        slide: Uuid,
    },
    DeleteSlide {
        slide: Uuid,
    },
    AddObject {
        slide: Uuid,
        object: SlideObject,
    },
    SetBounds {
        slide: Uuid,
        object: Uuid,
        bounds: Rect,
    },
    SetText {
        slide: Uuid,
        object: Uuid,
        text: String,
    },
    SetNotes {
        slide: Uuid,
        notes: String,
    },
}

impl Deck {
    pub fn new(title: &str) -> Self {
        let data = DeckSnapshot {
            title: title.into(),
            slides: vec![Slide::new(Layout::Title)],
        };
        Self {
            history: vec![data.clone()],
            data,
            assets: BTreeMap::new(),
            cursor: 0,
            disk_baseline: None,
            recovery_backup: None,
        }
    }

    pub fn slides(&self) -> &[Slide] {
        &self.data.slides
    }

    pub fn snapshot(&self) -> DeckSnapshot {
        self.data.clone()
    }

    pub fn assets(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.assets
    }

    pub fn add_png_or_jpeg(&mut self, bytes: &[u8]) -> Result<String, String> {
        if bytes.is_empty() || bytes.len() > MAX_ASSET_BYTES {
            return Err("image asset is empty or too large".into());
        }
        if !matches!(
            image::guess_format(bytes),
            Ok(image::ImageFormat::Png | image::ImageFormat::Jpeg)
        ) {
            return Err("only PNG and JPEG images are supported".into());
        }
        let (width, height) = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| format!("cannot identify image: {e}"))?
            .into_dimensions()
            .map_err(|e| format!("cannot read image dimensions: {e}"))?;
        if width == 0
            || height == 0
            || width > 8192
            || height > 8192
            || u64::from(width) * u64::from(height) > 20_000_000
        {
            return Err("image dimensions exceed the supported limit".into());
        }
        image::load_from_memory(bytes).map_err(|e| format!("cannot decode image: {e}"))?;
        let hash = digest_hex(bytes);
        if !self.assets.contains_key(&hash) && self.assets.len() >= 100 {
            return Err("deck cannot contain more than 100 image assets".into());
        }
        self.assets
            .entry(hash.clone())
            .or_insert_with(|| bytes.to_vec());
        Ok(hash)
    }

    pub fn apply(&mut self, command: Command) -> Result<(), String> {
        let mut next = self.data.clone();
        match command {
            Command::SetTitle { title } => next.title = title,
            Command::AddSlide { layout } => next.slides.push(Slide::new(layout)),
            Command::MoveSlide { slide, to } => {
                if to >= next.slides.len() {
                    return Err("slide destination is out of range".into());
                }
                let from = next
                    .slides
                    .iter()
                    .position(|item| item.id == slide)
                    .ok_or("slide not found")?;
                let moving = next.slides.remove(from);
                next.slides.insert(to, moving);
            }
            Command::DuplicateSlide { slide } => {
                let index = next
                    .slides
                    .iter()
                    .position(|item| item.id == slide)
                    .ok_or("slide not found")?;
                let mut copy = next.slides[index].clone();
                copy.id = Uuid::new_v4();
                for object in &mut copy.objects {
                    object.id = Uuid::new_v4();
                }
                next.slides.insert(index + 1, copy);
            }
            Command::DeleteSlide { slide } => {
                if next.slides.len() == 1 {
                    return Err("deck must keep at least one slide".into());
                }
                let index = next
                    .slides
                    .iter()
                    .position(|item| item.id == slide)
                    .ok_or("slide not found")?;
                next.slides.remove(index);
            }
            Command::AddObject { slide, object } => {
                let target = next
                    .slides
                    .iter_mut()
                    .find(|item| item.id == slide)
                    .ok_or("slide not found")?;
                target.objects.push(object);
            }
            Command::SetBounds {
                slide,
                object,
                bounds,
            } => {
                let item = find_object_mut(&mut next, slide, object)?;
                item.bounds = bounds;
            }
            Command::SetText {
                slide,
                object,
                text,
            } => {
                let item = find_object_mut(&mut next, slide, object)?;
                match &mut item.kind {
                    ObjectKind::Text { text: old, .. } => *old = text,
                    _ => return Err("object is not text".into()),
                }
            }
            Command::SetNotes { slide, notes } => {
                let target = next
                    .slides
                    .iter_mut()
                    .find(|item| item.id == slide)
                    .ok_or("slide not found")?;
                target.notes = notes;
            }
        }
        validate_snapshot(&next, &self.assets)?;
        if next == self.data {
            return Ok(());
        }
        self.history.truncate(self.cursor + 1);
        self.history.push(next.clone());
        if self.history.len() > MAX_HISTORY {
            self.history.remove(0);
        } else {
            self.cursor += 1;
        }
        self.data = next;
        Ok(())
    }

    pub fn undo(&mut self) -> Result<(), String> {
        if self.cursor == 0 {
            return Err("nothing to undo".into());
        }
        self.cursor -= 1;
        self.data = self.history[self.cursor].clone();
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), String> {
        if self.cursor + 1 >= self.history.len() {
            return Err("nothing to redo".into());
        }
        self.cursor += 1;
        self.data = self.history[self.cursor].clone();
        Ok(())
    }

    pub fn three_slide_fixture(png: &[u8]) -> Result<Self, String> {
        let mut deck = Self::new("Launch Brief");
        let hash = deck.add_png_or_jpeg(png)?;
        let box_at = |x, y, width, height| Rect {
            x,
            y,
            width,
            height,
        };
        let title = &mut deck.data.slides[0];
        title.objects.push(SlideObject::text(
            "Launch Brief",
            box_at(76.0, 155.0, 810.0, 80.0),
            48.0,
        ));
        title.objects.push(SlideObject::text(
            "October 2026",
            box_at(78.0, 245.0, 650.0, 45.0),
            26.0,
        ));
        let mut results = Slide::new(Layout::TwoColumn);
        results.objects.push(SlideObject::text(
            "Results",
            box_at(72.0, 44.0, 810.0, 58.0),
            36.0,
        ));
        results.objects.push(SlideObject::text(
            "10 pilots",
            box_at(76.0, 138.0, 354.0, 100.0),
            32.0,
        ));
        results.objects.push(SlideObject::text(
            "8 retained",
            box_at(524.0, 138.0, 360.0, 100.0),
            32.0,
        ));
        results.objects.push(SlideObject::image(
            hash,
            "Retention graphic",
            box_at(318.0, 275.0, 320.0, 180.0),
        ));
        let mut next = Slide::new(Layout::TitleContent);
        next.objects.push(SlideObject::text(
            "Next",
            box_at(72.0, 45.0, 810.0, 62.0),
            38.0,
        ));
        next.notes = "Pause for questions.".into();
        deck.data.slides.extend([results, next]);
        validate_snapshot(&deck.data, &deck.assets)?;
        deck.history = vec![deck.data.clone()];
        Ok(deck)
    }
}

pub(crate) fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn find_object_mut(
    data: &mut DeckSnapshot,
    slide: Uuid,
    object: Uuid,
) -> Result<&mut SlideObject, String> {
    data.slides
        .iter_mut()
        .find(|item| item.id == slide)
        .ok_or("slide not found")?
        .objects
        .iter_mut()
        .find(|item| item.id == object)
        .ok_or_else(|| "object not found".into())
}

pub(crate) fn validate_snapshot(
    data: &DeckSnapshot,
    assets: &BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    if data.title.len() > 2048 || data.slides.is_empty() || data.slides.len() > MAX_SLIDES {
        return Err("invalid deck title or slide count".into());
    }
    let mut ids = HashSet::new();
    for slide in &data.slides {
        if !ids.insert(slide.id)
            || slide.notes.len() > 100_000
            || slide.objects.len() > MAX_OBJECTS_PER_SLIDE
        {
            return Err("duplicate slide ID or overlarge slide".into());
        }
        for object in &slide.objects {
            if !ids.insert(object.id) {
                return Err("duplicate object ID".into());
            }
            object.bounds.validate()?;
            match &object.kind {
                ObjectKind::Text { text, size_pt, .. }
                    if text.len() <= 100_000
                        && size_pt.is_finite()
                        && *size_pt > 0.0
                        && *size_pt <= 500.0 =>
                {
                    ()
                }
                ObjectKind::Rectangle { .. } => (),
                ObjectKind::Image(image)
                    if !image.alt.trim().is_empty()
                        && image.alt.len() <= 1000
                        && assets.contains_key(&image.hash) =>
                {
                    ()
                }
                _ => return Err("invalid text, image, or missing image asset".into()),
            }
        }
    }
    Ok(())
}
