//! Crash recovery (PowerPoint's AutoRecover): unsaved changes of every open presentation are
//! written to a recovery folder (`file.recovery.save`, called on a timer by the app and after a
//! command panics); an explicit save or close/discard removes its entry; after a crash
//! `file.recovery.open` reopens what was left there as unsaved presentations that remember where
//! they were saved. Opening alone keeps the durable entry for a subsequent interrupted session.
//!
//! An entry is `<uid>.deckcraft` (the presentation) plus `<uid>.json` (`{"path", "title",
//! "saved"}`: the original file, the title and when it was written, Unix seconds).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::{DocState, EngineError, Result, Session};

/// The platform's per-user recovery folder.
pub fn default_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        return home.map(|h| h.join("Library/Application Support/SlideStrike/Recovery"));
    }
    if cfg!(target_os = "windows") {
        return std::env::var_os("APPDATA")
            .map(|a| PathBuf::from(a).join("SlideStrike").join("Recovery"));
    }
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home.map(|h| h.join(".local/share")))
        .map(|d| d.join("slidestrike/recovery"))
}

fn files(dir: &Path, uid: u64) -> (PathBuf, PathBuf) {
    (
        dir.join(format!("{uid}.deckcraft")),
        dir.join(format!("{uid}.json")),
    )
}

fn now_secs() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        0
    }
}

/// Remove a presentation's recovery entry (it was saved or closed).
pub fn discard(dir: &Path, uid: u64) {
    let (d, m) = files(dir, uid);
    let _ = std::fs::remove_file(d);
    let _ = std::fs::remove_file(m);
}

/// Write entries for the unsaved presentations (and drop those of saved ones). Returns how many
/// were written.
pub fn save(s: &Session, dir: &Path) -> Result<usize> {
    std::fs::create_dir_all(dir)
        .map_err(|e| EngineError::Other(format!("{}: {e}", dir.display())))?;
    let mut n = 0;
    for d in s.documents() {
        if !d.is_dirty() {
            discard(dir, d.uid);
            continue;
        }
        let (doc, meta) = files(dir, d.uid);
        let bytes =
            deckcraft_format::save(&d.doc).map_err(|e| EngineError::Other(e.to_string()))?;
        // Write then rename, so a crash mid-write never leaves a torn file.
        let tmp = doc.with_extension("tmp");
        std::fs::write(&tmp, &bytes)
            .and_then(|_| std::fs::rename(&tmp, &doc))
            .map_err(|e| EngineError::Other(format!("{}: {e}", doc.display())))?;
        let m = json!({"path": d.path, "title": d.title(), "saved": now_secs()});
        std::fs::write(&meta, m.to_string())
            .map_err(|e| EngineError::Other(format!("{}: {e}", meta.display())))?;
        n += 1;
    }
    Ok(n)
}

/// Entries in `dir`: (uid, metadata).
pub fn list(dir: &Path) -> Vec<(u64, Value)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("deckcraft") {
            continue;
        }
        let Some(uid) = p
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u64>().ok())
        else {
            continue;
        };
        // Metadata is a file on disk: anything but an object (corrupt, hand-edited) counts as none.
        let meta = std::fs::read_to_string(p.with_extension("json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .filter(Value::is_object)
            .unwrap_or(json!({}));
        out.push((uid, meta));
    }
    out.sort_by_key(|e| e.0);
    out
}

/// Reopen every entry as an unsaved presentation. Keep its original UID and durable entry until
/// the user explicitly saves or discards it. Unreadable entries are left in place.
pub fn open(s: &mut Session, dir: &Path) -> Result<Vec<usize>> {
    let mut opened = Vec::new();
    for (uid, meta) in list(dir) {
        if s.documents().iter().any(|d| d.uid == uid) {
            continue;
        }
        let (doc, _) = files(dir, uid);
        let Ok(bytes) = std::fs::read(&doc) else {
            continue;
        };
        let Ok(mut d) = deckcraft_format::load(&bytes) else {
            continue;
        };
        deckcraft_format::repair(&mut d);
        let path = meta.get("path").and_then(Value::as_str).map(str::to_string);
        let title = meta
            .get("title")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .unwrap_or("Presentation");
        let recovered_title = if title.ends_with(" (Recovered)") {
            title.to_string()
        } else {
            format!("{title} (Recovered)")
        };
        let Some(mut st) = DocState::recovered(d, path, recovered_title, uid) else {
            continue;
        };
        // Unsaved: the copy on disk (if any) is older than what was recovered.
        st.saved_doc = std::sync::Arc::new((*st.doc).clone());
        st.revision += 1;
        opened.push(s.add_document(st));
    }
    Ok(opened)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("slidestrike-{name}-{}-{id}", std::process::id()))
    }

    #[test]
    fn recovers_unsaved_presentations() {
        let dir = temp("recovery");
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        s.execute("file.new", &json!({})).unwrap();
        // Only the edited presentation is written.
        s.execute("slide.new", &json!({"layout": "blank"})).unwrap();
        assert_eq!(save(&s, &dir).unwrap(), 1);
        assert_eq!(list(&dir).len(), 1);
        // "Crash": a new session reopens it, unsaved, with its slides.
        let mut s2 = Session::new();
        assert_eq!(open(&mut s2, &dir).unwrap().len(), 1);
        let st = s2.doc().unwrap();
        assert!(st.is_dirty());
        assert_eq!(st.doc.slides.len(), 2);
        assert!(st.title().ends_with("(Recovered)"));
        assert_eq!(
            list(&dir).len(),
            1,
            "a recovered edit needs a durable copy until save or discard"
        );
        // Saving drops the entry.
        s.recovery_dir = Some(dir.clone());
        assert_eq!(save(&s, &dir).unwrap(), 1);
        let path = dir.join("saved.deckcraft");
        s.execute("file.saveAs", &json!({"path": path.to_string_lossy()}))
            .unwrap();
        assert!(list(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_entries_are_skipped() {
        let dir = temp("recovery-corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("7.deckcraft"), b"not a presentation").unwrap();
        std::fs::write(dir.join("7.json"), b"[1, 2]").unwrap();
        let mut s = Session::new();
        assert!(open(&mut s, &dir).unwrap().is_empty());
        s.recovery_dir = Some(dir.clone());
        let v = s.execute("file.recovery.list", &json!({})).unwrap();
        assert_eq!(v.as_array().map(Vec::len), Some(1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recovered_edit_survives_two_interrupted_sessions_until_save() {
        let dir = temp("two-crashes");
        let mut first = Session::with_new();
        first
            .execute(
                "slide.notes",
                &json!({"text": "Synthetic recovery marker."}),
            )
            .unwrap();
        first.recovery_dir = Some(dir.clone());
        let uid = first.doc().unwrap().uid;
        assert_eq!(save(&first, &dir).unwrap(), 1);
        drop(first); // An unclean first exit: no explicit save or discard.

        let mut second = Session::new();
        second.recovery_dir = Some(dir.clone());
        assert_eq!(open(&mut second, &dir).unwrap().len(), 1);
        assert_eq!(second.doc().unwrap().uid, uid);
        assert_eq!(
            second.doc().unwrap().doc.slides[0].notes_text(),
            "Synthetic recovery marker."
        );
        assert_eq!(
            list(&dir).len(),
            1,
            "opening must not consume the only durable copy"
        );
        drop(second); // Interrupt again before the periodic recovery save.

        let mut third = Session::new();
        third.recovery_dir = Some(dir.clone());
        assert_eq!(open(&mut third, &dir).unwrap().len(), 1);
        assert_eq!(
            third.doc().unwrap().doc.slides[0].notes_text(),
            "Synthetic recovery marker."
        );
        let path = dir.join("saved.deckcraft");
        third
            .execute("file.saveAs", &json!({"path": path.to_string_lossy()}))
            .unwrap();
        assert!(path.exists());
        assert!(
            list(&dir).is_empty(),
            "an explicit successful save retires recovery"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_recovery_path_is_isolated_from_deckcraft() {
        let path = default_dir().expect("a recovery home is needed for this test");
        let owner = path
            .parent()
            .and_then(Path::file_name)
            .unwrap()
            .to_string_lossy();
        assert_eq!(owner.to_ascii_lowercase(), "slidestrike");
    }

    #[test]
    fn closing_a_dirty_deck_keeps_recovery_until_explicit_discard() {
        let dir = temp("close-dirty");
        let mut session = Session::with_new();
        session.recovery_dir = Some(dir.clone());
        session
            .execute("slide.notes", &json!({"text": "Unsaved note"}))
            .unwrap();
        assert_eq!(save(&session, &dir).unwrap(), 1);
        let uid = session.doc().unwrap().uid;
        session.close_document(0);
        assert_eq!(list(&dir).len(), 1);
        session
            .execute("file.recovery.discard", &json!({"uid": uid}))
            .unwrap();
        assert!(list(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn periodic_resave_after_recovery_keeps_one_recovered_title_suffix() {
        let dir = temp("title-suffix");
        let mut first = Session::with_new();
        first.recovery_dir = Some(dir.clone());
        first
            .execute("slide.notes", &json!({"text": "Saved recovery"}))
            .unwrap();
        assert_eq!(save(&first, &dir).unwrap(), 1);
        drop(first);

        let mut second = Session::new();
        second.recovery_dir = Some(dir.clone());
        assert_eq!(open(&mut second, &dir).unwrap().len(), 1);
        assert_eq!(save(&second, &dir).unwrap(), 1); // The periodic recovery timer runs.
        drop(second);

        let mut third = Session::new();
        assert_eq!(open(&mut third, &dir).unwrap().len(), 1);
        let title = third.doc().unwrap().title();
        assert_eq!(title.matches(" (Recovered)").count(), 1, "{title}");
        assert_eq!(
            third.doc().unwrap().doc.slides[0].notes_text(),
            "Saved recovery"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
