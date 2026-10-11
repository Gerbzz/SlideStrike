#![cfg(feature = "donor-export")]

use designcraft_doc::Content;
use designcraft_geom::Rect;
use slidestrike::{Deck, donor_document, export_pdf_bytes, export_png_bytes, open_deck, save_deck};

fn brief() -> Deck {
    Deck::three_slide_fixture(include_bytes!(
        "../../../fixtures/slidestrike/blue-chart.png"
    ))
    .expect("valid self-authored brief")
}

#[test]
fn donor_pages_keep_order_image_and_private_notes() {
    let deck = brief();
    let built = donor_document(&deck).expect("convert checked scenes");
    assert_eq!(built.document.page_count(), 3);
    assert_eq!(
        built.object_ids.len(),
        deck.slides().iter().map(|s| s.objects.len()).sum::<usize>()
    );
    let text: Vec<_> = built
        .document
        .stories
        .values()
        .map(|story| story.text.as_str())
        .collect();
    assert!(text.contains(&"Launch Brief"));
    assert!(text.contains(&"Results"));
    assert!(text.contains(&"Next"));
    assert!(!text.contains(&"Pause for questions."));
    assert_eq!(built.document.assets.len(), 1);
    assert_eq!(
        built
            .document
            .assets
            .values()
            .next()
            .unwrap()
            .data
            .as_slice(),
        include_bytes!("../../../fixtures/slidestrike/blue-chart.png")
    );
    assert!(
        built
            .document
            .spreads
            .iter()
            .flat_map(|spread| spread.items.iter())
            .any(|item| item.alt_text == "Retention graphic")
    );
    let graphic = built
        .document
        .spreads
        .iter()
        .flat_map(|spread| spread.items.iter())
        .find_map(|item| match &item.content {
            Content::Graphic(graphic) => Some(graphic),
            _ => None,
        })
        .expect("placed graphic");
    let placed =
        graphic
            .xf
            .transform_rect_bbox(Rect::new(0.0, 0.0, graphic.size.0, graphic.size.1));
    assert!((placed.x0 - 318.0).abs() < 0.1);
    assert!((placed.y0 - 275.0).abs() < 0.1);
    assert!((placed.width() - 320.0).abs() < 0.1);
    assert!((placed.height() - 180.0).abs() < 0.1);
}

#[test]
fn donor_exports_three_real_pngs_and_pdf_pages() {
    let mut deck = brief();
    let pngs = export_png_bytes(&deck).expect("render pages");
    assert_eq!(pngs.len(), 3);
    for png in &pngs {
        let image = image::load_from_memory(&png.bytes).expect("PNG decodes");
        assert_eq!((image.width(), image.height()), (960, 540));
    }
    let pdf = export_pdf_bytes(&deck).expect("PDF export");
    assert!(pdf.bytes.starts_with(b"%PDF-"));
    let parsed = hayro_syntax::Pdf::new(pdf.bytes.clone()).expect("PDF reopens");
    assert_eq!(parsed.pages().len(), 3);
    if let Some(directory) = std::env::var_os("STRIKE_SLIDE_ACCEPTANCE_DIR") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        let native = directory.join("brief.sstrike");
        save_deck(&mut deck, &native).unwrap();
        let reopened = open_deck(&native).unwrap();
        assert_eq!(reopened.snapshot(), deck.snapshot());
        assert_eq!(reopened.assets(), deck.assets());
        for (index, png) in pngs.iter().enumerate() {
            std::fs::write(
                directory.join(format!("slide-{:02}.png", index + 1)),
                &png.bytes,
            )
            .unwrap();
        }
        std::fs::write(directory.join("brief.pdf"), &pdf.bytes).unwrap();
    }
}
