//! Emplacement de la bibliothèque PDFium (`pdfium.dll`, `libpdfium.so`).

use std::path::{Path, PathBuf};

/// Variable d'environnement qui impose le dossier de PDFium.
pub const LIBRARY_DIR_ENV: &str = "PDE_PDFIUM_DIR";

/// Dossiers où chercher PDFium, dans l'ordre :
/// 1. celui de la variable [`LIBRARY_DIR_ENV`] ;
/// 2. celui de l'exécutable (application installée) ;
/// 3. en développement seulement, `vendor/pdfium` du dépôt
///    (rempli par `scripts/fetch-pdfium.sh`).
pub fn library_candidates(env_dir: Option<&Path>, exe_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    dirs.extend(env_dir.map(Path::to_path_buf));
    dirs.extend(exe_dir.map(Path::to_path_buf));
    #[cfg(debug_assertions)]
    dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/pdfium"));
    dirs
}

/// Dossiers candidats pour le processus en cours.
#[cfg(not(target_os = "android"))]
pub(crate) fn current_candidates() -> Vec<PathBuf> {
    let env_dir = std::env::var_os(LIBRARY_DIR_ENV).map(PathBuf::from);
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    library_candidates(env_dir.as_deref(), exe_dir.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_comes_first_then_executable() {
        let dirs = library_candidates(Some(Path::new("/env")), Some(Path::new("/exe")));
        assert_eq!(dirs[0], Path::new("/env"));
        assert_eq!(dirs[1], Path::new("/exe"));
    }

    #[test]
    fn missing_sources_are_skipped() {
        let dirs = library_candidates(None, Some(Path::new("/exe")));
        assert_eq!(dirs[0], Path::new("/exe"));
    }

    #[test]
    fn development_build_looks_in_vendor() {
        let dirs = library_candidates(None, None);
        assert!(dirs.iter().any(|d| d.ends_with("vendor/pdfium")));
    }
}
