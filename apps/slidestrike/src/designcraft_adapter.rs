//! The selected DesignCraft page/compose/render/PDF crates are an export boundary.
//! Strike IDs, slide order and notes remain in `.sstrike`; notes never enter this document.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::Arc;

use designcraft_color::{Color, Swatch};
use designcraft_compose::Cache;
use designcraft_doc::build::NewDocument;
use designcraft_doc::{
    Asset, AssetId, Content, Document, Fill, Fitting, Graphic, Intent, Item, ItemId, Margins,
    ParaFormat, Shape, SpreadRef,
};
use designcraft_geom::{Affine, Rect as DonorRect, shapes};
use designcraft_pdf::PdfOptions;
use designcraft_render::{RenderOptions, Renderer};
use uuid::Uuid;

use crate::{Deck, DrawKind, SLIDE_HEIGHT_PT, SLIDE_WIDTH_PT, compose_slide, validate_snapshot};

pub struct DonorBuild {
    pub document: Document,
    /// A transient mapping for checking Strike object order and positions in donor output.
    pub object_ids: BTreeMap<Uuid, ItemId>,
}

pub struct ExportBytes {
    pub bytes: Vec<u8>,
    pub warnings: Vec<String>,
}

fn checked_image(bytes: &[u8]) -> Result<(u32, u32), String> {
    let (width, height) = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("cannot identify slide image: {e}"))?
        .into_dimensions()
        .map_err(|e| format!("cannot read slide image dimensions: {e}"))?;
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || u64::from(width) * u64::from(height) > 20_000_000
    {
        return Err("slide image dimensions exceed the supported limit".into());
    }
    image::load_from_memory(bytes).map_err(|e| format!("cannot decode slide image: {e}"))?;
    Ok((width, height))
}

fn swatch(document: &mut Document, id: Uuid, rgb: [u8; 3]) -> String {
    let name = format!("Strike-{}", id.simple());
    document.swatches.push(Swatch::color(
        name.as_str(),
        Color::rgb8(rgb[0], rgb[1], rgb[2]),
    ));
    name
}

/// Convert every checked Strike scene into donor pages in the same order.
pub fn donor_document(deck: &Deck) -> Result<DonorBuild, String> {
    validate_snapshot(&deck.data, &deck.assets)?;
    let mut document = Document::new(&NewDocument {
        title: deck.snapshot().title,
        intent: Intent::Web,
        width: SLIDE_WIDTH_PT,
        height: SLIDE_HEIGHT_PT,
        pages: deck.slides().len(),
        facing_pages: false,
        primary_text_frame: false,
        margins: Margins::uniform(0.0),
        ..NewDocument::default()
    });
    let layer = document.default_layer();
    let mut object_ids = BTreeMap::new();
    let mut asset_ids = BTreeMap::new();
    for (index, slide) in deck.slides().iter().enumerate() {
        let (spread, page) = document.page_loc(index).ok_or("donor page is missing")?;
        let page_x = document.spreads[spread].pages[page].x;
        let spread_ref = SpreadRef::Doc(spread);
        let scene = compose_slide(deck, slide.id)?;
        for draw in scene.draws {
            let b = draw.bounds;
            let rect = DonorRect::new(page_x + b.x, b.y, page_x + b.x + b.width, b.y + b.height);
            let item_id = match draw.kind {
                DrawKind::Text {
                    text,
                    size_pt,
                    color,
                } => {
                    let color_name = swatch(&mut document, draw.source_id, color);
                    let mut para = ParaFormat::default();
                    para.chars.size = Some(size_pt);
                    para.chars.fill = Some(color_name);
                    document
                        .add_text_frame(spread_ref, rect, layer, &text, para)
                        .map_err(|e| e.to_string())?
                        .0
                }
                DrawKind::Rectangle { fill } => {
                    let color_name = swatch(&mut document, draw.source_id, fill);
                    let id = ItemId(document.alloc());
                    let mut item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(rect));
                    item.fill = Fill::swatch(&color_name);
                    document
                        .insert_item(spread_ref, item, None)
                        .map_err(|e| e.to_string())?
                }
                DrawKind::Image { hash, alt } => {
                    let asset_id = if let Some(id) = asset_ids.get(&hash) {
                        *id
                    } else {
                        let bytes = deck
                            .assets()
                            .get(&hash)
                            .ok_or("slide image asset is missing")?;
                        let pixels = checked_image(bytes)?;
                        let id = AssetId(document.alloc());
                        document.assets.insert(
                            id,
                            Arc::new(Asset {
                                id,
                                name: format!("{hash}.image"),
                                mime: designcraft_images::mime(bytes).into(),
                                link: None,
                                data: Arc::new(bytes.clone()),
                                pixels: Some(pixels),
                                page: 0,
                            }),
                        );
                        asset_ids.insert(hash, id);
                        id
                    };
                    let id = ItemId(document.alloc());
                    let mut item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(rect));
                    item.alt_text = alt;
                    let asset = document
                        .assets
                        .get(&asset_id)
                        .ok_or("donor image asset is missing")?;
                    let pixels = asset.pixels.ok_or("donor image dimensions are missing")?;
                    let mut graphic = Graphic {
                        asset: asset_id,
                        size: (f64::from(pixels.0), f64::from(pixels.1)),
                        xf: Affine::IDENTITY,
                        auto_fit: Fitting::FitProportionally,
                        fit_align: 4,
                        crop: [0.0; 4],
                    };
                    graphic.xf = graphic
                        .fitted(rect, Fitting::FitProportionally)
                        .ok_or("slide image cannot fit its frame")?;
                    item.content = Content::Graphic(graphic);
                    document
                        .insert_item(spread_ref, item, None)
                        .map_err(|e| e.to_string())?
                }
            };
            object_ids.insert(draw.source_id, item_id);
        }
    }
    Ok(DonorBuild {
        document,
        object_ids,
    })
}

pub fn export_pdf_bytes(deck: &Deck) -> Result<ExportBytes, String> {
    let built = donor_document(deck)?;
    let report = designcraft_pdf::export_pdf_with_report(
        &built.document,
        &Cache::new(),
        &PdfOptions {
            tagged: true,
            ..PdfOptions::default()
        },
    )
    .map_err(|e| format!("donor PDF export failed: {e}"))?;
    if report.pages != deck.slides().len() {
        return Err("donor PDF page count differs from the deck".into());
    }
    Ok(ExportBytes {
        bytes: report.bytes,
        warnings: report.warnings,
    })
}

pub fn export_png_bytes(deck: &Deck) -> Result<Vec<ExportBytes>, String> {
    let built = donor_document(deck)?;
    let cache = Cache::new();
    let mut renderer = Renderer::new();
    let mut images = Vec::with_capacity(deck.slides().len());
    for index in 0..deck.slides().len() {
        let rendered = renderer
            .render_page(
                &built.document,
                &cache,
                index,
                1.0,
                false,
                &RenderOptions::default(),
            )
            .ok_or_else(|| format!("donor renderer refused slide {}", index + 1))?;
        let rgba =
            image::RgbaImage::from_raw(rendered.width, rendered.height, rendered.to_straight())
                .ok_or("donor renderer produced an invalid RGBA buffer")?;
        let mut output = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(rgba)
            .write_to(&mut output, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        images.push(ExportBytes {
            bytes: output.into_inner(),
            warnings: vec!["Verify glyph fallback and image placement in the exported PNG.".into()],
        });
    }
    Ok(images)
}
