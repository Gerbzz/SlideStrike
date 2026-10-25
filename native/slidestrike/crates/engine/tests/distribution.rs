//! Distribution uses the complete selection extent, including a wide/tall middle shape.

use deckcraft_engine::{Session, model::ShapeId};
use serde_json::json;

fn rects(session: &Session, ids: &[ShapeId; 3]) -> [[f64; 4]; 3] {
    std::array::from_fn(|i| {
        let x = session.doc().unwrap().shape(ids[i]).unwrap().xfrm.unwrap();
        [x.x, x.y, x.w, x.h]
    })
}

fn assert_full_extent_distribution(dir: &str, original: [[f64; 4]; 3], axis: usize) {
    let mut session = Session::with_new();
    let ids = std::array::from_fn(|i| {
        let result = session
            .execute("shape.insert", &json!({"preset": "rect", "rect": original[i]}))
            .unwrap();
        ShapeId(u32::try_from(result["id"].as_u64().unwrap()).unwrap())
    });
    // Selection order must not substitute for the spatial order of the shapes.
    session
        .execute("edit.select", &json!({"ids": [ids[2], ids[0], ids[1]]}))
        .unwrap();
    assert_eq!(rects(&session, &ids), original);
    let selection = session.doc().unwrap().selection.shapes.clone();
    let undo_before = session.doc().unwrap().history.undo.len();
    let mut expected = original;
    for (rect, position) in expected.iter_mut().zip([0.0, 10.0, 110.0]) {
        rect[axis] = position;
    }

    session
        .execute("arrange.distribute", &json!({"dir": dir, "to": "selection"}))
        .unwrap();

    // The middle shape sets the far edge at 120, giving zero gaps, not -40.
    // All other coordinates and dimensions must be preserved.
    assert_eq!(rects(&session, &ids), expected);
    assert_eq!(session.doc().unwrap().selection.shapes, selection);
    assert_eq!(session.doc().unwrap().history.undo.len(), undo_before + 1);
    session.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(rects(&session, &ids), original);
    assert_eq!(session.doc().unwrap().selection.shapes, selection);
    assert_eq!(session.doc().unwrap().history.undo.len(), undo_before);
    session.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(rects(&session, &ids), expected);
    assert_eq!(session.doc().unwrap().selection.shapes, selection);
    assert_eq!(session.doc().unwrap().history.undo.len(), undo_before + 1);
}

#[test]
fn horizontal_distribution_uses_full_selection_extent_and_roundtrips_history() {
    assert_full_extent_distribution(
        "horizontal",
        [[0.0, 10.0, 10.0, 10.0], [20.0, 10.0, 100.0, 10.0], [30.0, 10.0, 10.0, 10.0]],
        0,
    );
}

#[test]
fn vertical_distribution_uses_full_selection_extent_and_roundtrips_history() {
    assert_full_extent_distribution(
        "vertical",
        [[10.0, 0.0, 10.0, 10.0], [10.0, 20.0, 10.0, 100.0], [10.0, 30.0, 10.0, 10.0]],
        1,
    );
}
