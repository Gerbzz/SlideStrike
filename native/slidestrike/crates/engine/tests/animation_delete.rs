//! Regression coverage for the bounded DeckCraft PR #996 deletion-cleanup backport.

use deckcraft_engine::{
    Session,
    model::{Animation, ShapeId},
};
use serde_json::json;

fn setup<const N: usize>() -> (Session, [ShapeId; N]) {
    let mut session = Session::with_new();
    let ids = std::array::from_fn(|i| {
        let rect = [10.0 + i as f64 * 110.0, 10.0, 100.0, 100.0];
        let result = session
            .execute("shape.insert", &json!({"preset": "rect", "rect": rect}))
            .unwrap();
        ShapeId(u32::try_from(result["id"].as_u64().unwrap()).unwrap())
    });
    (session, ids)
}

fn group(session: &mut Session, ids: &[ShapeId]) -> ShapeId {
    let result = session
        .execute("arrange.group", &json!({"ids": ids}))
        .unwrap();
    ShapeId(u32::try_from(result["id"].as_u64().unwrap()).unwrap())
}

fn animations(session: &Session) -> Vec<Animation> {
    session
        .doc()
        .unwrap()
        .current_slide()
        .unwrap()
        .animations
        .clone()
}

fn effect(session: &mut Session, shape: ShapeId, trigger: Option<ShapeId>) {
    let index = animations(session).len();
    session
        .execute("animation.add", &json!({"effect": "fade", "ids": [shape]}))
        .unwrap();
    session
        .execute(
            "animation.timing",
            &json!({
                "index": index, "trigger": trigger, "start": "afterPrevious",
                "duration": 730, "delay": 190, "repeat": 3, "rewind": true
            }),
        )
        .unwrap();
}

#[test]
fn deleting_a_trigger_preserves_effects_and_unrelated_triggers() {
    let (mut session, [animated, trigger, unrelated, other_trigger]) = setup();
    effect(&mut session, animated, Some(trigger));
    effect(&mut session, unrelated, Some(other_trigger));
    effect(&mut session, animated, None);
    effect(&mut session, trigger, Some(animated));
    let mut expected = animations(&session);
    expected.pop(); // The deleted shape loses its own effect.
    expected[0].trigger = None;

    let result = session
        .execute("edit.delete", &json!({"ids": [trigger]}))
        .unwrap();

    assert_eq!(result["deleted"], 1);
    assert!(session.doc().unwrap().shape(trigger).is_none());
    assert_eq!(
        animations(&session),
        expected,
        "only the deleted target/trigger references change"
    );
}

#[test]
fn deleting_a_nested_group_clears_triggers_at_every_removed_level() {
    let (mut session, [animated, a, b, c]) = setup();
    let inner = group(&mut session, &[a, b]);
    let outer = group(&mut session, &[inner, c]);
    for trigger in [a, b, c, inner, outer] {
        effect(&mut session, animated, Some(trigger));
    }
    let mut expected = animations(&session);
    for effect in &mut expected {
        effect.trigger = None;
    }

    let result = session
        .execute("edit.delete", &json!({"ids": [outer]}))
        .unwrap();

    assert_eq!(result["deleted"], 5);
    for id in [a, b, c, inner, outer] {
        assert!(session.doc().unwrap().shape(id).is_none());
    }
    assert_eq!(animations(&session), expected);
}

#[test]
fn deleting_a_nested_group_removes_effects_targeting_its_descendants() {
    let (mut session, [survivor, a, b, c]) = setup();
    let inner = group(&mut session, &[a, b]);
    let outer = group(&mut session, &[inner, c]);
    effect(&mut session, survivor, None);
    let expected = animations(&session);
    for target in [a, b, c, inner, outer] {
        effect(&mut session, target, Some(survivor));
    }

    session
        .execute("edit.delete", &json!({"ids": [outer]}))
        .unwrap();

    assert_eq!(animations(&session), expected);
}

#[test]
fn deleting_last_children_cleans_up_animations_for_emptied_ancestors() {
    let (mut session, [survivor, a, b, c]) = setup();
    let inner = group(&mut session, &[a, b]);
    let outer = group(&mut session, &[inner, c]);
    effect(&mut session, survivor, Some(inner));
    effect(&mut session, survivor, Some(outer));
    let mut expected = animations(&session);
    for effect in &mut expected {
        effect.trigger = None;
    }
    effect(&mut session, inner, Some(survivor));
    effect(&mut session, outer, Some(survivor));

    let result = session
        .execute("edit.delete", &json!({"ids": [a, b, c]}))
        .unwrap();

    assert_eq!(result["deleted"], 5);
    assert!(session.doc().unwrap().shape(inner).is_none());
    assert!(session.doc().unwrap().shape(outer).is_none());
    assert_eq!(animations(&session), expected);
}

#[test]
fn deleting_an_unanimated_shape_keeps_all_existing_effects() {
    let (mut session, [animated, trigger, unrelated]) = setup();
    effect(&mut session, animated, Some(trigger));
    effect(&mut session, trigger, None);
    let before = animations(&session);

    session
        .execute("edit.delete", &json!({"ids": [unrelated]}))
        .unwrap();

    assert_eq!(animations(&session), before);
}

#[test]
fn deleting_shapes_leaves_other_slides_animations_unchanged() {
    let (mut session, [animated, trigger]) = setup();
    effect(&mut session, animated, Some(trigger));
    session
        .execute("slide.duplicate", &json!({"index": 0}))
        .unwrap();
    session.execute("slide.go", &json!({"index": 0})).unwrap();
    let other_before = session.doc().unwrap().doc.slides[1].clone();

    session
        .execute("edit.delete", &json!({"ids": [trigger]}))
        .unwrap();

    assert_eq!(session.doc().unwrap().doc.slides[1], other_before);
    assert_eq!(animations(&session)[0].trigger, None);
}

#[test]
fn deleting_a_group_is_one_undoable_edit_and_roundtrips_after_redo() {
    let (mut session, [survivor, a, b]) = setup();
    let grouped = group(&mut session, &[a, b]);
    effect(&mut session, survivor, Some(a));
    effect(&mut session, b, Some(survivor));
    session
        .execute("edit.select", &json!({"ids": [grouped]}))
        .unwrap();
    let path = std::env::temp_dir().join(format!(
        "slidestrike-animation-delete-{}-{}.deckcraft",
        std::process::id(),
        session.doc().unwrap().uid
    ));
    session
        .execute("file.saveAs", &json!({"path": path}))
        .unwrap();
    assert!(!session.doc().unwrap().is_dirty());
    let before = session.doc().unwrap().doc.clone();
    let selected_before = session.doc().unwrap().selection.clone();
    let undo_before = session.doc().unwrap().history.undo.len();

    session.execute("edit.delete", &json!({})).unwrap();

    assert_eq!(session.doc().unwrap().history.undo.len(), undo_before + 1);
    assert!(session.doc().unwrap().selection.shapes.is_empty());
    assert!(session.doc().unwrap().is_dirty());
    let mut expected = before.slides[0].animations.clone();
    expected.pop();
    expected[0].trigger = None;
    assert_eq!(animations(&session), expected);
    let after = session.doc().unwrap().doc.clone();

    for _ in 0..2 {
        session.execute("edit.undo", &json!({})).unwrap();
        assert_eq!(session.doc().unwrap().doc, before);
        assert_eq!(session.doc().unwrap().selection, selected_before);
        assert!(!session.doc().unwrap().is_dirty());
        session.execute("edit.redo", &json!({})).unwrap();
        assert_eq!(session.doc().unwrap().doc, after);
        assert!(session.doc().unwrap().is_dirty());
    }
    session
        .execute("file.saveAs", &json!({"path": path}))
        .unwrap();
    assert!(!session.doc().unwrap().is_dirty());
    let mut reopened = Session::new();
    reopened
        .execute("file.open", &json!({"path": path}))
        .unwrap();
    assert_eq!(animations(&reopened), expected);
    assert!(reopened.doc().unwrap().shape(a).is_none());
    assert!(reopened.doc().unwrap().shape(b).is_none());
    std::fs::remove_file(path).unwrap();
}
