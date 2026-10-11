use slidestrike::{Deck, DrawKind, compose_slide};

#[test]
fn scene_preserves_order_coordinates_and_excludes_notes() {
    let deck = Deck::three_slide_fixture(include_bytes!(
        "../../../fixtures/slidestrike/blue-chart.png"
    ))
    .unwrap();
    let scene = compose_slide(&deck, deck.slides()[2].id).unwrap();
    assert_eq!((scene.width, scene.height), (960.0, 540.0));
    assert!(
        scene
            .draws
            .iter()
            .any(|draw| matches!(&draw.kind, DrawKind::Text { text, .. } if text == "Next"))
    );
    assert!(!scene.draws.iter().any(|draw| matches!(&draw.kind, DrawKind::Text { text, .. } if text.contains("Pause for questions"))));
    assert!(
        !scene
            .reading_order
            .iter()
            .any(|text| text.contains("Pause for questions"))
    );
}

#[test]
fn donor_page_contract_carries_text_image_and_stable_source_ids() {
    let deck = Deck::three_slide_fixture(include_bytes!(
        "../../../fixtures/slidestrike/blue-chart.png"
    ))
    .unwrap();
    let slide = &deck.slides()[1];
    let scene = compose_slide(&deck, slide.id).unwrap();
    let donor = slidestrike::donor_page_contract(&scene);
    assert_eq!((donor.page_width_pt, donor.page_height_pt), (960.0, 540.0));
    assert_eq!(donor.frames.len(), slide.objects.len());
    assert!(
        donor
            .frames
            .iter()
            .zip(&slide.objects)
            .all(|(frame, object)| frame.source_id == object.id && frame.bounds == object.bounds)
    );
    assert!(
        donor
            .frames
            .iter()
            .any(|frame| frame.alt.as_deref() == Some("Retention graphic"))
    );
}
