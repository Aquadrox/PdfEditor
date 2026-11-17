//! Dossiers de session : `recovery/<session>/doc-<n>/`, avec le verrou
//! `session.lock` tenu par le lancement qui les écrit.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::manifest::{self, Recovered, Snapshot};
use crate::error::{AppError, ErrorKind};
use crate::lock;

const LOCK_FILE: &str = "session.lock";
static SESSIONS: AtomicU64 = AtomicU64::new(0);
const DOC_PREFIX: &str = "doc-";

/// Document récupérable, tel que l'interface le présente.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecoveryView {
    /// `<session>/doc-<n>` : à rendre tel quel pour récupérer ou ignorer.
    pub id: String,
    pub file_name: String,
    pub path: String,
    /// Secondes depuis 1970.
    pub saved_at: u64,
    pub elements: usize,
}

pub struct RecoveryStore {
    root: PathBuf,
    session: PathBuf,
    /// Verrou de notre session (relâché avant de l'effacer : Windows refuse
    /// d'effacer un fichier ouvert).
    own_lock: Mutex<Option<File>>,
    /// Sessions orphelines prises par ce lancement, verrou compris.
    claimed: Mutex<HashMap<String, File>>,
    /// Session effacée (fermeture normale) : plus rien n'y est écrit.
    closed: AtomicBool,
}

fn io_error(e: io::Error) -> AppError {
    AppError::new(ErrorKind::Io, format!("sauvegarde automatique : {e}"))
}

/// Nom de dossier sûr : lettres, chiffres et tirets seulement.
fn plain_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// `<session>/doc-<n>` → ses deux parties, vérifiées (aucun chemin ne sort
/// du dossier de récupération).
fn parse_id(id: &str) -> Result<(&str, &str), AppError> {
    let invalid = || AppError::new(ErrorKind::InvalidRequest, "récupération inconnue");
    let (session, doc) = id.split_once('/').ok_or_else(invalid)?;
    let valid = plain_name(session) && plain_name(doc) && doc.starts_with(DOC_PREFIX);
    if valid {
        Ok((session, doc))
    } else {
        Err(invalid())
    }
}

fn try_lock(path: &Path) -> io::Result<Option<File>> {
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(e)) => Err(e),
    }
}

impl RecoveryStore {
    /// Crée la session de ce lancement sous `root` et la verrouille.
    pub fn open(root: &Path) -> Result<Self, AppError> {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        // Compteur : deux sessions du même processus dans la même
        // milliseconde (tests) gardent des noms distincts.
        let n = SESSIONS.fetch_add(1, Ordering::Relaxed);
        let session = root.join(format!("{millis}-{}-{n}", std::process::id()));
        fs::create_dir_all(&session).map_err(io_error)?;
        let lock_file = try_lock(&session.join(LOCK_FILE))
            .map_err(io_error)?
            .ok_or_else(|| io_error(io::Error::other("session déjà verrouillée")))?;
        Ok(Self {
            root: root.to_path_buf(),
            session,
            own_lock: Mutex::new(Some(lock_file)),
            claimed: Mutex::new(HashMap::new()),
            closed: AtomicBool::new(false),
        })
    }

    fn doc_dir(&self, doc: u64) -> PathBuf {
        self.session.join(format!("{DOC_PREFIX}{doc}"))
    }

    pub fn write(&self, doc: u64, snapshot: &Snapshot, saved_at: u64) -> Result<(), AppError> {
        if self.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        manifest::write(&self.doc_dir(doc), snapshot, saved_at)
    }

    /// Document enregistré, fermé ou revenu à son état d'origine.
    pub fn remove(&self, doc: u64) {
        let _ = fs::remove_dir_all(self.doc_dir(doc));
    }

    /// Fermeture normale : la session disparaît.
    pub fn clear(&self) {
        self.closed.store(true, Ordering::SeqCst);
        drop(lock(&self.own_lock).take());
        let _ = fs::remove_dir_all(&self.session);
    }

    /// Documents des sessions terminées sans fermeture normale, les plus
    /// récents d'abord. Les sessions vides ou illisibles sont effacées.
    pub fn orphans(&self) -> Vec<RecoveryView> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for entry in entries.flatten() {
            let dir = entry.path();
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if dir == self.session
                || !dir.is_dir()
                || !plain_name(&name)
                || !self.claim(&name, &dir)
            {
                continue;
            }
            let documents = Self::documents(&name, &dir);
            if documents.is_empty() {
                self.drop_session(&name);
            }
            found.extend(documents);
        }
        found.sort_by_key(|v| std::cmp::Reverse(v.saved_at));
        found
    }

    /// Prend la session `name` si aucun lancement ne la tient.
    fn claim(&self, name: &str, dir: &Path) -> bool {
        let mut claimed = lock(&self.claimed);
        if claimed.contains_key(name) {
            return true;
        }
        match try_lock(&dir.join(LOCK_FILE)) {
            Ok(Some(file)) => {
                claimed.insert(name.to_owned(), file);
                true
            }
            _ => false,
        }
    }

    fn documents(session: &str, dir: &Path) -> Vec<RecoveryView> {
        let Ok(entries) = fs::read_dir(dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|entry| {
                let doc = entry.file_name().to_str()?.to_owned();
                if !doc.starts_with(DOC_PREFIX) {
                    return None;
                }
                let (file_name, path, saved_at, elements) = manifest::summary(&entry.path())?;
                Some(RecoveryView {
                    id: format!("{session}/{doc}"),
                    file_name,
                    path: path.display().to_string(),
                    saved_at,
                    elements,
                })
            })
            .collect()
    }

    fn claimed_dir(&self, id: &str) -> Result<PathBuf, AppError> {
        let (session, doc) = parse_id(id)?;
        if !lock(&self.claimed).contains_key(session) {
            return Err(AppError::new(
                ErrorKind::InvalidRequest,
                "récupération inconnue",
            ));
        }
        Ok(self.root.join(session).join(doc))
    }

    pub fn load(&self, id: &str) -> Result<Recovered, AppError> {
        manifest::read(&self.claimed_dir(id)?)
    }

    /// Oublie une récupération ; sa session disparaît avec son dernier document.
    pub fn discard(&self, id: &str) {
        let Ok(dir) = self.claimed_dir(id) else {
            return;
        };
        let _ = fs::remove_dir_all(&dir);
        if let Ok((session, _)) = parse_id(id)
            && Self::documents(session, &self.root.join(session)).is_empty()
        {
            self.drop_session(session);
        }
    }

    fn drop_session(&self, name: &str) {
        drop(lock(&self.claimed).remove(name));
        let _ = fs::remove_dir_all(self.root.join(name));
    }
}

#[cfg(test)]
mod tests {
    use pde_core::edit::{Element, Images, TextBox, TextStyle};
    use pde_core::geometry::Point;

    use super::*;

    fn root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pde-recovery-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn snapshot(text: &str) -> Snapshot {
        Snapshot {
            path: PathBuf::from("/docs/contrat.pdf"),
            file_name: "contrat.pdf".into(),
            digest: "00".repeat(32),
            page_count: 1,
            pages: None,
            elements: vec![Element::Text(TextBox {
                page: 0,
                top_left: Point::new(72.0, 700.0),
                width: 200.0,
                text: text.into(),
                style: TextStyle::default(),
            })],
            images: Images::new(),
        }
    }

    #[test]
    fn a_running_session_is_never_offered() {
        let dir = root("vivante");
        let running = RecoveryStore::open(&dir).unwrap();
        running.write(1, &snapshot("a"), 10).unwrap();
        let other = RecoveryStore::open(&dir).unwrap();
        assert!(other.orphans().is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn crashed_session_is_offered_then_restored_or_discarded() {
        let dir = root("plantee");
        let crashed = RecoveryStore::open(&dir).unwrap();
        crashed.write(1, &snapshot("premier"), 10).unwrap();
        crashed.write(2, &snapshot("second"), 20).unwrap();
        // Plantage : le verrou disparaît avec le processus, rien n'est effacé.
        drop(crashed);

        let next = RecoveryStore::open(&dir).unwrap();
        let offered = next.orphans();
        assert_eq!(offered.len(), 2);
        assert_eq!(offered[0].saved_at, 20, "plus récent d'abord");
        assert_eq!(offered[0].file_name, "contrat.pdf");
        let Element::Text(text) = &next.load(&offered[0].id).unwrap().elements[0] else {
            panic!("zone de texte attendue");
        };
        assert_eq!(text.text, "second");

        next.discard(&offered[0].id);
        assert_eq!(next.orphans().len(), 1);
        next.discard(&offered[1].id);
        assert!(next.orphans().is_empty());
        // Session orpheline effacée avec son dernier document.
        let sessions = fs::read_dir(&dir).unwrap().count();
        assert_eq!(sessions, 1, "seule la session en cours reste");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn normal_quit_leaves_nothing_to_recover() {
        let dir = root("fermee");
        let first = RecoveryStore::open(&dir).unwrap();
        first.write(1, &snapshot("a"), 10).unwrap();
        first.remove(1);
        first.write(2, &snapshot("b"), 10).unwrap();
        first.clear();
        // Une écriture tardive (minuterie) ne recrée rien.
        first.write(3, &snapshot("c"), 10).unwrap();
        let next = RecoveryStore::open(&dir).unwrap();
        assert!(next.orphans().is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn ids_cannot_leave_the_recovery_folder() {
        let dir = root("chemins");
        let store = RecoveryStore::open(&dir).unwrap();
        for id in [
            "../x/doc-1",
            "a/../../doc-1",
            "a/b",
            "a",
            "",
            "a/doc-1/..",
            "a\\b/doc-1",
        ] {
            assert!(store.load(id).is_err(), "{id}");
        }
        // Session non prise par ce lancement : refusée.
        assert!(store.load("123-4/doc-1").is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
