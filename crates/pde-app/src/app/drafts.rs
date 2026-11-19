//! Nouveau document et import d'un document Word ou OpenDocument (E-13) :
//! le PDF est rangé parmi les brouillons et ouvert sans titre. L'import
//! passe par Microsoft Word, sinon LibreOffice ; un `.docx` sans l'un ni
//! l'autre est repris par l'import intégré (mise en page simplifiée).

use std::path::Path;

use pde_core::convert::importable_extension;
use serde::Serialize;

use super::App;
use crate::drafts::{NewDocument, blank_pdf, pdf_name};
use crate::error::{AppError, ErrorKind};
use crate::view::DocumentView;

/// Document importé et la façon dont il l'a été.
#[derive(Debug, Clone, Serialize)]
pub struct ImportedView {
    pub document: DocumentView,
    /// Logiciel qui a converti (« LibreOffice », « Microsoft Word »), ou
    /// `None` pour l'import intégré.
    pub converter: Option<String>,
    /// Ce qui n'a pas été repris (import intégré), à dire à l'utilisateur.
    pub notice: Option<String>,
}

fn io(message: impl Into<String>) -> AppError {
    AppError::new(ErrorKind::Io, message)
}

impl App {
    fn drafts(&self) -> Result<&crate::drafts::Drafts, AppError> {
        self.drafts
            .as_ref()
            .ok_or_else(|| io("dossier des brouillons indisponible"))
    }

    /// Document vierge, ouvert sans titre.
    pub fn new_document(&self, request: &NewDocument) -> Result<DocumentView, AppError> {
        if request.pages == 0 || request.pages > crate::drafts::MAX_NEW_PAGES {
            return Err(AppError::new(
                ErrorKind::InvalidRequest,
                format!("de 1 à {} pages", crate::drafts::MAX_NEW_PAGES),
            ));
        }
        let bytes = blank_pdf(request.page_size(), request.pages);
        let path = self.drafts()?.store("Sans titre.pdf", &bytes)?;
        self.viewer.open_draft(&path)
    }

    /// Document Word, OpenDocument ou RTF converti en PDF, ouvert sans titre.
    pub fn import_document(&self, path: &Path) -> Result<ImportedView, AppError> {
        let ext = importable_extension(path).ok_or_else(|| {
            AppError::new(
                ErrorKind::InvalidRequest,
                "formats importables : .docx, .doc, .odt, .rtf",
            )
        })?;
        let drafts = self.drafts()?;
        let work = std::env::temp_dir().join(format!(
            "pde-import-{}-{}",
            std::process::id(),
            super::now()
        ));
        let mut failures = Vec::new();
        for converter in &self.converters {
            std::fs::create_dir_all(&work).map_err(|e| io(e.to_string()))?;
            let result = converter
                .convert(path, &work)
                .and_then(|pdf| std::fs::read(pdf).map_err(|e| e.to_string()));
            let _ = std::fs::remove_dir_all(&work);
            match result {
                Ok(bytes) => {
                    log::info!(
                        "document importé par {} : {} Ko",
                        converter.name(),
                        bytes.len() / 1024
                    );
                    let stored = drafts.store(&pdf_name(path), &bytes)?;
                    return Ok(ImportedView {
                        document: self.viewer.open_draft(&stored)?,
                        converter: Some(converter.name().to_string()),
                        notice: None,
                    });
                }
                Err(e) => {
                    log::warn!("conversion par {} refusée : {e}", converter.name());
                    failures.push(format!("{} : {e}", converter.name()));
                }
            }
        }
        if ext != "docx" {
            let mut message = format!(
                "pour importer un fichier .{ext}, installez LibreOffice (gratuit) ou Microsoft Word"
            );
            if !failures.is_empty() {
                message = format!("conversion impossible ({})", failures.join(" ; "));
            }
            return Err(AppError::new(ErrorKind::InvalidRequest, message));
        }
        self.import_builtin(path)
    }

    /// Import intégré d'un `.docx` : texte, titres, listes, tableaux.
    fn import_builtin(&self, path: &Path) -> Result<ImportedView, AppError> {
        let bytes = crate::viewer::read_file(path, pde_docx::MAX_DOCX_BYTES as u64)?;
        let doc =
            pde_docx::read(&bytes).map_err(|e| AppError::new(ErrorKind::InvalidRequest, e))?;
        let laid = pde_docx::layout(&doc);
        let pages = u32::try_from(laid.pages).unwrap_or(u32::MAX);
        if pages > crate::drafts::MAX_IMPORT_PAGES {
            return Err(AppError::new(
                ErrorKind::InvalidRequest,
                "document trop long pour l'import intégré",
            ));
        }
        let pdf = self
            .viewer
            .build_pdf(blank_pdf(laid.page_size, pages), laid.elements)?;
        let stored = self.drafts()?.store(&pdf_name(path), &pdf)?;
        let mut notice = "Mise en page simplifiée : installez LibreOffice ou Microsoft Word pour une conversion fidèle.".to_string();
        if doc.skipped_images > 0 {
            notice.push_str(&format!(" {} image(s) non reprise(s).", doc.skipped_images));
        }
        Ok(ImportedView {
            document: self.viewer.open_draft(&stored)?,
            converter: None,
            notice: Some(notice),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use pde_core::convert::DocumentConverter;

    use super::*;
    use crate::drafts::{Drafts, Paper};
    use crate::fake::{FakeEngine, SIMPLE_PDF};
    use crate::recent::RecentFiles;
    use crate::settings_store::SettingsStore;

    /// Convertisseur simulé : réussit (écrit un PDF) ou échoue.
    struct FakeConverter(bool);

    impl DocumentConverter for FakeConverter {
        fn name(&self) -> &str {
            "Faux traitement de texte"
        }

        fn convert(&self, input: &Path, out_dir: &Path) -> Result<PathBuf, String> {
            if !self.0 {
                return Err("en panne".into());
            }
            let pdf = out_dir
                .join(input.file_stem().unwrap())
                .with_extension("pdf");
            std::fs::write(&pdf, SIMPLE_PDF).unwrap();
            Ok(pdf)
        }
    }

    fn app(name: &str, converters: Vec<Arc<dyn DocumentConverter>>) -> (App, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("pde-drafts-app-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let app = App::new(
            Arc::new(FakeEngine::default()),
            SettingsStore::load(dir.join("config.toml")),
            RecentFiles::load(dir.join("recent.json")),
        )
        .with_drafts(Drafts::new(&dir.join("brouillons")))
        .with_converters(converters);
        (app, dir)
    }

    #[test]
    fn a_new_document_is_untitled_until_saved_where_chosen() {
        let (app, dir) = app("nouveau", Vec::new());
        let request = NewDocument {
            paper: Paper::A4,
            landscape: false,
            pages: 2,
        };
        let view = app.new_document(&request).unwrap();
        assert!(view.untitled);
        assert_eq!(view.file_name, "Sans titre.pdf");
        let draft = app.viewer.document_path(view.id).unwrap();
        assert!(draft.exists());
        // « Enregistrer » sans fichier : refusé, il faut choisir où.
        assert_eq!(
            app.save_document(view.id, None).unwrap_err().kind,
            ErrorKind::InvalidRequest
        );
        let (_, name) = app.save_as_proposal(view.id).unwrap();
        assert_eq!(name, "Sans titre.pdf");
        let target = dir.join("formulaire.pdf");
        app.save_document(view.id, Some(target.clone())).unwrap();
        assert!(target.exists() && !draft.exists());
        assert!(!app.viewer.is_untitled(view.id));
        assert!(
            app.new_document(&NewDocument {
                pages: 0,
                ..request
            })
            .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_closed_draft_is_removed() {
        let (app, dir) = app("fermeture", Vec::new());
        let view = app
            .new_document(&NewDocument {
                paper: Paper::Letter,
                landscape: true,
                pages: 1,
            })
            .unwrap();
        let draft = app.viewer.document_path(view.id).unwrap();
        app.close_document(view.id);
        assert!(!draft.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn word_documents_go_through_the_first_working_converter() {
        let (app, dir) = app(
            "import",
            vec![
                Arc::new(FakeConverter(false)),
                Arc::new(FakeConverter(true)),
            ],
        );
        let source = dir.join("bail.odt");
        std::fs::write(&source, b"contenu").unwrap();
        let imported = app.import_document(&source).unwrap();
        assert_eq!(
            imported.converter.as_deref(),
            Some("Faux traitement de texte")
        );
        assert_eq!(imported.document.file_name, "bail.pdf");
        assert!(imported.document.untitled && imported.notice.is_none());
        assert!(app.import_document(&dir.join("x.pdf")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_converter_only_docx_is_imported_by_pdfeditor() {
        let (app, dir) = app("integre", Vec::new());
        let odt = dir.join("lettre.odt");
        std::fs::write(&odt, b"contenu").unwrap();
        let error = app.import_document(&odt).unwrap_err();
        assert!(error.message.contains("LibreOffice"), "{error:?}");
        let docx = dir.join("formulaire.docx");
        std::fs::write(
            &docx,
            include_bytes!("../../../pde-docx/tests/fixtures/formulaire.docx"),
        )
        .unwrap();
        let imported = app.import_document(&docx).unwrap();
        assert_eq!(imported.converter, None);
        assert!(imported.notice.unwrap().contains("simplifiée"));
        assert_eq!(imported.document.file_name, "formulaire.pdf");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
