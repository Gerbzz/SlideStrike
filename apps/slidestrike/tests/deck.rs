use slidestrike::{Command, Deck, Rect, SlideObject};

fn fixture() -> Deck {
    Deck::three_slide_fixture(include_bytes!(
        "../../../fixtures/slidestrike/blue-chart.png"
    ))
    .expect("valid self-authored fixture")
}

#[test]
fn three_slide_reorder_undo_and_notes_stay_with_slide() {
    let mut deck = fixture();
    let originals: Vec<_> = deck.slides().iter().map(|slide| slide.id).collect();
    deck.apply(Command::MoveSlide {
        slide: originals[2],
        to: 1,
    })
    .unwrap();
    assert_eq!(
        deck.slides()
            .iter()
            .map(|slide| slide.id)
            .collect::<Vec<_>>(),
        vec![originals[0], originals[2], originals[1]]
    );
    assert_eq!(deck.slides()[1].notes, "Pause for questions.");
    deck.undo().unwrap();
    assert_eq!(
        deck.slides()
            .iter()
            .map(|slide| slide.id)
            .collect::<Vec<_>>(),
        originals
    );
    deck.redo().unwrap();
    assert_eq!(deck.slides()[1].id, originals[2]);
}

#[test]
fn geometry_and_missing_assets_fail_without_mutating_deck() {
    let mut deck = fixture();
    let before = deck.snapshot();
    let slide = deck.slides()[0].id;
    let object = deck.slides()[0].objects[0].id;
    for width in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            deck.apply(Command::SetBounds {
                slide,
                object,
                bounds: Rect {
                    x: 10.0,
                    y: 10.0,
                    width,
                    height: 30.0,
                },
            })
            .is_err()
        );
        assert_eq!(deck.snapshot(), before);
    }
    let missing_image = SlideObject::image(
        "0".repeat(64),
        "Missing",
        Rect::new(20.0, 20.0, 40.0, 40.0).unwrap(),
    );
    assert!(
        deck.apply(Command::AddObject {
            slide,
            object: missing_image
        })
        .is_err()
    );
    assert_eq!(deck.snapshot(), before);
}

#[test]
fn fixture_has_actual_png_bytes_and_stable_alt_text() {
    let deck = fixture();
    let image = deck.slides()[1]
        .objects
        .iter()
        .find_map(|object| object.image_ref())
        .unwrap();
    assert_eq!(image.alt, "Retention graphic");
    assert_eq!(
        deck.assets()[&image.hash],
        include_bytes!("../../../fixtures/slidestrike/blue-chart.png")
    );
}

#[test]
fn duplicate_remaps_all_ids_and_delete_is_undoable() {
    let mut deck = fixture();
    let original = deck.slides()[1].id;
    let original_objects: Vec<_> = deck.slides()[1]
        .objects
        .iter()
        .map(|item| item.id)
        .collect();
    deck.apply(Command::DuplicateSlide { slide: original })
        .unwrap();
    assert_eq!(deck.slides().len(), 4);
    let copy = &deck.slides()[2];
    assert_ne!(copy.id, original);
    assert_eq!(copy.objects.len(), original_objects.len());
    assert!(
        copy.objects
            .iter()
            .zip(original_objects)
            .all(|(item, old)| item.id != old)
    );
    let copy_id = copy.id;
    deck.apply(Command::DeleteSlide { slide: copy_id }).unwrap();
    assert_eq!(deck.slides().len(), 3);
    deck.undo().unwrap();
    assert_eq!(deck.slides()[2].id, copy_id);
}

#[test]
fn three_slide_seed_matches_reviewed_brief() {
    let brief: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../fixtures/slidestrike/brief.json")).unwrap();
    let deck = fixture();
    assert_eq!(
        deck.slides().len(),
        brief["slides"].as_array().unwrap().len()
    );
    for (slide, expected) in deck
        .slides()
        .iter()
        .zip(brief["slides"].as_array().unwrap())
    {
        if let Some(title) = expected["title"].as_str() {
            assert!(slide.objects.iter().any(|object| matches!(&object.kind,
                slidestrike::ObjectKind::Text { text, .. } if text == title)));
        }
        if let Some(notes) = expected["notes"].as_str() {
            assert_eq!(slide.notes, notes);
        }
    }
}
