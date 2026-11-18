//! Signatures du document ouvert (S-07) et signature (S-05, S-06).
//!
//! On signe le **fichier enregistré** : le document ne doit pas avoir de
//! modifications en attente, et le fichier sur le disque doit être celui
//! que l'application a ouvert ou enregistré. La signature est ajoutée par
//! une mise à jour incrémentale, relue par notre vérificateur et par le
//! moteur, puis le fichier est remplacé de façon atomique.

use pde_sign::{
    Appearance, Identity, SignOptions, SignatureField, SignatureStatus, TimestampAuthority,
};

use super::{OpenDocument, Viewer, read_file, sha256_hex};
use crate::edit_view::SavedView;
use crate::error::{AppError, ErrorKind};
use crate::files::write_atomic;
use crate::signing::drawings::{decode_png, png_from_base64};
use crate::signing::identities::sign_error;
use crate::signing::{SignRequest, SignatureView};

/// Au-delà de cette taille, les signatures ne sont cherchées que si le
/// fichier en contient visiblement une (lecture complète de la structure).
const QUICK_SCAN_BYTES: usize = 64 * 1024 * 1024;

/// Champs de signature d'un fichier ; un PDF illisible pour `lopdf` (mais
/// ouvert par PDFium) n'en a simplement pas.
pub(super) fn read_signatures(bytes: &[u8], password: Option<&str>) -> Vec<SignatureField> {
    if bytes.len() > QUICK_SCAN_BYTES && !contains(bytes, b"/ByteRange") {
        return Vec::new();
    }
    pde_sign::signatures(bytes, password).unwrap_or_else(|e| {
        log::info!("signatures non lues : {e}");
        Vec::new()
    })
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Fichier à écrire pour enregistrer un document signé (F-03) : le fichier
/// signé suivi d'une mise à jour incrémentale avec ce que le moteur a
/// changé (`rewritten`). Les signatures valides doivent le rester ; sinon
/// rien n'est écrit. Document sans signature : `rewritten` tel quel.
pub(super) fn preserve_signatures(
    open: &OpenDocument,
    rewritten: Vec<u8>,
) -> Result<Vec<u8>, AppError> {
    let valid = |fields: &[SignatureField]| {
        fields
            .iter()
            .filter(|f| {
                f.signature
                    .as_ref()
                    .is_some_and(|s| s.status == SignatureStatus::Valid)
            })
            .count()
    };
    if !open.signatures.iter().any(|f| f.signature.is_some()) {
        return Ok(rewritten);
    }
    let current = read_file(&open.path, super::MAX_FILE_BYTES)?;
    if sha256_hex(&current) != open.saved_digest {
        return Err(AppError::new(
            ErrorKind::Signature,
            "le fichier signé a changé sur le disque depuis son ouverture : rouvrez-le avant d'enregistrer",
        ));
    }
    let password = open.password.as_deref();
    let merged = pde_sign::append_revision(&current, &rewritten, password).map_err(|e| {
        AppError::new(
            ErrorKind::Signature,
            format!("les signatures ne peuvent pas être préservées ({e})"),
        )
    })?;
    let after = pde_sign::signatures(&merged, password).map_err(sign_error)?;
    if valid(&after) < valid(&open.signatures) {
        return Err(AppError::new(
            ErrorKind::Signature,
            "l'enregistrement invaliderait une signature : le fichier n'a pas été modifié",
        ));
    }
    log::info!(
        "document signé enregistré par ajout : {} Ko ajoutés",
        (merged.len() - current.len()) / 1024
    );
    Ok(merged)
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
    /// depuis 1970 ; `tsa` : serveur d'horodatage éventuel. Le document
    /// affiché reste celui d'avant : l'interface le rouvre pour montrer le
    /// fichier signé.
    ///
    /// Les documents ne restent pas verrouillés pendant la signature (appel
    /// au serveur d'horodatage) : les pages continuent de s'afficher.
    pub fn sign(
        &self,
        id: u64,
        identity: &Identity,
        request: &SignRequest,
        now: u64,
        tsa: Option<&dyn TimestampAuthority>,
    ) -> Result<SavedView, AppError> {
        let (path, digest, target, password) = self.with_document(id, |open, _| {
            if open.edits.is_dirty() {
                return Err(AppError::new(
                    ErrorKind::Unsaved,
                    "enregistrez le document avant de le signer",
                ));
            }
            let target = request
                .target
                .to_target(|page| open.info.page_size(page).ok())
                .ok_or_else(|| {
                    AppError::new(ErrorKind::InvalidRequest, "cadre de signature invalide")
                })?;
            Ok((
                open.path.clone(),
                open.saved_digest.clone(),
                target,
                open.password.clone(),
            ))
        })?;
        let appearance = Appearance {
            image: match &request.image {
                Some(png) => Some(decode_png(&png_from_base64(png)?)?),
                None => None,
            },
            lines: request.appearance.clone(),
        };
        let bytes = read_file(&path, super::MAX_FILE_BYTES)?;
        if sha256_hex(&bytes) != digest {
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
            appearance,
            timestamp: tsa,
            password: password.as_deref(),
        };
        let signed = pde_sign::sign(&bytes, &options).map_err(sign_error)?;
        let fields = self.check_signed(&signed, password.as_deref())?;
        self.with_document(id, |open, _| {
            // Fichier enregistré entre-temps : on ne l'écrase pas.
            if open.path != path || open.saved_digest != digest {
                return Err(AppError::new(
                    ErrorKind::Signature,
                    "le document a été enregistré pendant la signature : recommencez",
                ));
            }
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
    fn check_signed(
        &self,
        signed: &[u8],
        password: Option<&str>,
    ) -> Result<Vec<SignatureField>, AppError> {
        let fields = pde_sign::signatures(signed, password).map_err(sign_error)?;
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
        let reopened = self
            .engine
            .open(signed.to_vec(), password.map(str::to_owned))?;
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
            timestamp: false,
            image: None,
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
            .sign(s.id, &identity(), &request(target), NOW, None)
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
            .sign(
                s.id,
                &identity(),
                &request(TargetRequest::Invisible),
                NOW,
                None,
            )
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
            .sign(
                s.id,
                &identity(),
                &request(TargetRequest::Invisible),
                NOW,
                None,
            )
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Unsaved);
        s.viewer.undo(s.id).unwrap();

        std::fs::write(&s.path, b"%PDF-1.7 autre contenu").unwrap();
        let error = s
            .viewer
            .sign(
                s.id,
                &identity(),
                &request(TargetRequest::Invisible),
                NOW,
                None,
            )
            .unwrap_err();
        assert!(error.message.contains("a changé sur le disque"), "{error}");
        // Le fichier n'a pas été touché.
        assert_eq!(std::fs::read(&s.path).unwrap(), b"%PDF-1.7 autre contenu");
    }

    #[test]
    fn saving_a_signed_document_keeps_its_signature() {
        let s = Signing::new("keep");
        s.viewer
            .sign(
                s.id,
                &identity(),
                &request(TargetRequest::Invisible),
                NOW,
                None,
            )
            .unwrap();
        let signed = std::fs::read(&s.path).unwrap();
        // Rouvert comme le fait l'interface, modifié puis enregistré.
        let id = s.viewer.open_file(&s.path, None).unwrap().id;
        let metadata = pde_core::edit::Metadata {
            title: "Nouveau titre".into(),
            ..Default::default()
        };
        s.viewer.set_metadata(id, metadata).unwrap();
        s.viewer.save(id, None).unwrap();
        let saved = std::fs::read(&s.path).unwrap();
        assert!(saved.starts_with(&signed), "signature préservée");
        let found = s.viewer.signatures(id).unwrap();
        assert_eq!(found[0].state, SignatureState::Valid);

        // Fichier signé changé entre-temps : rien n'est écrit.
        s.viewer
            .set_metadata(id, pde_core::edit::Metadata::default())
            .unwrap();
        std::fs::write(&s.path, b"%PDF-1.7 autre").unwrap();
        let error = s.viewer.save(id, None).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Signature);
        assert_eq!(std::fs::read(&s.path).unwrap(), b"%PDF-1.7 autre");
    }

    #[test]
    fn timestamp_comes_from_the_server_over_http() {
        let s = Signing::new("timestamp");
        let url = crate::signing::tsa::tests::serve_once(
            crate::signing::tsa::tests::local_authority(NOW + 3),
        );
        let tsa = crate::signing::HttpTimestampAuthority::new(&url).unwrap();
        let mut asked = request(TargetRequest::Invisible);
        asked.timestamp = true;
        s.viewer
            .sign(s.id, &identity(), &asked, NOW, Some(&tsa))
            .unwrap();
        let view = &s.viewer.signatures(s.id).unwrap()[0];
        assert_eq!(view.state, SignatureState::Valid);
        let timestamp = view.timestamp.as_ref().unwrap();
        assert_eq!(timestamp.time, Some(NOW + 3));
        assert_eq!(timestamp.authority.as_deref(), Some("Horodatage de test"));
        assert_eq!(timestamp.problem, None);
    }

    #[test]
    fn handwritten_image_is_decoded_and_bad_images_refused() {
        use base64::Engine;
        let s = Signing::new("image");
        let mut ink = image::RgbaImage::new(40, 12);
        ink.put_pixel(3, 4, image::Rgba([20, 30, 90, 255]));
        let mut png = Vec::new();
        ink.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let frame = TargetRequest::New {
            page: 0,
            x: 300.0,
            y: 700.0,
            width: 200.0,
            height: 60.0,
        };
        let mut asked = request(frame);
        asked.image = Some("pas une image".into());
        let error = s
            .viewer
            .sign(s.id, &identity(), &asked, NOW, None)
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidRequest);
        assert_eq!(std::fs::read(&s.path).unwrap(), REAL_PDF);
        asked.image = Some(base64::engine::general_purpose::STANDARD.encode(png));
        s.viewer.sign(s.id, &identity(), &asked, NOW, None).unwrap();
        assert_eq!(
            s.viewer.signatures(s.id).unwrap()[0].state,
            SignatureState::Valid
        );
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
            .sign(s.id, &identity(), &request(target), NOW, None)
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidRequest);
    }
}
