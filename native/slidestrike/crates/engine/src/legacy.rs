//! Loss-aware conversion of the original `.sstrike` archive.

use std::path::Path;
use std::sync::Arc;

use deckcraft_color::Rgba;
use deckcraft_fonts::FontDb;
use deckcraft_geom::Xfrm;
use deckcraft_model::style::PictureFill;
use deckcraft_model::text::{Align, Anchor, AutoFit, Bullet, Spacing};
use deckcraft_model::{
    ColorRef, Fill, Geom, LayoutType, Line, PhType, Placeholder, Presentation, Shape, ShapeId,
    ShapeKind, TextBody, defaults,
};
use legacy_slidestrike::{Layout, ObjectKind};

use crate::{EngineError, Result};

const LEGACY_FONT: &str = "Source Serif 4";
const SUBSTITUTE_FONT: &str = "Ubuntu";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyFont {
    NoText,
    VerifiedBundledOriginal,
    LocalOriginal,
    Substituted,
}

impl LegacyFont {
    fn family(self) -> &'static str {
        match self {
            Self::VerifiedBundledOriginal | Self::LocalOriginal => LEGACY_FONT,
            Self::Substituted => SUBSTITUTE_FONT,
            Self::NoText => "",
        }
    }
}

pub struct ConvertedDeck {
    pub doc: Presentation,
    pub font: LegacyFont,
}

/// `face` may return a substitute, so accepting it requires checking the resolved family.
/// The bundled original takes priority; another installed face can be used locally if needed.
fn resolve_font(db: &FontDb, requested: &str, allow_substitution: bool) -> Result<LegacyFont> {
    if db
        .face(requested, "Regular")
        .family
        .eq_ignore_ascii_case(requested)
    {
        if requested.eq_ignore_ascii_case(LEGACY_FONT) && db.verified_source_serif_4() {
            return Ok(LegacyFont::VerifiedBundledOriginal);
        }
        return Ok(LegacyFont::LocalOriginal);
    }
    if allow_substitution {
        return Ok(LegacyFont::Substituted);
    }
    Err(EngineError::Other(format!(
        "legacy text requires {requested}; this font is not installed or could not be loaded. Install it locally, or explicitly allowFontSubstitution=true to use Ubuntu; import was refused to avoid an unreported font change"
    )))
}

pub fn convert_path(path: &Path, allow_font_substitution: bool) -> Result<ConvertedDeck> {
    // The old reader bounds ZIP entries, verifies asset hashes and validates object geometry.
    let source = legacy_slidestrike::open_deck(path).map_err(EngineError::Other)?;
    let snapshot = source.snapshot();
    let has_text = snapshot
        .slides
        .iter()
        .flat_map(|s| &s.objects)
        .any(|o| matches!(&o.kind, ObjectKind::Text { text, .. } if !text.is_empty()));
    let font = if has_text {
        resolve_font(FontDb::global(), LEGACY_FONT, allow_font_substitution)?
    } else {
        LegacyFont::NoText
    };
    let mut doc = defaults::blank_presentation(defaults::WIDE, Default::default(), false);
    doc.props.title = snapshot.title;
    for (slide_index, old_slide) in snapshot.slides.iter().enumerate() {
        let kind = match old_slide.layout {
            Layout::Title => LayoutType::Title,
            Layout::TitleContent => LayoutType::TitleAndContent,
            Layout::TwoColumn => LayoutType::TwoContent,
        };
        let layout = defaults::layout_of_kind(&doc, kind).ok_or_else(|| {
            EngineError::Other(format!(
                "legacy slide {slide_index}: native layout is unavailable"
            ))
        })?;
        let mut slide = defaults::new_slide(&mut doc, layout);
        slide.shapes.clear(); // Legacy slides have only explicit objects.
        slide.notes = TextBody::from_text(&old_slide.notes);
        let mut title_inferred = false;
        for (object_index, old_object) in old_slide.objects.iter().enumerate() {
            let bounds = old_object.bounds;
            let mut shape = Shape {
                id: ShapeId(doc.alloc_id()),
                name: format!("Imported object {}", object_index + 1),
                xfrm: Some(Xfrm::new(bounds.x, bounds.y, bounds.width, bounds.height)),
                ..Default::default()
            };
            match &old_object.kind {
                ObjectKind::Text {
                    text,
                    size_pt,
                    color,
                } => {
                    let mut body = TextBody::from_text(text);
                    // The original DesignCraft export used zero text-frame insets. Native text
                    // boxes otherwise inherit 7.2 pt left/right and 3.6 pt top/bottom.
                    body.body.inset_l = Some(0.0);
                    body.body.inset_t = Some(0.0);
                    body.body.inset_r = Some(0.0);
                    body.body.inset_b = Some(0.0);
                    body.body.anchor = Some(Anchor::Top);
                    body.body.autofit = Some(AutoFit::None);
                    for paragraph in &mut body.paragraphs {
                        paragraph.props.align = Some(Align::Left);
                        paragraph.props.margin_left = Some(0.0);
                        paragraph.props.indent = Some(0.0);
                        paragraph.props.bullet = Some(Bullet::None);
                        paragraph.props.space_before = Some(Spacing::Pts(0.0));
                        paragraph.props.space_after = Some(Spacing::Pts(0.0));
                        for run in &mut paragraph.runs {
                            // The installed original face is used without copying it into the
                            // document. Ubuntu is used only after explicit substitution consent.
                            run.props.font = Some(font.family().into());
                            run.props.size = Some(*size_pt);
                            run.props.fill = Some(Fill::solid(ColorRef::rgb(Rgba::rgb(
                                color[0], color[1], color[2],
                            ))));
                        }
                    }
                    shape.text_box = true;
                    shape.fill = Some(Fill::None);
                    shape.line = Some(Line::none());
                    shape.text = Some(body);
                    if !title_inferred && !text.trim().is_empty() {
                        shape.ph = Some(Placeholder {
                            kind: if matches!(old_slide.layout, Layout::Title) {
                                PhType::CtrTitle
                            } else {
                                PhType::Title
                            },
                            idx: 0,
                            ..Default::default()
                        });
                        title_inferred = true;
                    }
                }
                ObjectKind::Rectangle { fill } => {
                    shape.geom = Geom::preset("rect");
                    shape.fill = Some(Fill::solid(ColorRef::rgb(Rgba::rgb(
                        fill[0], fill[1], fill[2],
                    ))));
                    shape.line = Some(Line::none());
                }
                ObjectKind::Image(image) => {
                    let bytes = source.assets().get(&image.hash).ok_or_else(|| {
                        EngineError::Other(format!("legacy slide {slide_index} image is missing"))
                    })?;
                    let content_type = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
                        "image/png"
                    } else if bytes.starts_with(&[0xFF, 0xD8]) {
                        "image/jpeg"
                    } else {
                        return Err(EngineError::Other(format!(
                            "legacy slide {slide_index} uses an unsupported image"
                        )));
                    };
                    let extension = if content_type == "image/png" {
                        "png"
                    } else {
                        "jpg"
                    };
                    let media = doc.add_media(
                        &format!("{}.{}", image.hash, extension),
                        content_type,
                        bytes.clone(),
                    );
                    shape.kind = ShapeKind::Picture {
                        fill: PictureFill {
                            media,
                            ..Default::default()
                        },
                    };
                    shape.descr = image.alt.clone();
                }
            }
            slide.shapes.push(shape);
        }
        doc.slides.push(Arc::new(slide));
    }
    if doc.media.len() != source.assets().len() {
        return Err(EngineError::Other(
            "legacy deck has unattached assets; import would discard them".into(),
        ));
    }
    let problems = doc.validate();
    if !problems.is_empty() {
        return Err(EngineError::Other(format!(
            "converted deck is invalid: {}",
            problems.join("; ")
        )));
    }
    Ok(ConvertedDeck { doc, font })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_family_resolution_does_not_accept_fallback() {
        let db = FontDb::with_font_dirs(Vec::new());
        assert!(db.verified_source_serif_4());
        assert_eq!(
            resolve_font(&db, "Source Serif 4", false).unwrap(),
            LegacyFont::VerifiedBundledOriginal
        );
        assert_eq!(
            resolve_font(&db, "Ubuntu", false).unwrap(),
            LegacyFont::LocalOriginal
        );
        let missing = "A Font Family That Does Not Exist 987654321";
        let err = resolve_font(&db, missing, false).unwrap_err().to_string();
        assert!(err.contains(missing) && err.contains("allowFontSubstitution"));
        assert_eq!(
            resolve_font(&db, missing, true).unwrap(),
            LegacyFont::Substituted
        );
    }

    #[cfg(windows)]
    #[test]
    fn installed_windows_face_can_be_loaded_by_its_real_family() {
        let dir = std::env::var_os("WINDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "C:\\Windows".into())
            .join("Fonts");
        if !dir.join("arial.ttf").exists() {
            return;
        }
        let db = FontDb::with_font_dirs(vec![dir]);
        assert_eq!(
            resolve_font(&db, "Arial", false).unwrap(),
            LegacyFont::LocalOriginal
        );
    }

    #[test]
    fn older_deck_import_preserves_content_and_source_bytes() {
        let png = include_bytes!("../../../../../fixtures/slidestrike/blue-chart.png");
        let mut old = legacy_slidestrike::Deck::three_slide_fixture(png).unwrap();
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("slidestrike-import-{unique}"));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("original.sstrike");
        legacy_slidestrike::save_deck(&mut old, &path).unwrap();
        let original_bytes = std::fs::read(&path).unwrap();

        let converted = convert_path(&path, false).unwrap();
        assert_eq!(converted.font, LegacyFont::VerifiedBundledOriginal);
        assert_eq!(converted.doc.props.title, old.snapshot().title);
        assert_eq!(converted.doc.slides.len(), 3);
        assert_eq!(
            converted.doc.slides[1].shapes.len(),
            old.slides()[1].objects.len()
        );
        assert_eq!(converted.doc.slides[2].notes.text(), "Pause for questions.");
        assert!(
            converted.doc.slides[1]
                .shapes
                .iter()
                .any(|shape| shape.descr == "Retention graphic")
        );
        assert_eq!(converted.doc.media.len(), old.assets().len());

        let native_bytes = deckcraft_format::save(&converted.doc).unwrap();
        let reopened = deckcraft_format::load(&native_bytes).unwrap();
        assert_eq!(reopened.slides.len(), 3);
        assert_eq!(reopened.slides[2].notes.text(), "Pause for questions.");
        assert_eq!(std::fs::read(&path).unwrap(), original_bytes);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
