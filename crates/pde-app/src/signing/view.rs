//! Signatures telles que l'interface les reçoit, et demande de signature
//! telle qu'elle l'envoie (cadres en points, origine en haut à gauche).

use pde_core::document::PageSize;
use pde_core::geometry::{Point, Rect};
use pde_sign::{Lock, SignatureField, SignatureStatus, Target};
use serde::{Deserialize, Serialize};

use crate::text_view::BoxView;

/// État d'un champ de signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureState {
    Valid,
    Invalid,
    Unsupported,
    /// Champ vide, à signer.
    Empty,
}

/// Verrouillage, en mots de l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LockChoice {
    /// Le document reste modifiable.
    #[default]
    None,
    /// Seuls les formulaires et d'autres signatures restent permis.
    Forms,
    /// Plus aucune modification.
    All,
}

impl From<LockChoice> for Lock {
    fn from(choice: LockChoice) -> Self {
        match choice {
            LockChoice::None => Lock::None,
            LockChoice::Forms => Lock::FormsAndSignatures,
            LockChoice::All => Lock::Everything,
        }
    }
}

impl From<Lock> for LockChoice {
    fn from(lock: Lock) -> Self {
        match lock {
            Lock::None => LockChoice::None,
            Lock::FormsAndSignatures => LockChoice::Forms,
            Lock::Everything => LockChoice::All,
        }
    }
}

/// Horodatage d'une signature (S-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimestampView {
    /// Heure attestée par le serveur (jeton valide), secondes depuis 1970.
    pub time: Option<u64>,
    pub authority: Option<String>,
    /// Raison du refus d'un jeton invalide.
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SignatureView {
    /// Nom du champ.
    pub name: String,
    pub page: Option<usize>,
    pub bounds: Option<BoxView>,
    pub state: SignatureState,
    /// Raison d'une signature invalide ou non vérifiable.
    pub problem: Option<String>,
    /// Nom du signataire (certificat, sinon `/Name`).
    pub signer: Option<String>,
    pub organization: Option<String>,
    pub issuer: Option<String>,
    /// Certificat personnel, qu'aucune autorité ne garantit.
    pub self_signed: bool,
    pub reason: Option<String>,
    pub location: Option<String>,
    /// Instant de signature, secondes depuis 1970.
    pub time: Option<u64>,
    /// Des octets ont été ajoutés au fichier après cette signature.
    pub modified_after: bool,
    /// Signature de certification : verrouillage posé.
    pub lock: Option<LockChoice>,
    /// Horodatage joint.
    pub timestamp: Option<TimestampView>,
}

impl SignatureView {
    pub fn new(field: &SignatureField, page_size: impl Fn(usize) -> Option<PageSize>) -> Self {
        let bounds = match (field.page, field.rect) {
            (Some(page), Some(r)) if r[2] - r[0] > 0.0 && r[3] - r[1] > 0.0 => {
                page_size(page).map(|size| {
                    let rect = Rect::from_corners(
                        Point::new(f64::from(r[0]), f64::from(r[1])),
                        Point::new(f64::from(r[2]), f64::from(r[3])),
                    );
                    BoxView::new(&rect, size)
                })
            }
            _ => None,
        };
        let mut view = SignatureView {
            name: field.name.clone(),
            page: field.page,
            bounds,
            state: SignatureState::Empty,
            problem: None,
            signer: None,
            organization: None,
            issuer: None,
            self_signed: false,
            reason: None,
            location: None,
            time: None,
            modified_after: false,
            lock: None,
            timestamp: None,
        };
        if let Some(info) = &field.signature {
            (view.state, view.problem) = match &info.status {
                SignatureStatus::Valid => (SignatureState::Valid, None),
                SignatureStatus::Invalid(why) => (SignatureState::Invalid, Some(why.clone())),
                SignatureStatus::Unsupported(why) => {
                    (SignatureState::Unsupported, Some(why.clone()))
                }
            };
            let certificate = info.signer.as_ref();
            view.signer = certificate
                .map(|c| c.common_name.clone())
                .or_else(|| info.name.clone());
            view.organization = certificate.and_then(|c| c.organization.clone());
            view.issuer = certificate.map(|c| c.issuer_common_name.clone());
            view.self_signed = certificate.is_some_and(|c| c.self_signed);
            view.reason = info.reason.clone();
            view.location = info.location.clone();
            view.time = info.time;
            view.modified_after = !info.covers_whole_document;
            view.lock = info.lock.map(LockChoice::from);
            view.timestamp = info.timestamp.as_ref().map(|t| TimestampView {
                time: t.time,
                authority: t.authority.clone(),
                problem: t.problem.clone(),
            });
        }
        view
    }
}

/// Où signer, vu de l'interface.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetRequest {
    Invisible,
    /// Champ de signature vide existant.
    Field {
        name: String,
    },
    /// Nouveau cadre sur la page (points, origine en haut à gauche).
    New {
        page: usize,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
}

impl TargetRequest {
    /// Cible de `pde-sign` (origine en bas à gauche de la page affichée).
    pub fn to_target(&self, page_size: impl Fn(usize) -> Option<PageSize>) -> Option<Target> {
        Some(match self {
            TargetRequest::Invisible => Target::Invisible,
            TargetRequest::Field { name } => Target::Field(name.clone()),
            TargetRequest::New {
                page,
                x,
                y,
                width,
                height,
            } => {
                let size = page_size(*page)?;
                let valid = [*x, *y, *width, *height].iter().all(|v| v.is_finite())
                    && *width >= 1.0
                    && *height >= 1.0;
                if !valid {
                    return None;
                }
                let top = size.height - y;
                Target::NewField {
                    page: *page,
                    rect: [
                        *x as f32,
                        (top - height) as f32,
                        (x + width) as f32,
                        top as f32,
                    ],
                }
            }
        })
    }
}

/// Signature demandée par l'interface.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SignRequest {
    /// Identité choisie (voir `IdentityStore`).
    pub identity: String,
    pub target: TargetRequest,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub lock: LockChoice,
    /// Lignes de l'apparence visible (rédigées par l'interface).
    #[serde(default)]
    pub appearance: Vec<String>,
    /// Signature manuscrite (PNG en base64), au-dessus des lignes.
    #[serde(default)]
    pub image: Option<String>,
    /// Horodater par le serveur des réglages (S-08).
    #[serde(default)]
    pub timestamp: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a4(page: usize) -> Option<PageSize> {
        (page < 2).then(|| PageSize::new(595.0, 842.0))
    }

    #[test]
    fn new_frame_is_flipped_to_the_pdf_origin() {
        let request = TargetRequest::New {
            page: 1,
            x: 100.0,
            y: 42.0,
            width: 200.0,
            height: 50.0,
        };
        assert_eq!(
            request.to_target(a4),
            Some(Target::NewField {
                page: 1,
                rect: [100.0, 750.0, 300.0, 800.0]
            })
        );
    }

    #[test]
    fn invalid_frames_are_refused() {
        let frame = |page, width: f64| TargetRequest::New {
            page,
            x: 0.0,
            y: 0.0,
            width,
            height: 10.0,
        };
        assert_eq!(frame(5, 10.0).to_target(a4), None);
        assert_eq!(frame(0, 0.5).to_target(a4), None);
        assert_eq!(frame(0, f64::NAN).to_target(a4), None);
    }

    #[test]
    fn request_reads_the_interface_json() {
        let request: SignRequest = serde_json::from_str(
            r#"{"identity":"ab12","target":{"kind":"field","name":"Client"},"lock":"forms"}"#,
        )
        .unwrap();
        assert_eq!(
            request.target,
            TargetRequest::Field {
                name: "Client".into()
            }
        );
        assert_eq!(request.lock, LockChoice::Forms);
        assert!(request.appearance.is_empty());
    }
}
