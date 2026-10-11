use std::fs;

use slidestrike::{Command, Deck, open_deck, save_deck};
use uuid::Uuid;

#[test]
fn native_deck_round_trip_preserves_order_asset_and_undo() {
    let dir = std::env::temp_dir().join(format!("slidestrike-store-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("brief.sstrike");
    let mut deck = Deck::three_slide_fixture(include_bytes!(
        "../../../fixtures/slidestrike/blue-chart.png"
    ))
    .unwrap();
    let first = deck.slides()[0].id;
    let last = deck.slides()[2].id;
    deck.apply(Command::MoveSlide { slide: last, to: 1 })
        .unwrap();
    save_deck(&mut deck, &path).unwrap();
    let mut reopened = open_deck(&path).unwrap();
    assert_eq!(reopened.slides()[1].id, last);
    assert_eq!(reopened.assets(), deck.assets());
    reopened.undo().unwrap();
    assert_eq!(reopened.slides()[0].id, first);
    assert_eq!(reopened.slides()[2].id, last);
    reopened.redo().unwrap();
    assert_eq!(reopened.slides()[1].id, last);
    reopened
        .apply(Command::SetNotes {
            slide: last,
            notes: "Revised presenter cue.".into(),
        })
        .unwrap();
    save_deck(&mut reopened, &path).unwrap();
    let latest = open_deck(&path).unwrap();
    assert_eq!(latest.slides()[1].notes, "Revised presenter cue.");
    let prior = open_deck(&path.with_extension("sstrike.previous")).unwrap();
    assert_eq!(prior.slides()[1].notes, "Pause for questions.");
}

#[test]
fn external_deck_edit_blocks_stale_save_and_keeps_disk_version() {
    let dir = std::env::temp_dir().join(format!("slidestrike-conflict-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("brief.sstrike");
    let mut first = Deck::new("Original");
    save_deck(&mut first, &path).unwrap();
    let mut other = open_deck(&path).unwrap();
    other
        .apply(Command::SetTitle {
            title: "External edit".into(),
        })
        .unwrap();
    save_deck(&mut other, &path).unwrap();
    let backup = path.with_extension("sstrike.previous");
    let first_backup = fs::read(&backup).unwrap();
    other
        .apply(Command::SetTitle {
            title: "External edit again".into(),
        })
        .unwrap();
    save_deck(&mut other, &path).unwrap();
    assert_eq!(fs::read(&backup).unwrap(), first_backup);
    let external_bytes = fs::read(&path).unwrap();
    first
        .apply(Command::SetTitle {
            title: "Stale local edit".into(),
        })
        .unwrap();
    assert!(
        save_deck(&mut first, &path)
            .unwrap_err()
            .contains("changed outside")
    );
    assert_eq!(fs::read(&path).unwrap(), external_bytes);
    assert_eq!(
        open_deck(&path).unwrap().snapshot().title,
        "External edit again"
    );
    assert!(backup.exists());
}

#[test]
fn malformed_archive_refused_without_changing_source() {
    let dir = std::env::temp_dir().join(format!("slidestrike-bad-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("bad.sstrike");
    fs::write(&path, b"not a deck").unwrap();
    assert!(open_deck(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"not a deck");
}
