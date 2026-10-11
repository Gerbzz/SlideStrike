use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::{Deck, DeckSnapshot, MAX_ASSET_BYTES, MAX_HISTORY, digest_hex, validate_snapshot};

const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_JSON_BYTES: usize = 20 * 1024 * 1024;
const MAX_ASSETS: usize = 100;

#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u32,
    current: DeckSnapshot,
    history: Vec<DeckSnapshot>,
    cursor: usize,
    assets: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct RecoveryRecord {
    version: u32,
    backup: String,
    staged: String,
    #[serde(default)]
    original_digest: Option<[u8; 32]>,
    published_digest: [u8; 32],
}

fn recovery_path(path: &Path) -> PathBuf {
    path.with_extension("sstrike.recovery")
}

fn safe_recovery_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains(':')
        && !name.contains('\0')
        && name != "."
        && name != ".."
}

fn read_recovery(path: &Path) -> Result<Option<RecoveryRecord>, String> {
    let marker = recovery_path(path);
    if !path_present(&marker) {
        return Ok(None);
    }
    if fs::metadata(&marker).map_err(|e| e.to_string())?.len() > 4096 {
        return Err(format!(
            "recovery record is too large: {}",
            marker.display()
        ));
    }
    let record: RecoveryRecord =
        serde_json::from_slice(&fs::read(&marker).map_err(|e| e.to_string())?)
            .map_err(|e| format!("invalid recovery record {}: {e}", marker.display()))?;
    let backup_prefix = previous_version(path)
        .file_name()
        .ok_or("deck filename is missing")?
        .to_string_lossy()
        .into_owned();
    let staged_prefix = format!(
        "{}.",
        path.file_name()
            .ok_or("deck filename is missing")?
            .to_string_lossy()
    );
    if !matches!(record.version, 1 | 2)
        || (record.version == 2 && record.original_digest.is_none())
        || !safe_recovery_name(&record.backup)
        || !safe_recovery_name(&record.staged)
        || !(record.backup == backup_prefix
            || record.backup.starts_with(&format!("{backup_prefix}.")))
        || !record.staged.starts_with(&staged_prefix)
        || !record.staged.ends_with(".tmp")
    {
        return Err(format!("invalid recovery record {}", marker.display()));
    }
    Ok(Some(record))
}

fn write_recovery(
    path: &Path,
    backup: &Path,
    staged: &Path,
    original_digest: [u8; 32],
    published_digest: [u8; 32],
) -> Result<(), String> {
    let marker = recovery_path(path);
    let record = RecoveryRecord {
        version: 2,
        backup: backup
            .file_name()
            .ok_or("backup filename is missing")?
            .to_string_lossy()
            .into_owned(),
        staged: staged
            .file_name()
            .ok_or("staged filename is missing")?
            .to_string_lossy()
            .into_owned(),
        original_digest: Some(original_digest),
        published_digest,
    };
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker)
        .map_err(|e| format!("cannot create recovery record {}: {e}", marker.display()))?;
    file.write_all(&serde_json::to_vec(&record).map_err(|e| e.to_string())?)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("cannot sync recovery record {}: {e}", marker.display()))?;
    Ok(())
}

fn resolve_stale_recovery(path: &Path) -> Result<(), String> {
    if let Some(record) = read_recovery(path)? {
        if path_present(path) && file_digest(path).ok() == Some(record.published_digest) {
            fs::remove_file(recovery_path(path)).map_err(|e| e.to_string())?;
        } else {
            if path_present(path)
                && record
                    .original_digest
                    .is_some_and(|digest| file_digest(path).ok() != Some(digest))
            {
                return Err(format!(
                    "deck changed outside SlideStrike during interrupted save; main file at {}, staged copy at {}. Use Save copy after review",
                    path.display(),
                    recovery_sibling(path, &record.staged).display(),
                ));
            }
            return Err(format!(
                "an interrupted deck save needs Resume or Abandon before Save; staged copy at {}, recovery record at {}",
                recovery_sibling(path, &record.staged).display(),
                recovery_path(path).display(),
            ));
        }
    }
    Ok(())
}

fn recovery_sibling(path: &Path, name: &str) -> PathBuf {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(name)
}

pub(crate) fn pending_pre_move_deck(path: &Path) -> Result<Option<PathBuf>, String> {
    let Some(record) = read_recovery(path)? else {
        return Ok(None);
    };
    if file_digest(path).ok() == Some(record.published_digest) {
        return Ok(None);
    }
    let Some(original_digest) = record.original_digest else {
        return Err(format!(
            "legacy recovery record at {} needs manual review",
            recovery_path(path).display()
        ));
    };
    let staged = recovery_sibling(path, &record.staged);
    if file_digest(&staged).ok() != Some(record.published_digest) {
        return Err(format!(
            "staged deck changed or is missing at {}; inspect it before recovery",
            staged.display()
        ));
    }
    if file_digest(path).ok() != Some(original_digest) {
        return Err(format!(
            "deck changed outside SlideStrike during interrupted save; original and staged copy retained at {} and {}",
            path.display(),
            staged.display()
        ));
    }
    let backup = recovery_sibling(path, &record.backup);
    if path_present(&backup) {
        return Err(format!(
            "recovery backup already exists at {}; inspect it before resuming",
            backup.display()
        ));
    }
    Ok(Some(staged))
}

pub(crate) fn resume_pre_move_deck(path: &Path) -> Result<PathBuf, String> {
    let staged = pending_pre_move_deck(path)?.ok_or("no interrupted deck save can be resumed")?;
    open_deck(&staged)
        .map_err(|e| format!("staged deck is invalid at {}: {e}", staged.display()))?;
    let record = read_recovery(path)?.ok_or("recovery record disappeared")?;
    let original = record
        .original_digest
        .ok_or("recovery record has no original digest")?;
    let backup = recovery_sibling(path, &record.backup);
    fs::rename(path, &backup).map_err(|e| {
        format!(
            "could not preserve original deck at {}: {e}",
            backup.display()
        )
    })?;
    if file_digest(&backup).ok() != Some(original)
        || file_digest(&staged).ok() != Some(record.published_digest)
    {
        if !path_present(path) {
            let _ = fs::hard_link(&backup, path);
        }
        return Err(format!(
            "deck changed during recovery; original at {}, staged copy at {}",
            backup.display(),
            staged.display()
        ));
    }
    if let Err(error) = fs::hard_link(&staged, path) {
        if !path_present(path) {
            let _ = fs::hard_link(&backup, path);
        }
        return Err(format!(
            "could not publish staged deck: {error}; original at {}, staged copy at {}",
            backup.display(),
            staged.display()
        ));
    }
    let _ = fs::remove_file(recovery_path(path));
    Ok(staged)
}

pub(crate) fn abandon_pre_move_deck(path: &Path) -> Result<PathBuf, String> {
    let staged = pending_pre_move_deck(path)?.ok_or("no interrupted deck save can be abandoned")?;
    fs::remove_file(recovery_path(path))
        .map_err(|e| format!("cannot clear recovery record: {e}"))?;
    Ok(staged)
}

pub(crate) fn recover_missing_deck(path: &Path) -> Result<Option<(Deck, PathBuf)>, String> {
    if path_present(path) {
        return Err("deck path is present; do not recover over it".into());
    }
    if let Some(record) = read_recovery(path)? {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let backup = parent.join(record.backup);
        if !path_present(&backup) {
            return Err(format!(
                "deck save was interrupted; backup is missing at {}. Staged file: {}",
                backup.display(),
                parent.join(record.staged).display()
            ));
        }
        if record
            .original_digest
            .is_some_and(|digest| file_digest(&backup).ok() != Some(digest))
        {
            return Err(format!(
                "deck recovery backup changed outside SlideStrike: {}",
                backup.display()
            ));
        }
        let mut deck = open_deck(&backup)?;
        deck.disk_baseline = None;
        deck.recovery_backup = Some(backup.clone());
        return Ok(Some((deck, backup)));
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let prefix = previous_version(path)
        .file_name()
        .ok_or("deck filename is missing")?
        .to_string_lossy()
        .into_owned();
    let backups: Vec<_> = fs::read_dir(parent)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|candidate| {
            candidate.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                name.as_ref() == prefix || name.starts_with(&format!("{prefix}."))
            })
        })
        .collect();
    if !backups.is_empty() {
        return Err(format!(
            "deck is missing; recovery copies exist at {}. Choose a copy before creating a new deck",
            backups
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(None)
}

fn target_path(path: &Path) -> Result<PathBuf, String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path.file_name().ok_or("deck path needs a filename")?;
    Ok(fs::canonicalize(parent)
        .map_err(|e| e.to_string())?
        .join(name))
}

fn file_digest(path: &Path) -> Result<[u8; 32], String> {
    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest.finalize().into())
}

fn path_present(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

fn checked_existing(path: &Path, expected: Option<[u8; 32]>) -> Result<(), String> {
    match expected {
        Some(digest) if file_digest(path).ok() == Some(digest) => Ok(()),
        None if !path_present(path) => Ok(()),
        _ => Err(format!(
            "{} changed outside SlideStrike; reopen to review it or use Save copy",
            path.display()
        )),
    }
}

fn available_backup(path: &Path) -> PathBuf {
    let first = previous_version(path);
    if !path_present(&first) {
        first
    } else {
        path.with_extension(format!("sstrike.previous.{}", Uuid::new_v4()))
    }
}

pub fn save_deck(deck: &mut Deck, path: &Path) -> Result<(), String> {
    validate(deck)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err("deck parent directory does not exist".into());
    }
    let target = target_path(path)?;
    let restoring = if let Some(backup) = &deck.recovery_backup {
        let record =
            read_recovery(path)?.ok_or("recovery record is missing; save a separate copy")?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        if path_present(path) || parent.join(record.backup) != *backup {
            return Err("recovery source changed; save a separate copy".into());
        }
        true
    } else {
        resolve_stale_recovery(path)?;
        false
    };
    let expected = match &deck.disk_baseline {
        Some((bound, digest)) if *bound == target => Some(*digest),
        Some(_) => {
            return Err("this deck editor is bound to another file; create a separate copy".into());
        }
        None => None,
    };
    checked_existing(path, expected)?;
    let manifest = Manifest {
        version: 1,
        current: deck.data.clone(),
        history: deck.history.clone(),
        cursor: deck.cursor,
        assets: deck.assets.keys().cloned().collect(),
    };
    let json = serde_json::to_vec(&manifest).map_err(|e| e.to_string())?;
    if json.len() > MAX_JSON_BYTES {
        return Err("deck metadata exceeds 20 MiB".into());
    }
    let expanded = deck
        .assets
        .values()
        .fold(json.len() as u64, |total, bytes| {
            total.saturating_add(bytes.len() as u64)
        });
    if expanded > MAX_ARCHIVE_BYTES {
        return Err("deck contents exceed the 64 MiB reopen limit".into());
    }
    let temp = path.with_extension(format!("sstrike.{}.tmp", Uuid::new_v4()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let staged = (|| -> Result<(), String> {
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("manifest.json", options)
            .map_err(|e| e.to_string())?;
        zip.write_all(&json).map_err(|e| e.to_string())?;
        for (hash, bytes) in &deck.assets {
            zip.start_file(format!("assets/{hash}.bin"), options)
                .map_err(|e| e.to_string())?;
            zip.write_all(bytes).map_err(|e| e.to_string())?;
        }
        zip.finish()
            .map_err(|e| e.to_string())?
            .sync_all()
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(error) = staged {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    if fs::metadata(&temp).map_err(|e| e.to_string())?.len() > MAX_ARCHIVE_BYTES {
        let _ = fs::remove_file(&temp);
        return Err("deck ZIP exceeds the 64 MiB reopen limit".into());
    }
    let staged_digest = file_digest(&temp)?;
    checked_existing(path, expected)
        .map_err(|error| format!("{error}; staged copy retained at {}", temp.display()))?;
    let backup = if let Some(digest) = expected {
        let backup = available_backup(path);
        write_recovery(path, &backup, &temp, digest, staged_digest)?;
        fs::rename(path, &backup).map_err(|e| {
            let _ = fs::remove_file(recovery_path(path));
            format!(
                "could not preserve current deck; staged copy at {}: {e}",
                temp.display()
            )
        })?;
        if file_digest(&backup).ok() != Some(digest) {
            if !path_present(path) {
                let _ = fs::hard_link(&backup, path);
            }
            return Err(format!(
                "deck changed during save; external original retained at {}, staged copy at {}",
                backup.display(),
                temp.display()
            ));
        }
        Some(backup)
    } else {
        None
    };
    if let Err(error) = fs::hard_link(&temp, path) {
        if let Some(backup) = &backup {
            if !path_present(path) {
                let _ = fs::hard_link(backup, path);
            }
        }
        if expected.is_some() && file_digest(path).ok() == expected {
            let _ = fs::remove_file(recovery_path(path));
        }
        return Err(format!(
            "could not publish deck without replacing another file: {error}; staged copy at {}{}",
            temp.display(),
            backup
                .as_ref()
                .map(|p| format!(", previous file at {}", p.display()))
                .unwrap_or_default()
        ));
    }
    let _ = fs::remove_file(&temp);
    if backup.is_some() || restoring {
        let _ = fs::remove_file(recovery_path(path));
    }
    deck.disk_baseline = Some((target, staged_digest));
    deck.recovery_backup = None;
    Ok(())
}

pub fn open_deck(path: &Path) -> Result<Deck, String> {
    let baseline = file_digest(path)?;
    let size = fs::metadata(path)
        .map_err(|e| {
            if previous_version(path).exists() {
                format!(
                    "deck unavailable ({e}); a previous version is at {}",
                    previous_version(path).display()
                )
            } else {
                e.to_string()
            }
        })?
        .len();
    if size > MAX_ARCHIVE_BYTES {
        return Err("deck archive exceeds 64 MiB".into());
    }
    let mut zip =
        ZipArchive::new(File::open(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if zip.len() == 0 || zip.len() > MAX_ASSETS + 1 {
        return Err("invalid deck archive entry count".into());
    }
    let mut seen = BTreeSet::new();
    let mut manifest = None;
    let mut assets = BTreeMap::new();
    let mut expanded = 0u64;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().map_err(|e| e.to_string())?.to_string();
        if !seen.insert(name.clone()) {
            return Err("duplicate deck archive entry".into());
        }
        let limit = if name == "manifest.json" {
            MAX_JSON_BYTES
        } else {
            MAX_ASSET_BYTES
        };
        if entry.size() > limit as u64 || expanded.saturating_add(entry.size()) > MAX_ARCHIVE_BYTES
        {
            return Err("deck archive entry exceeds limit".into());
        }
        let mut bytes = Vec::new();
        entry
            .take((limit + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > limit {
            return Err("deck entry expanded beyond limit".into());
        }
        expanded = expanded.saturating_add(bytes.len() as u64);
        if expanded > MAX_ARCHIVE_BYTES {
            return Err("deck contents exceed the 64 MiB expansion limit".into());
        }
        if name == "manifest.json" {
            manifest = Some(bytes);
        } else if let Some(hash) = name
            .strip_prefix("assets/")
            .and_then(|rest| rest.strip_suffix(".bin"))
        {
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return Err("unsafe deck asset name".into());
            }
            if digest_hex(&bytes) != hash {
                return Err("image asset hash mismatch".into());
            }
            assets.insert(hash.to_string(), bytes);
        } else {
            return Err("unsafe deck archive entry".into());
        }
    }
    let manifest: Manifest = serde_json::from_slice(&manifest.ok_or("missing deck manifest")?)
        .map_err(|e| format!("invalid deck manifest: {e}"))?;
    if manifest.version != 1 {
        return Err("unsupported deck version".into());
    }
    if manifest.assets != assets.keys().cloned().collect::<Vec<_>>() {
        return Err("deck asset inventory mismatch".into());
    }
    let deck = Deck {
        data: manifest.current,
        assets,
        history: manifest.history,
        cursor: manifest.cursor,
        disk_baseline: Some((target_path(path)?, baseline)),
        recovery_backup: None,
    };
    validate(&deck)?;
    if file_digest(path)? != baseline {
        return Err("deck changed while opening; reopen the latest file".into());
    }
    Ok(deck)
}

pub fn previous_version(path: &Path) -> std::path::PathBuf {
    path.with_extension("sstrike.previous")
}

fn validate(deck: &Deck) -> Result<(), String> {
    if deck.assets.len() > MAX_ASSETS
        || deck.history.is_empty()
        || deck.history.len() > MAX_HISTORY
        || deck.cursor >= deck.history.len()
    {
        return Err("deck asset or undo history limit exceeded".into());
    }
    for (hash, bytes) in &deck.assets {
        if bytes.is_empty() || bytes.len() > MAX_ASSET_BYTES || digest_hex(bytes) != *hash {
            return Err("invalid deck asset hash or size".into());
        }
    }
    for state in &deck.history {
        validate_snapshot(state, &deck.assets)?;
    }
    validate_snapshot(&deck.data, &deck.assets)?;
    if deck.history[deck.cursor] != deck.data {
        return Err("deck history does not match current slides".into());
    }
    Ok(())
}
