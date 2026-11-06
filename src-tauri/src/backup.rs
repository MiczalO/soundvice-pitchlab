//! Kopia zapasowa: jeden plik ZIP z bazą (spójna migawka `VACUUM INTO`) i opcjonalnie
//! plikami audio. Klucze API nie są w bazie, więc nigdy nie trafiają do kopii.
//!
//! Przywracanie nie podmienia otwartej bazy w locie: kopia jest rozpakowywana do
//! `pending_restore/`, a przy następnym starcie (przed otwarciem bazy) podmieniana.
//! Poprzednia baza zostaje w `before_restore/`.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::db::{SCHEMA_VERSION, now_ms};
use crate::error::{AppError, AppResult};

pub const DB_NAME: &str = "pitchlab.sqlite3";
const PENDING: &str = "pending_restore";
const BEFORE: &str = "before_restore";
const MANIFEST: &str = "manifest.json";

#[derive(Serialize, Debug)]
pub struct ExportInfo {
    pub path: String,
    pub audio_files: usize,
    pub size: u64,
}

fn io_err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("Błąd pliku: {e}"))
}

fn zip_err(e: impl std::fmt::Display) -> AppError {
    AppError::Other(format!("Błąd archiwum ZIP: {e}"))
}

/// Zapisuje kopię do `dest`. `audio_dir` jest pomijany, gdy `include_audio` = false.
pub fn export(
    conn: &Connection,
    audio_dir: &Path,
    dest: &Path,
    include_audio: bool,
    app_version: &str,
) -> AppResult<ExportInfo> {
    // Nazwa unikalna także przy równoległych eksportach (czas + proces + licznik).
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let tmp = std::env::temp_dir().join(format!(
        "pitchlab-export-{}-{}-{}.sqlite3",
        now_ms(),
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = fs::remove_file(&tmp);
    conn.execute("VACUUM INTO ?1", [tmp.to_string_lossy().as_ref()])?;

    let result = (|| -> AppResult<ExportInfo> {
        let mut zip = ZipWriter::new(File::create(dest).map_err(io_err)?);
        let deflate = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file(DB_NAME, deflate).map_err(zip_err)?;
        io::copy(&mut File::open(&tmp).map_err(io_err)?, &mut zip).map_err(io_err)?;

        let mut audio_files = 0;
        if include_audio && audio_dir.is_dir() {
            // Audio jest już skompresowane (lub nie da się go sensownie spakować) — bez kompresji.
            let stored = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Stored)
                .large_file(true);
            for entry in fs::read_dir(audio_dir).map_err(io_err)? {
                let path = entry.map_err(io_err)?.path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if !path.is_file() || !safe_name(name) {
                    continue;
                }
                zip.start_file(format!("audio/{name}"), stored)
                    .map_err(zip_err)?;
                io::copy(&mut File::open(&path).map_err(io_err)?, &mut zip).map_err(io_err)?;
                audio_files += 1;
            }
        }
        let manifest = serde_json::json!({
            "app": "Soundvice pitchLAB",
            "version": app_version,
            "created_at": now_ms(),
            "schema": SCHEMA_VERSION,
            "includes_audio": include_audio,
            "audio_files": audio_files,
        });
        zip.start_file(MANIFEST, deflate).map_err(zip_err)?;
        io::Write::write_all(&mut zip, manifest.to_string().as_bytes()).map_err(io_err)?;
        zip.finish().map_err(zip_err)?;
        Ok(ExportInfo {
            path: dest.to_string_lossy().into_owned(),
            audio_files,
            size: fs::metadata(dest).map(|m| m.len()).unwrap_or(0),
        })
    })();
    let _ = fs::remove_file(&tmp);
    result
}

/// Nazwa pliku bez separatorów i kropek na początku (ochrona przed zip-slip).
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
        && name != ".."
}

/// Rozpakowuje kopię do `data_dir/pending_restore/` i sprawdza, że to poprawna baza pitchLAB.
/// Nic poza znanymi wpisami (baza, manifest, `audio/<plik>`) nie jest zapisywane.
pub fn stage_restore(zip_path: &Path, data_dir: &Path) -> AppResult<()> {
    let pending = data_dir.join(PENDING);
    let _ = fs::remove_dir_all(&pending);
    let result = (|| -> AppResult<()> {
        let mut archive = ZipArchive::new(File::open(zip_path).map_err(io_err)?)
            .map_err(|_| AppError::Other("To nie jest plik kopii pitchLAB (ZIP).".into()))?;
        fs::create_dir_all(pending.join("audio")).map_err(io_err)?;
        let mut has_db = false;
        for i in 0..archive.len() {
            let mut f = archive.by_index(i).map_err(zip_err)?;
            if f.is_dir() {
                continue;
            }
            let Ok(name) = f.name().map(|n| n.to_string()) else {
                continue; // nieczytelna nazwa wpisu — pomijamy
            };
            let target: Option<PathBuf> = if name == DB_NAME {
                has_db = true;
                Some(pending.join(DB_NAME))
            } else if let Some(file) = name.strip_prefix("audio/") {
                safe_name(file).then(|| pending.join("audio").join(file))
            } else {
                None // manifest i wszystko nieznane pomijamy
            };
            if let Some(t) = target {
                io::copy(&mut f, &mut File::create(t).map_err(io_err)?).map_err(io_err)?;
            }
        }
        if !has_db {
            return Err(AppError::Other(
                "W kopii nie ma bazy danych — to nie jest kopia pitchLAB.".into(),
            ));
        }
        validate_db(&pending.join(DB_NAME))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&pending);
    }
    result
}

fn validate_db(path: &Path) -> AppResult<()> {
    let bad =
        || AppError::Other("Baza w kopii jest uszkodzona lub nie pochodzi z pitchLAB.".into());
    let c = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|_| bad())?;
    let tables: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'
             AND name IN ('artists','tracks','pitches','rules','settings')",
            [],
            |r| r.get(0),
        )
        .map_err(|_| bad())?;
    if tables != 5 {
        return Err(bad());
    }
    let version: u32 = c
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|_| bad())?;
    if version > SCHEMA_VERSION {
        return Err(AppError::Other(
            "Ta kopia pochodzi z nowszej wersji pitchLAB. Zaktualizuj aplikację i spróbuj ponownie."
                .into(),
        ));
    }
    Ok(())
}

/// Wywoływane na starcie PRZED otwarciem bazy. Zwraca `true`, jeśli podmieniono bazę.
pub fn apply_pending(data_dir: &Path) -> AppResult<bool> {
    let pending = data_dir.join(PENDING);
    let new_db = pending.join(DB_NAME);
    if !new_db.is_file() {
        let _ = fs::remove_dir_all(&pending);
        return Ok(false);
    }
    let db = data_dir.join(DB_NAME);
    if db.is_file() {
        let before = data_dir.join(BEFORE);
        let _ = fs::remove_dir_all(&before);
        fs::create_dir_all(&before).map_err(io_err)?;
        fs::rename(&db, before.join(DB_NAME)).map_err(io_err)?;
    }
    for ext in ["-wal", "-shm"] {
        let _ = fs::remove_file(data_dir.join(format!("{DB_NAME}{ext}")));
    }
    fs::rename(&new_db, &db).map_err(io_err)?;

    let audio_src = pending.join("audio");
    if audio_src.is_dir() {
        let audio_dst = data_dir.join("audio");
        fs::create_dir_all(&audio_dst).map_err(io_err)?;
        for entry in fs::read_dir(&audio_src).map_err(io_err)? {
            let p = entry.map_err(io_err)?.path();
            let Some(name) = p.file_name() else { continue };
            let dst = audio_dst.join(name);
            if !dst.exists() {
                fs::rename(&p, &dst)
                    .or_else(|_| fs::copy(&p, &dst).map(|_| ()))
                    .map_err(io_err)?;
            }
        }
    }
    let _ = fs::remove_dir_all(&pending);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use std::io::Write;

    fn seeded(dir: &Path) -> db::Db {
        let d = db::open(&dir.join(DB_NAME)).unwrap();
        d.0.lock()
            .unwrap()
            .execute(
                "INSERT INTO artists (name, slug, created_at, updated_at) VALUES ('Mata','mata',1,1)",
                [],
            )
            .unwrap();
        d
    }

    fn names(zip: &Path) -> Vec<String> {
        let mut a = ZipArchive::new(File::open(zip).unwrap()).unwrap();
        (0..a.len())
            .map(|i| a.by_index(i).unwrap().name().unwrap().to_string())
            .collect()
    }

    #[test]
    fn export_and_restore_roundtrip_with_audio() {
        let src = tempfile::tempdir().unwrap();
        let d = seeded(src.path());
        fs::create_dir_all(src.path().join("audio")).unwrap();
        fs::write(src.path().join("audio/abc.wav"), b"RIFFdata").unwrap();
        let zip = src.path().join("kopia.zip");
        let info = export(
            &d.0.lock().unwrap(),
            &src.path().join("audio"),
            &zip,
            true,
            "0.1.0",
        )
        .unwrap();
        assert_eq!(info.audio_files, 1);
        let mut n = names(&zip);
        n.sort();
        assert_eq!(
            n,
            vec!["audio/abc.wav", "manifest.json", "pitchlab.sqlite3"]
        );

        // przywrócenie do świeżego katalogu danych
        let dst = tempfile::tempdir().unwrap();
        stage_restore(&zip, dst.path()).unwrap();
        assert!(apply_pending(dst.path()).unwrap());
        assert!(!dst.path().join(PENDING).exists());
        let restored = db::open(&dst.path().join(DB_NAME)).unwrap();
        let name: String = restored
            .0
            .lock()
            .unwrap()
            .query_row("SELECT name FROM artists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Mata");
        assert_eq!(
            fs::read(dst.path().join("audio/abc.wav")).unwrap(),
            b"RIFFdata"
        );
    }

    #[test]
    fn export_without_audio_has_no_audio_entries_and_keeps_old_db_on_restore() {
        let src = tempfile::tempdir().unwrap();
        let d = seeded(src.path());
        fs::create_dir_all(src.path().join("audio")).unwrap();
        fs::write(src.path().join("audio/a.wav"), b"x").unwrap();
        let zip = src.path().join("k.zip");
        let info = export(
            &d.0.lock().unwrap(),
            &src.path().join("audio"),
            &zip,
            false,
            "0.1.0",
        )
        .unwrap();
        assert_eq!(info.audio_files, 0);
        assert!(names(&zip).iter().all(|n| !n.starts_with("audio/")));

        let dst = tempfile::tempdir().unwrap();
        let old = seeded(dst.path());
        old.0
            .lock()
            .unwrap()
            .execute("UPDATE artists SET name='Stara'", [])
            .unwrap();
        drop(old);
        stage_restore(&zip, dst.path()).unwrap();
        apply_pending(dst.path()).unwrap();
        // poprzednia baza zachowana w before_restore/
        let before = db::open(&dst.path().join(BEFORE).join(DB_NAME)).unwrap();
        let n: String = before
            .0
            .lock()
            .unwrap()
            .query_row("SELECT name FROM artists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, "Stara");
    }

    #[test]
    fn rejects_non_zip_zip_without_db_and_foreign_database() {
        let dir = tempfile::tempdir().unwrap();
        let junk = dir.path().join("junk.zip");
        fs::write(&junk, b"to nie jest zip").unwrap();
        assert!(stage_restore(&junk, dir.path()).is_err());

        let no_db = dir.path().join("nodb.zip");
        let mut z = ZipWriter::new(File::create(&no_db).unwrap());
        z.start_file("notatki.txt", SimpleFileOptions::default())
            .unwrap();
        z.write_all(b"x").unwrap();
        z.finish().unwrap();
        assert!(stage_restore(&no_db, dir.path()).is_err());

        let foreign = dir.path().join("foreign.zip");
        let fdb = dir.path().join("f.sqlite3");
        Connection::open(&fdb)
            .unwrap()
            .execute("CREATE TABLE x (a)", [])
            .unwrap();
        let mut z = ZipWriter::new(File::create(&foreign).unwrap());
        z.start_file(DB_NAME, SimpleFileOptions::default()).unwrap();
        z.write_all(&fs::read(&fdb).unwrap()).unwrap();
        z.finish().unwrap();
        assert!(stage_restore(&foreign, dir.path()).is_err());
        assert!(
            !dir.path().join(PENDING).exists(),
            "nieudane przywracanie sprząta po sobie"
        );
    }

    #[test]
    fn rejects_database_from_newer_schema() {
        let src = tempfile::tempdir().unwrap();
        let d = seeded(src.path());
        d.0.lock()
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        let zip = src.path().join("k.zip");
        export(
            &d.0.lock().unwrap(),
            &src.path().join("audio"),
            &zip,
            false,
            "9.9.9",
        )
        .unwrap();
        let dst = tempfile::tempdir().unwrap();
        let e = stage_restore(&zip, dst.path()).unwrap_err();
        assert!(e.to_string().contains("nowszej wersji"), "{e}");
    }

    #[test]
    fn zip_slip_entries_are_ignored() {
        let src = tempfile::tempdir().unwrap();
        let d = seeded(src.path());
        let good = src.path().join("good.zip");
        export(
            &d.0.lock().unwrap(),
            &src.path().join("audio"),
            &good,
            false,
            "0.1.0",
        )
        .unwrap();
        // dopisz złośliwe wpisy do kopii poprawnej bazy
        let evil = src.path().join("evil.zip");
        let mut z = ZipWriter::new(File::create(&evil).unwrap());
        let o = SimpleFileOptions::default();
        z.start_file(DB_NAME, o).unwrap();
        z.write_all(
            &fs::read({
                let mut a = ZipArchive::new(File::open(&good).unwrap()).unwrap();
                let mut tmp = src.path().join("extracted.sqlite3");
                let mut f = File::create(&tmp).unwrap();
                io::copy(&mut a.by_name(DB_NAME).unwrap(), &mut f).unwrap();
                tmp.shrink_to_fit();
                tmp
            })
            .unwrap(),
        )
        .unwrap();
        for bad in [
            "../evil.txt",
            "audio/../../evil2.txt",
            "audio/..\\evil3.txt",
            "/abs.txt",
        ] {
            z.start_file(bad, o).unwrap();
            z.write_all(b"evil").unwrap();
        }
        z.finish().unwrap();

        let parent = tempfile::tempdir().unwrap();
        let data = parent.path().join("data");
        fs::create_dir_all(&data).unwrap();
        stage_restore(&evil, &data).unwrap();
        apply_pending(&data).unwrap();
        let mut all: Vec<String> = Vec::new();
        for e in walk(parent.path()) {
            all.push(e.file_name().unwrap().to_string_lossy().into_owned());
        }
        assert!(
            all.iter().all(|n| !n.starts_with("evil") && n != "abs.txt"),
            "{all:?}"
        );
    }

    fn walk(p: &Path) -> Vec<PathBuf> {
        let mut out = vec![];
        for e in fs::read_dir(p).unwrap() {
            let path = e.unwrap().path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
        out
    }
}
