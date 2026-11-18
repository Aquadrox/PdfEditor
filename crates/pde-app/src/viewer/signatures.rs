//! Signatures du document ouvert (S-07) et signature (S-05, S-06).
//!
//! On signe le **fichier enregistré** : le document ne doit pas avoir de
//! modifications en attente, et le fichier sur le disque doit être celui
//! que l'application a ouvert ou enregistré. La signature est ajoutée par
//! une mise à jour incrémentale, relue par notre vérificateur et par le
//! moteur, puis le fichier est remplacé de façon atomique.

use pde_sign::{Identity, SignOptions, SignatureField, SignatureStatus};

use super::{Viewer, read_file, sha256_hex};
use crate::edit_view::SavedView;
use crate::error::{AppError, ErrorKind};
use crate::files::write_atomic;
use crate::signing::identities::sign_error;
use crate::signing::{SignRequest, SignatureView};

/// Au-delà de cette taille, les signatures ne sont cherchées que si le
/// fichier en contient visiblement une (lecture complète de la structure).
const QUICK_SCAN_BYTES: usize = 64 * 1024 * 1024;

/// Champs de signature d'un fichier ; un PDF illisible pour `lopdf` (mais
/// ouvert par PDFium) n'en a simplement pas.
pub(super) fn read_signatures(bytes: &[u8]) -> Vec<SignatureField> {
    if bytes.len() > QUICK_SCAN_BYTES && !contains(bytes, b"/ByteRange") {
        return Vec::new();
    }
    pde_sign::signatures(bytes).unwrap_or_else(|e| {
        log::info!("signatures non lues : {e}");
        Vec::new()
    })
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

impl Viewer {
    /// Champs de signature du document, signés ou vides.
    pub fn signatures(&self, id: u64) -> Result<Vec<SignatureView>, AppError> {
        self.with_document(id, |open, _| {
            Ok(open
                .signatures
                .iter()
                .map(|field| SignatureView::new(field, |page| open.info.page_size(page).ok()))
                .collect())
        })
    }

    /// Signe le fichier du document avec `identity` ; `now` : secondes
    /// depuis 1970. Le document affiché reste celui d'avant : l'interface le
    /// rouvre pour montrer le fichier signé.
    pub fn sign(
        &self,
        id: u64,
        identity: &Identity,
        request: &SignRequest,
        now: u64,
    ) -> Result<SavedView, AppError> {
        self.with_document(id, |open, _| {
            if open.edits.is_dirty() {
                return Err(AppError::new(
                    ErrorKind::Unsaved,
                    "enregistrez le document avant de le signer",
                ));
            }
            if open.password.is_some() {
                return Err(AppError::new(
                    ErrorKind::Signature,
                    "la signature d'un document protégé par mot de passe n'est pas encore possible",
                ));
            }
            let target = request
                .target
                .to_target(|page| open.info.page_size(page).ok())
                .ok_or_else(|| AppError::new(ErrorKind::InvalidRequest, "cadre de signature invalide"))?;
            let bytes = read_file(&open.path, super::MAX_FILE_BYTES)?;
            if sha256_hex(&bytes) != open.saved_digest {
                return Err(AppError::new(
                    ErrorKind::Signature,
                    "le fichier a changé sur le disque depuis son ouverture : rouvrez-le avant de le signer",
                ));
            }
            let options = SignOptions {
                identity,
                target,
                reason: request.reason.clone(),
                location: request.location.clone(),
                lock: request.lock.into(),
                time: now,
                appearance: request.appearance.clone(),
            };
            let signed = pde_sign::sign(&bytes, &options).map_err(sign_error)?;
            let fields = self.check_signed(&signed)?;
            write_atomic(&open.path, &signed)?;
            log::info!("document signé : {} Ko", signed.len() / 1024);
            open.saved_digest = sha256_hex(&signed);
            open.signatures = fields;
            Ok(SavedView {
                file_name: super::edit::file_name(&open.path),
                path: open.path.display().to_string(),
            })
        })
    }

    /// Le fichier signé est relu avant d'être écrit : la nouvelle signature
    /// est valide et couvre tout le fichier, et le moteur l'ouvre.
    fn check_signed(&self, signed: &[u8]) -> Result<Vec<SignatureField>, AppError> {
        let fields = pde_sign::signatures(signed).map_err(sign_error)?;
        let newest = fields
            .iter()
            .filter_map(|f| f.signature.as_ref())
            .find(|s| s.covers_whole_document);
        if !newest.is_some_and(|s| s.status == SignatureStatus::Valid) {
            return Err(AppError::new(
                ErrorKind::Signature,
                "la signature produite n'a pas pu être vérifiée ; le fichier n'a pas été modifié",
            ));
        }
        let reopened = self.engine.open(signed.to_vec(), None)?;
        self.engine.close(reopened.id);
        Ok(fields)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::fake::FakeEngine;
    use crate::signing::{LockChoice, SignatureState, TargetRequest};

    /// Vrai PDF de deux pages (PDF de référence de pde-pdf).
    const REAL_PDF: &[u8] = include_bytes!("../../../pde-pdf/tests/fixtures/simple.pdf");
    const NOW: u64 = 1_795_003_200;

    struct Signing {
        viewer: Viewer,
        id: u64,
        path: std::path::PathBuf,
        dir: std::path::PathBuf,
    }

    impl Signing {
        fn new(name: &str) -> Self {
            let viewer = Viewer::new(Arc::new(FakeEngine::default()), 1 << 20);
            let dir =
                std::env::temp_dir().join(format!("pde-signing-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("contrat.pdf");
            std::fs::write(&path, REAL_PDF).unwrap();
            let id = viewer.open_file(&path, None).unwrap().id;
            Self {
                viewer,
                id,
                path,
                dir,
            }
        }
    }

    impl Drop for Signing {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn identity() -> Identity {
        Identity::self_signed(
            &pde_sign::Subject {
                name: "Claire Martin".into(),
                ..Default::default()
            },
            NOW,
        )
        .unwrap()
    }

    fn request(target: TargetRequest) -> SignRequest {
        SignRequest {
            identity: "x".into(),
            target,
            reason: Some("Accord".into()),
            location: None,
            lock: LockChoice::None,
            appearance: vec!["Claire Martin".into()],
        }
    }

    #[test]
    fn signs_the_file_in_place_and_lists_the_signature() {
        let s = Signing::new("place");
        assert!(s.viewer.signatures(s.id).unwrap().is_empty());
        let target = TargetRequest::New {
            page: 1,
            x: 300.0,
            y: 700.0,
            width: 200.0,
            height: 60.0,
        };
        let saved = s
            .viewer
            .sign(s.id, &identity(), &request(target), NOW)
            .unwrap();
        assert_eq!(saved.file_name, "contrat.pdf");
        let written = std::fs::read(&s.path).unwrap();
        assert!(written.starts_with(REAL_PDF), "mise à jour incrémentale");

        let found = s.viewer.signatures(s.id).unwrap();
        assert_eq!(found.len(), 1);
        let view = &found[0];
        assert_eq!(view.state, SignatureState::Valid);
        assert_eq!(view.signer.as_deref(), Some("Claire Martin"));
        assert_eq!(view.reason.as_deref(), Some("Accord"));
        assert_eq!(view.page, Some(1));
        assert!(view.self_signed && !view.modified_after);
        // Le cadre revient tel qu'envoyé, origine en haut.
        let bounds = view.bounds.as_ref().unwrap();
        assert!((bounds.x - 300.0).abs() < 0.01 && (bounds.y - 700.0).abs() < 0.01);
        assert!((bounds.width - 200.0).abs() < 0.01);

        // Une seconde signature suit la première (fichier connu à jour).
        s.viewer
            .sign(s.id, &identity(), &request(TargetRequest::Invisible), NOW)
            .unwrap();
        let found = s.viewer.signatures(s.id).unwrap();
        assert_eq!(found.len(), 2);
        assert!(found[0].modified_after);
        assert!(found[1].bounds.is_none());
    }

    #[test]
    fn unsaved_changes_or_changed_file_are_refused() {
        let s = Signing::new("refused");
        let metadata = pde_core::edit::Metadata {
            title: "Nouveau titre".into(),
            ..Default::default()
        };
        s.viewer.set_metadata(s.id, metadata).unwrap();
        let error = s
            .viewer
            .sign(s.id, &identity(), &request(TargetRequest::Invisible), NOW)
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Unsaved);
        s.viewer.undo(s.id).unwrap();

        std::fs::write(&s.path, b"%PDF-1.7 autre contenu").unwrap();
        let error = s
            .viewer
            .sign(s.id, &identity(), &request(TargetRequest::Invisible), NOW)
            .unwrap_err();
        assert!(error.message.contains("a changé sur le disque"), "{error}");
        // Le fichier n'a pas été touché.
        assert_eq!(std::fs::read(&s.path).unwrap(), b"%PDF-1.7 autre contenu");
    }

    #[test]
    fn invalid_frame_is_refused() {
        let s = Signing::new("frame");
        let target = TargetRequest::New {
            page: 9,
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        };
        let error = s
            .viewer
            .sign(s.id, &identity(), &request(target), NOW)
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidRequest);
    }
}
