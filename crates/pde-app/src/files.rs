//! Écriture sûre des fichiers de l'application : un fichier n'est jamais
//! laissé à moitié écrit (coupure, disque plein).

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use crate::error::{AppError, ErrorKind};

/// Écrit dans un fichier temporaire voisin, le force sur le disque, puis le
/// renomme à la place de `path` (opération atomique).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let failed =
        |e: std::io::Error| AppError::new(ErrorKind::Io, format!("{} : {e}", path.display()));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(failed)?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = Path::new(&temporary);
    let mut file = File::create(temporary).map_err(failed)?;
    file.write_all(bytes).map_err(failed)?;
    file.sync_all().map_err(failed)?;
    drop(file);
    fs::rename(temporary, path).map_err(failed)
}

/// Nom proposé pour une copie : « contrat.pdf » + « (modifié) » →
/// « contrat (modifié).pdf ». Une copie de copie ne reprend pas le suffixe.
pub fn copy_name(file_name: &str, suffix: &str) -> String {
    let stem = match file_name.rsplit_once('.') {
        Some((stem, ext)) if ext.eq_ignore_ascii_case("pdf") && !stem.is_empty() => stem,
        _ => file_name,
    };
    if suffix.is_empty() || stem.ends_with(suffix) {
        format!("{stem}.pdf")
    } else {
        format!("{stem}{suffix}.pdf")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_names() {
        assert_eq!(
            copy_name("contrat.pdf", " (modifié)"),
            "contrat (modifié).pdf"
        );
        assert_eq!(copy_name("Contrat.PDF", "-v2"), "Contrat-v2.pdf");
        assert_eq!(
            copy_name("contrat (modifié).pdf", " (modifié)"),
            "contrat (modifié).pdf"
        );
        assert_eq!(copy_name("devis.2026.pdf", ""), "devis.2026.pdf");
        assert_eq!(
            copy_name("sans-extension", " copie"),
            "sans-extension copie.pdf"
        );
        assert_eq!(copy_name(".pdf", " copie"), ".pdf copie.pdf");
    }

    #[test]
    fn replaces_the_file_and_leaves_no_temporary() {
        let dir = std::env::temp_dir().join(format!("pde-files-{}", std::process::id()));
        let path = dir.join("sous-dossier/a.txt");
        write_atomic(&path, b"un").unwrap();
        write_atomic(&path, b"deux").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"deux");
        assert!(!dir.join("sous-dossier/a.txt.tmp").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
