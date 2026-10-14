use deckcraft_engine::Session;
use deckcraft_fonts::FontDb;
use legacy_slidestrike::{Deck, save_deck};
use serde_json::json;

fn scratch(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "slidestrike-legacy-{name}-{}-{id}",
        std::process::id()
    ))
}

#[test]
fn image_notes_and_order_survive_native_editable_roundtrip() {
    let picture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../fixtures/slidestrike/blue-chart.png");
    let png = std::fs::read(picture).unwrap();
    let source = scratch("source").with_extension("sstrike");
    let target = scratch("converted").with_extension("deckcraft");
    let mut old = Deck::three_slide_fixture(&png).unwrap();
    let expected = old.snapshot();
    save_deck(&mut old, &source).unwrap();
    let original = std::fs::read(&source).unwrap();

    let mut session = Session::new();
    let report = session
        .execute(
            "file.importLegacy",
            &json!({"path": source, "allowFontSubstitution": true}),
        )
        .unwrap();
    assert_eq!(report["saveAsRequired"], true);
    assert_eq!(report["identifiersRegenerated"], true);
    assert!(FontDb::global().verified_source_serif_4());
    assert!(report["fontSubstitution"].is_null());
    assert_eq!(report["fontResolution"]["source"], "pinned-ofl-original");
    assert_eq!(report["fontResolution"]["portable"], true);
    let expected_font = "Source Serif 4";
    assert_eq!(report["titleInferred"], true);
    let doc = &session.doc().unwrap().doc;
    assert!(session.doc().unwrap().path.is_none());
    assert_eq!(doc.slides.len(), 3);
    assert_eq!(doc.props.title, expected.title);
    assert_eq!(
        doc.slides
            .iter()
            .map(|slide| slide.title())
            .collect::<Vec<_>>(),
        vec![
            "Launch Brief".to_string(),
            "Results".to_string(),
            "Next".to_string()
        ]
    );
    for (native, legacy) in doc.slides.iter().zip(&expected.slides) {
        assert_eq!(native.shapes.len(), legacy.objects.len());
        assert_eq!(native.notes_text(), legacy.notes);
        for (shape, object) in native.shapes.iter().zip(&legacy.objects) {
            let box_at = shape.xfrm.unwrap();
            assert_eq!(
                [box_at.x, box_at.y, box_at.w, box_at.h],
                [
                    object.bounds.x,
                    object.bounds.y,
                    object.bounds.width,
                    object.bounds.height
                ]
            );
            if let legacy_slidestrike::ObjectKind::Text { .. } = &object.kind {
                let body = shape.text.as_ref().unwrap();
                assert_eq!(
                    [
                        body.body.inset_l,
                        body.body.inset_t,
                        body.body.inset_r,
                        body.body.inset_b
                    ],
                    [Some(0.0); 4]
                );
                assert!(
                    body.paragraphs
                        .iter()
                        .flat_map(|p| &p.runs)
                        .all(|r| r.props.font.as_deref() == Some(expected_font))
                );
            }
        }
    }
    let rendered = deckcraft_render::render_slide(doc, 1, &Default::default());
    assert_eq!((rendered.width, rendered.height), (960, 540));
    let title_left = (60..180)
        .find(|x| {
            (44..100).any(|y| {
                let px = rendered.pixel(*x, y);
                px[0] < 100 && px[1] < 100 && px[2] < 100
            })
        })
        .unwrap();
    assert!(
        title_left <= 80,
        "legacy title frame starts at x=72; native glyph begins at {title_left}"
    );
    let chart_blue = rendered.pixel(380, 360);
    assert!(
        chart_blue[2] > chart_blue[0] && chart_blue[2] > chart_blue[1],
        "imported chart lost its blue pixels: {chart_blue:?}"
    );
    assert_eq!(doc.media.len(), 1);
    assert_eq!(doc.media[0].data.as_slice(), png.as_slice());
    session.execute("slide.go", &json!({"index": 2})).unwrap();
    session
        .execute("slide.notes", &json!({"text": "Questions and follow-up."}))
        .unwrap();
    session.execute("edit.undo", &json!({})).unwrap();
    session.execute("edit.redo", &json!({})).unwrap();
    session
        .execute("file.saveAs", &json!({"path": target}))
        .unwrap();
    let mut reopened = Session::new();
    reopened
        .execute("file.open", &json!({"path": target}))
        .unwrap();
    assert_eq!(reopened.doc().unwrap().doc.slides.len(), 3);
    assert_eq!(
        reopened.doc().unwrap().doc.slides[0].title(),
        "Launch Brief"
    );
    assert_eq!(
        deckcraft_render::render_slide(&reopened.doc().unwrap().doc, 1, &Default::default()).pixels,
        rendered.pixels,
        "the imported slide render changed after native save/reopen"
    );
    assert_eq!(
        reopened.doc().unwrap().doc.slides[2].notes_text(),
        "Questions and follow-up."
    );
    assert_eq!(
        reopened.doc().unwrap().doc.media[0].data.as_slice(),
        png.as_slice()
    );
    assert_eq!(std::fs::read(&source).unwrap(), original);
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(target);
}

#[test]
fn text_import_uses_verified_pinned_original_without_substitution() {
    let picture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../fixtures/slidestrike/blue-chart.png");
    let png = std::fs::read(picture).unwrap();
    let source = scratch("font-refusal").with_extension("sstrike");
    let mut old = Deck::three_slide_fixture(&png).unwrap();
    save_deck(&mut old, &source).unwrap();
    let original = std::fs::read(&source).unwrap();
    let mut session = Session::with_new();
    let count = session.documents().len();
    let report = session
        .execute("file.importLegacy", &json!({"path": source}))
        .unwrap();
    assert!(FontDb::global().verified_source_serif_4());
    assert_eq!(report["fontResolution"]["resolved"], "Source Serif 4");
    assert_eq!(report["fontResolution"]["source"], "pinned-ofl-original");
    assert!(report["fontSubstitution"].is_null());
    assert_eq!(session.documents().len(), count + 1);
    assert!(session.doc().unwrap().path.is_none());
    assert_eq!(std::fs::read(&source).unwrap(), original);
    let _ = std::fs::remove_file(source);
}

#[test]
fn text_free_legacy_deck_needs_no_font_substitution() {
    let source = scratch("no-font-loss").with_extension("sstrike");
    let mut old = Deck::new("Empty");
    save_deck(&mut old, &source).unwrap();
    let mut session = Session::new();
    let report = session
        .execute("file.importLegacy", &json!({"path": source}))
        .unwrap();
    assert_eq!(report["fontSubstitution"], serde_json::Value::Null);
    assert_eq!(report["fontResolution"], serde_json::Value::Null);
    assert_eq!(report["slides"], 1);
    let _ = std::fs::remove_file(source);
}

#[test]
fn damaged_legacy_deck_does_not_replace_an_open_deck() {
    let source = scratch("damaged").with_extension("sstrike");
    std::fs::write(&source, b"not a deck").unwrap();
    let original = std::fs::read(&source).unwrap();
    let mut session = Session::with_new();
    let count = session.documents().len();
    assert!(
        session
            .execute("file.importLegacy", &json!({"path": source}))
            .is_err()
    );
    assert_eq!(session.documents().len(), count);
    assert_eq!(std::fs::read(&source).unwrap(), original);
    let _ = std::fs::remove_file(source);
}
