//! A checked, point-based page contract. The pinned DesignCraft `doc` crate uses
//! points, y down, pages in spread space (`Page`, `Spread`, `Item`); `render`
//! rasterizes, while PDF export is separately in its `pdf` crate. This module
//! deliberately does not assert that either donor path is linked or tested yet.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Deck, ObjectKind, Rect, SLIDE_HEIGHT_PT, SLIDE_WIDTH_PT, validate_snapshot};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DrawKind {
    Text {
        text: String,
        size_pt: f64,
        color: [u8; 3],
    },
    Rectangle {
        fill: [u8; 3],
    },
    Image {
        hash: String,
        alt: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Draw {
    pub source_id: Uuid,
    pub bounds: Rect,
    pub kind: DrawKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub slide_id: Uuid,
    pub width: f64,
    pub height: f64,
    /// Drawing and reading order are the editor's object order, back to front.
    pub draws: Vec<Draw>,
    pub reading_order: Vec<String>,
}

pub fn compose_slide(deck: &Deck, slide_id: Uuid) -> Result<Scene, String> {
    validate_snapshot(&deck.data, &deck.assets)?;
    let slide = deck
        .slides()
        .iter()
        .find(|slide| slide.id == slide_id)
        .ok_or("slide not found")?;
    let mut draws = Vec::with_capacity(slide.objects.len());
    let mut reading_order = Vec::new();
    for object in &slide.objects {
        let kind = match &object.kind {
            ObjectKind::Text {
                text,
                size_pt,
                color,
            } => {
                reading_order.push(text.clone());
                DrawKind::Text {
                    text: text.clone(),
                    size_pt: *size_pt,
                    color: *color,
                }
            }
            ObjectKind::Rectangle { fill } => DrawKind::Rectangle { fill: *fill },
            ObjectKind::Image(image) => {
                reading_order.push(image.alt.clone());
                DrawKind::Image {
                    hash: image.hash.clone(),
                    alt: image.alt.clone(),
                }
            }
        };
        draws.push(Draw {
            source_id: object.id,
            bounds: object.bounds,
            kind,
        });
    }
    Ok(Scene {
        slide_id,
        width: SLIDE_WIDTH_PT,
        height: SLIDE_HEIGHT_PT,
        draws,
        reading_order,
    })
}

/// One slide's loss-checked bridge input for DesignCraft's `Page` / `Item`.
/// `source_id` remains a Strike UUID even though donor item IDs are numeric;
/// the renderer adapter must hold that mapping and prove it after reopen.
#[derive(Clone, Debug, PartialEq)]
pub struct DonorPageContract {
    pub slide_id: Uuid,
    pub page_width_pt: f64,
    pub page_height_pt: f64,
    pub frames: Vec<DonorFrameContract>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DonorFrameContract {
    pub source_id: Uuid,
    pub bounds: Rect,
    pub text: Option<String>,
    pub image_hash: Option<String>,
    pub alt: Option<String>,
}

pub fn donor_page_contract(scene: &Scene) -> DonorPageContract {
    let frames = scene
        .draws
        .iter()
        .map(|draw| {
            let (text, image_hash, alt) = match &draw.kind {
                DrawKind::Text { text, .. } => (Some(text.clone()), None, None),
                DrawKind::Rectangle { .. } => (None, None, None),
                DrawKind::Image { hash, alt } => (None, Some(hash.clone()), Some(alt.clone())),
            };
            DonorFrameContract {
                source_id: draw.source_id,
                bounds: draw.bounds,
                text,
                image_hash,
                alt,
            }
        })
        .collect();
    DonorPageContract {
        slide_id: scene.slide_id,
        page_width_pt: scene.width,
        page_height_pt: scene.height,
        frames,
    }
}
