//! Copies locales des documents ouverts par une adresse du système (A-02,
//! Android : `content://…` du sélecteur de documents, « Ouvrir avec »,
//! « Partager ») : le moteur travaille sur un fichier, rangé dans le dossier
//! de l'application ; son origine est gardée pour y réécrire le document à
//! l'enregistrement. Ce module ne lit ni n'écrit l'adresse elle-même :
//! c'est le rôle de la plateforme.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::{AppError, ErrorKind};
use crate::files::write_atomic;
use crate::lock;

/// Plus long nom de fichier gardé (sans l'extension).
const MAX_NAME: usize = 80;

pub struct LocalCopies {
    dir: PathBuf,
    /// Copie locale → adresse d'origine (où réécrire le document).
    origins: Mutex<HashMap<PathBuf, String>>,
}

/// Nom de fichier lisible tiré d'une adresse (`content://…/Download%2Fbail.pdf`
/// → `bail.pdf`) ; « document.pdf » à défaut.
pub fn name_from_uri(uri: &str) -> String {
    let last = uri.rsplit(['/', ':']).next().unwrap_or_default();
    let decoded = percent_decode(last);
    let last = decoded.rsplit(['/', ':']).next().unwrap_or_default();
    let name = sanitize(last);
    if name.is_empty() {
        "document.pdf".into()
    } else {
        name
    }
}

/// Décode les « %XX » d'une adresse (octets UTF-8) ; le reste est gardé.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && let (Some(hi), Some(lo)) = (
                bytes.get(i + 1).copied().and_then(hex),
                bytes.get(i + 2).copied().and_then(hex),
            )
        {
            out.push((hi * 16 + lo) as u8);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Nom de fichier sûr : sans séparateurs ni caractères réservés, borné,
/// terminé par « .pdf ».
fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.');
    let stem = cleaned
        .strip_suffix(".pdf")
        .or_else(|| cleaned.strip_suffix(".PDF"))
        .unwrap_or(cleaned);
    let stem: String = stem.chars().take(MAX_NAME).collect();
    if stem.trim().is_empty() {
        String::new()
    } else {
        format!("{}.pdf", stem.trim())
    }
}

impl LocalCopies {
    /// Copies rangées dans `dir` (créé au besoin).
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            origins: Mutex::new(HashMap::new()),
        }
    }

    /// Range une copie de `bytes` sous un nom tiré de `name` (« -2 »,
    /// « -3 »… s'il est pris) et retient son origine.
    pub fn store(
        &self,
        name: &str,
        bytes: &[u8],
        origin: Option<&str>,
    ) -> Result<PathBuf, AppError> {
        let path = self.free_path(name);
        write_atomic(&path, bytes)?;
        if let Some(origin) = origin {
            self.set_origin(&path, origin);
        }
        Ok(path)
    }

    /// Chemin libre pour une nouvelle copie nommée d'après `name`.
    pub fn free_path(&self, name: &str) -> PathBuf {
        let name = match sanitize(name) {
            n if n.is_empty() => "document.pdf".to_owned(),
            n => n,
        };
        let stem = name.trim_end_matches(".pdf").to_owned();
        let mut path = self.dir.join(&name);
        let mut n = 2;
        while path.exists() {
            path = self.dir.join(format!("{stem}-{n}.pdf"));
            n += 1;
        }
        path
    }

    /// La copie `path` s'enregistre aussi dans `origin`.
    pub fn set_origin(&self, path: &Path, origin: &str) {
        lock(&self.origins).insert(path.to_path_buf(), origin.to_owned());
    }

    /// Adresse d'origine de la copie `path`, s'il y en a une.
    pub fn origin(&self, path: &Path) -> Option<String> {
        lock(&self.origins).get(path).cloned()
    }

    /// Contenu de la copie, pour le réécrire dans son origine.
    pub fn read(&self, path: &Path) -> Result<Vec<u8>, AppError> {
        std::fs::read(path).map_err(|e| AppError::new(ErrorKind::Io, e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_come_from_the_address() {
        assert_eq!(
            name_from_uri(
                "content://com.android.externalstorage.documents/document/primary%3ADownload%2FBail%20sign%C3%A9.pdf"
            ),
            "Bail signé.pdf"
        );
        assert_eq!(
            name_from_uri("content://media/external/file/1234"),
            "1234.pdf"
        );
        assert_eq!(name_from_uri("content://x/"), "document.pdf");
        assert_eq!(
            name_from_uri("content://x/..%2F..%2Fetc%2Fpasswd"),
            "passwd.pdf"
        );
        assert_eq!(name_from_uri("content://x/a%3Cb%3E.PDF"), "a_b_.pdf");
    }

    #[test]
    fn copies_never_overwrite_each_other_and_keep_their_origin() {
        let dir = std::env::temp_dir().join(format!("pde-copies-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let copies = LocalCopies::new(&dir);
        let first = copies
            .store("bail.pdf", b"%PDF-1", Some("content://a"))
            .unwrap();
        let second = copies.store("bail.pdf", b"%PDF-2", None).unwrap();
        assert_eq!(first.file_name().unwrap(), "bail.pdf");
        assert_eq!(second.file_name().unwrap(), "bail-2.pdf");
        assert_eq!(copies.origin(&first).as_deref(), Some("content://a"));
        assert_eq!(copies.origin(&second), None);
        assert_eq!(copies.read(&first).unwrap(), b"%PDF-1");
        copies.set_origin(&second, "content://b");
        assert_eq!(copies.origin(&second).as_deref(), Some("content://b"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
