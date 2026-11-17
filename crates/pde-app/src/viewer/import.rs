//! Pages d'autres PDF (P-03) : insérer les pages de fichiers PDF (fusionner,
//! c'est les insérer à la fin), et extraire des pages dans un nouveau PDF.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pde_core::document::PageIndex;
use pde_core::edit::{ImportId, ImportedPdf, PagePlan};
use pde_core::engine::{Edition, EngineError};

use super::edit::{invalid, view};
use super::pages::{PagesView, document_view};
use super::{MAX_FILE_BYTES, OpenDocument, Viewer, read_file, sha256_hex};
use crate::edit_view::SavedView;
use crate::error::{AppError, ErrorKind};
use crate::files::write_atomic;

/// Fichier d'un PDF inséré : la sauvegarde automatique le désigne (sans le
/// copier) et ne le reprend que s'il n'a pas changé.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportFile {
    pub path: PathBuf,
    /// SHA-256 du contenu inséré, en hexadécimal.
    pub digest: String,
}

/// PDF lu et décrit par le moteur, prêt à être inséré.
pub(super) struct ReadPdf {
    pub pdf: ImportedPdf,
    pub file: ImportFile,
}

impl Viewer {
    /// Insère, à la place `at`, toutes les pages des fichiers `paths`, dans
    /// l'ordre : une seule opération, annulable d'un coup. Un PDF protégé
    /// par mot de passe est refusé.
    pub fn insert_pdf_files(
        &self,
        id: u64,
        paths: &[PathBuf],
        at: PageIndex,
    ) -> Result<PagesView, AppError> {
        if paths.is_empty() {
            return Err(invalid("aucun fichier à insérer"));
        }
        // Lus et décrits hors du verrou des documents.
        let read = paths
            .iter()
            .map(|path| self.read_pdf(path))
            .collect::<Result<Vec<_>, _>>()?;
        self.with_document(id, |open, doc| {
            let pages: Vec<(ImportId, PageIndex)> = register(open, read)
                .into_iter()
                .flat_map(|(file, count)| (0..count).map(move |index| (file, index)))
                .collect();
            let change = open.edits.insert_imported(at, &pages).map_err(invalid)?;
            self.commit(open, doc, change)?;
            Ok(PagesView {
                document: document_view(open, doc),
                edit: view(open),
                selected: (at..at + pages.len()).collect(),
            })
        })
    }

    /// Lit un PDF à insérer et en demande la taille des pages au moteur.
    pub(super) fn read_pdf(&self, path: &Path) -> Result<ReadPdf, AppError> {
        let bytes = read_file(path, MAX_FILE_BYTES)?;
        let digest = sha256_hex(&bytes);
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let opened = self.engine.open(bytes.clone(), None).map_err(|e| match e {
            EngineError::PasswordRequired => AppError::new(
                ErrorKind::InvalidFile,
                format!(
                    "« {name} » est protégé par un mot de passe : il ne peut pas encore être inséré"
                ),
            ),
            other => AppError::from(other),
        })?;
        self.engine.close(opened.id);
        Ok(ReadPdf {
            pdf: ImportedPdf {
                bytes,
                page_sizes: opened.info.page_sizes,
            },
            file: ImportFile {
                path: path.to_path_buf(),
                digest,
            },
        })
    }

    /// Écrit dans `target` un nouveau PDF fait des pages `pages` (dans
    /// l'ordre du document), avec ce qui y a été ajouté ; le document ouvert
    /// ne change pas. Le contenu est relu avant d'écrire.
    pub fn export_pages(
        &self,
        id: u64,
        pages: &[PageIndex],
        target: &Path,
    ) -> Result<SavedView, AppError> {
        self.with_document(id, |open, doc| {
            let edition = extracted(open, pages)?;
            let count = edition.plan.len();
            let bytes = self.engine.export(doc, edition)?;
            let check = self.engine.open(bytes.clone(), open.password.clone())?;
            self.engine.close(check.id);
            if check.info.page_count() != count {
                return Err(AppError::new(
                    ErrorKind::Engine,
                    "le PDF extrait ne correspond pas : rien n'a été écrit",
                ));
            }
            write_atomic(target, &bytes)?;
            log::info!("{count} page(s) extraite(s) : {} Ko", bytes.len() / 1024);
            Ok(SavedView {
                file_name: target
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: target.display().to_string(),
            })
        })
    }
}

/// Le document réduit aux pages choisies : plan, éléments de ces pages
/// (renumérotés), images et PDF insérés.
fn extracted(open: &OpenDocument, pages: &[PageIndex]) -> Result<Edition, AppError> {
    let plan = open.edits.plan();
    let chosen: BTreeSet<PageIndex> = pages.iter().copied().collect();
    if chosen.is_empty() {
        return Err(invalid("aucune page choisie"));
    }
    if let Some(page) = chosen.iter().find(|&&p| p >= plan.len()) {
        return Err(invalid(format!("la page {} n'existe pas", page + 1)));
    }
    let order: Vec<PageIndex> = chosen.into_iter().collect();
    let elements = open
        .edits
        .elements()
        .filter_map(|(id, element)| {
            let at = order.iter().position(|&p| p == element.page())?;
            Some((id, element.clone().with_page(at)))
        })
        .collect();
    Ok(Edition {
        plan: PagePlan::new(order.iter().map(|&p| plan.pages()[p]).collect()),
        elements,
        images: open.images.clone(),
        imports: open.imports.clone(),
        fields: open.edits.fields().clone(),
        metadata: open.edits.metadata().cloned(),
        flatten: open.edits.flatten(),
    })
}

/// Enregistre les PDF lus dans le document, sous de nouveaux identifiants ;
/// renvoie chacun avec son nombre de pages.
pub(super) fn register(open: &mut OpenDocument, read: Vec<ReadPdf>) -> Vec<(ImportId, usize)> {
    read.into_iter()
        .map(|item| {
            let id = ImportId(open.imports.keys().next_back().map_or(1, |last| last.0 + 1));
            let count = item.pdf.page_sizes.len();
            register_as(open, id, item);
            (id, count)
        })
        .collect()
}

/// Enregistre un PDF lu sous l'identifiant `id` (reprise d'une sauvegarde
/// automatique : le plan relu le désigne ainsi).
pub(super) fn register_as(open: &mut OpenDocument, id: ImportId, item: ReadPdf) {
    open.edits.register_import(id, item.pdf.page_sizes.clone());
    open.imports.insert(id, Arc::new(item.pdf));
    open.import_files.insert(id, item.file);
}

#[cfg(test)]
mod tests {
    use super::super::testing::Session;
    use super::*;
    use crate::fake::{LOCKED_PDF, SIMPLE_PDF, pdf_with_pages};

    /// Fichier PDF de deux pages à côté du document de la session.
    fn annexe(s: &Session, name: &str, bytes: &[u8]) -> PathBuf {
        let path = s.dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn pages_of_several_files_are_inserted_in_one_operation() {
        let s = Session::new("insert-pdf");
        let a = annexe(&s, "a.pdf", SIMPLE_PDF);
        let b = annexe(&s, "b.pdf", &pdf_with_pages(3));
        let view = s.viewer.insert_pdf_files(s.id, &[a, b], 1).unwrap();
        assert_eq!(view.document.pages.len(), 7);
        assert_eq!(view.selected, [1, 2, 3, 4, 5]);
        assert!(view.edit.dirty && view.edit.layout == 1);
        // Une seule annulation.
        s.viewer.undo(s.id).unwrap();
        assert_eq!(s.viewer.document_view(s.id).unwrap().pages.len(), 2);
        // Rétablir : les PDF insérés sont toujours connus.
        s.viewer.redo(s.id).unwrap();
        assert_eq!(s.viewer.document_view(s.id).unwrap().pages.len(), 7);
        s.viewer.save(s.id, None).unwrap();
        assert_eq!(
            std::fs::read(s.dir.join("contrat.pdf")).unwrap(),
            pdf_with_pages(7)
        );
    }

    #[test]
    fn protected_missing_or_misplaced_files_are_refused() {
        let s = Session::new("insert-refused");
        let locked = annexe(&s, "secret.pdf", LOCKED_PDF);
        let error = s.viewer.insert_pdf_files(s.id, &[locked], 0).unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidFile);
        assert!(error.message.contains("mot de passe"), "{}", error.message);
        let missing = s
            .viewer
            .insert_pdf_files(s.id, &[s.dir.join("absent.pdf")], 0)
            .unwrap_err();
        assert_eq!(missing.kind, ErrorKind::FileNotFound);
        let a = annexe(&s, "a.pdf", SIMPLE_PDF);
        assert!(s.viewer.insert_pdf_files(s.id, &[a], 9).is_err());
        assert!(s.viewer.insert_pdf_files(s.id, &[], 0).is_err());
        assert!(!s.viewer.edit_view(s.id).unwrap().can_undo);
    }

    #[test]
    fn extracted_pages_are_written_and_the_document_is_unchanged() {
        let s = Session::new("extract");
        s.viewer.insert_blank_page(s.id, 2).unwrap();
        let target = s.dir.join("extrait.pdf");
        let saved = s.viewer.export_pages(s.id, &[2, 0, 2], &target).unwrap();
        assert_eq!(saved.file_name, "extrait.pdf");
        assert_eq!(std::fs::read(&target).unwrap(), pdf_with_pages(2));
        // Rien de nouveau dans l'historique.
        let edit = s.viewer.edit_view(s.id).unwrap();
        assert_eq!(s.viewer.document_view(s.id).unwrap().pages.len(), 3);
        assert!(edit.can_undo && edit.layout == 1);
        s.viewer.undo(s.id).unwrap();
        assert!(!s.viewer.edit_view(s.id).unwrap().can_undo);
        // Pages absentes ou aucune page : refusé, rien d'écrit.
        let other = s.dir.join("autre.pdf");
        assert!(s.viewer.export_pages(s.id, &[5], &other).is_err());
        assert!(s.viewer.export_pages(s.id, &[], &other).is_err());
        assert!(!other.exists());
    }
}
