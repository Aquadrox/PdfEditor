//! Port vers les convertisseurs de documents (E-13) : un document Word ou
//! OpenDocument devient un PDF. Les implémentations (LibreOffice, Microsoft
//! Word) vivent dans `pde-platform` ; le reste de l'application ne sait pas
//! lequel a servi.

use std::path::{Path, PathBuf};

/// Extensions des documents qu'on peut importer.
pub const IMPORTABLE: &[&str] = &["docx", "doc", "odt", "rtf"];

/// Convertisseur d'un document de bureautique en PDF.
pub trait DocumentConverter: Send + Sync {
    /// Nom montré à l'utilisateur (« Microsoft Word », « LibreOffice »).
    fn name(&self) -> &str;

    /// Écrit le PDF de `input` dans le dossier `out_dir` et renvoie son
    /// chemin ; l'erreur est formulée pour l'utilisateur.
    fn convert(&self, input: &Path, out_dir: &Path) -> Result<PathBuf, String>;
}

/// Extension en minuscules d'un fichier importable, sinon `None`.
pub fn importable_extension(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    IMPORTABLE.contains(&ext.as_str()).then_some(ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn importable_files_are_recognized_by_extension() {
        assert_eq!(
            importable_extension(Path::new("a/Bail.DOCX")).as_deref(),
            Some("docx")
        );
        assert_eq!(
            importable_extension(Path::new("a.odt")).as_deref(),
            Some("odt")
        );
        assert_eq!(importable_extension(Path::new("a.pdf")), None);
        assert_eq!(importable_extension(Path::new("docx")), None);
    }
}
