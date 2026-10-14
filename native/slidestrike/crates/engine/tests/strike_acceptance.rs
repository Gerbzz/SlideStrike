use std::path::PathBuf;

use deckcraft_engine::Session;
use serde_json::json;

fn output_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../verification/native-slidestrike-20261012")
}

#[test]
fn self_authored_deck_edit_save_reopen_and_export() {
    let out = output_dir();
    std::fs::create_dir_all(&out).unwrap();
    let picture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../fixtures/slidestrike/blue-chart.png");
    let native = out.join("self-authored-brief.deckcraft");
    let pptx = out.join("self-authored-brief.pptx");
    let pdf = out.join("self-authored-brief.pdf");
    let png = out.join("self-authored-brief.png");

    let mut session = Session::new();
    session.execute("file.new", &json!({"blank": true})).unwrap();
    session
        .execute(
            "slide.new",
            &json!({"layout": "titleAndContent", "title": "Quarterly Brief", "body": "A self-authored editable deck"}),
        )
        .unwrap();
    session
        .execute("slide.notes", &json!({"text": "Introduce the quarter and source data."}))
        .unwrap();
    session
        .execute("slide.new", &json!({"layout": "titleOnly", "title": "Revenue Mix"}))
        .unwrap();
    session
        .execute(
            "insert.picture",
            &json!({"path": picture.to_string_lossy(), "rect": [100, 150, 640, 320]}),
        )
        .unwrap();
    session
        .execute("slide.notes", &json!({"text": "Chart is an embedded image in this fixture."}))
        .unwrap();
    let before = session.doc().unwrap().doc.slides[1].shapes.len();
    session
        .execute("insert.textBox", &json!({"rect": [100, 480, 600, 35], "text": "Data refreshed"}))
        .unwrap();
    assert_eq!(session.doc().unwrap().doc.slides[1].shapes.len(), before + 1);
    session.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(session.doc().unwrap().doc.slides[1].shapes.len(), before);
    session.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(session.doc().unwrap().doc.slides[1].shapes.len(), before + 1);
    session
        .execute(
            "slide.new",
            &json!({"layout": "titleAndContent", "title": "Next Steps", "body": "Review, revise, present"}),
        )
        .unwrap();
    session
        .execute("file.saveAs", &json!({"path": native.to_string_lossy()}))
        .unwrap();

    let mut reopened = Session::new();
    reopened
        .execute("file.open", &json!({"path": native.to_string_lossy()}))
        .unwrap();
    let doc = &reopened.doc().unwrap().doc;
    assert_eq!(doc.slides.len(), 3);
    assert_eq!(doc.slides.iter().map(|s| s.title()).collect::<Vec<_>>(), ["Quarterly Brief", "Revenue Mix", "Next Steps"]);
    assert!(doc.slides[0].notes_text().contains("Introduce the quarter"));
    assert!(doc.slides[1].notes_text().contains("embedded image"));
    assert!(!doc.media.is_empty());
    assert_eq!(doc.slides[1].shapes.len(), before + 1);

    reopened
        .execute("file.export", &json!({"path": pptx.to_string_lossy(), "format": "pptx"}))
        .unwrap();
    reopened
        .execute("file.export", &json!({"path": pdf.to_string_lossy(), "format": "pdf"}))
        .unwrap();
    reopened
        .execute("file.export", &json!({"path": png.to_string_lossy(), "format": "png", "slide": 1, "scale": 1.0}))
        .unwrap();
    for path in [&native, &pptx, &pdf, &png] {
        assert!(std::fs::metadata(path).unwrap().len() > 100, "{}", path.display());
    }

    let mut imported = Session::new();
    imported
        .execute("file.open", &json!({"path": pptx.to_string_lossy()}))
        .unwrap();
    assert_eq!(imported.doc().unwrap().doc.slides.len(), 3);
    assert_eq!(imported.doc().unwrap().doc.slides[1].title(), "Revenue Mix");
}
