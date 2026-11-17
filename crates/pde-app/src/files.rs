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

/// Nom du fichier sans l'extension « .pdf ».
fn pdf_stem(file_name: &str) -> &str {
    match file_name.rsplit_once('.') {
        Some((stem, ext)) if ext.eq_ignore_ascii_case("pdf") && !stem.is_empty() => stem,
        _ => file_name,
    }
}

/// Nom proposé pour une copie : « contrat.pdf » + « (modifié) » →
/// « contrat (modifié).pdf ». Une copie de copie ne reprend pas le suffixe.
pub fn copy_name(file_name: &str, suffix: &str) -> String {
    let stem = pdf_stem(file_name);
    if suffix.is_empty() || stem.ends_with(suffix) {
        format!("{stem}.pdf")
    } else {
        format!("{stem}{suffix}.pdf")
    }
}

/// Nom proposé pour des pages extraites (numéros à partir de 0) :
/// « contrat (page 2).pdf », « contrat (pages 1-3, 5).pdf ».
pub fn extract_name(file_name: &str, pages: &[usize]) -> String {
    let mut pages = pages.to_vec();
    pages.sort_unstable();
    pages.dedup();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for page in pages.iter().map(|p| p + 1) {
        match ranges.last_mut() {
            Some((_, end)) if *end + 1 == page => *end = page,
            _ => ranges.push((page, page)),
        }
    }
    let list = ranges
        .iter()
        .map(|&(start, end)| {
            if start == end {
                start.to_string()
            } else {
                format!("{start}-{end}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let label = if pages.len() == 1 { "page" } else { "pages" };
    format!("{} ({label} {list}).pdf", pdf_stem(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_names() {
        assert_eq!(extract_name("contrat.pdf", &[1]), "contrat (page 2).pdf");
        assert_eq!(
            extract_name("contrat.pdf", &[4, 0, 1, 2, 2]),
            "contrat (pages 1-3, 5).pdf"
        );
        assert_eq!(extract_name("scan", &[0, 2]), "scan (pages 1, 3).pdf");
    }

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
