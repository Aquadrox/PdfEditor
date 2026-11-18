//! Lister et vérifier les signatures d'un PDF (S-07).
//!
//! Pour chaque champ de signature : intégrité des octets couverts
//! (`/ByteRange`), signature CMS, certificat, et si le fichier a été
//! modifié depuis (octets ajoutés après la révision signée).

use lopdf::{Dictionary, Object};

use crate::certificate::CertificateInfo;
use crate::cms::{self, CmsFailure};
use crate::error::Result;
use crate::pdf::date;
use crate::pdf::fields::{self, Field};
use crate::pdf::geometry::PageFrame;
use crate::pdf::permissions;
use crate::pdf::sig_dict::Lock;
use crate::pdf::source::Source;

/// Verdict sur une signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureStatus {
    /// Octets signés intacts, signature correcte.
    Valid,
    /// Le document ou la signature a été altéré ; la raison en français.
    Invalid(String),
    /// Format ou algorithme que l'application ne sait pas vérifier.
    Unsupported(String),
}

/// Une signature posée dans un champ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureInfo {
    pub status: SignatureStatus,
    /// Certificat du signataire, s'il a pu être lu.
    pub signer: Option<CertificateInfo>,
    /// Nom déclaré dans le dictionnaire (`/Name`).
    pub name: Option<String>,
    pub reason: Option<String>,
    pub location: Option<String>,
    /// Instant de signature (`/M`, sinon attribut du CMS), secondes depuis 1970.
    pub time: Option<u64>,
    /// La signature couvre tout le fichier : rien n'a été ajouté depuis.
    pub covers_whole_document: bool,
    /// Signature de certification : niveau de verrouillage.
    pub lock: Option<Lock>,
}

/// Un champ de signature du document, signé ou vide.
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureField {
    /// Nom complet du champ.
    pub name: String,
    /// Page du widget (à partir de 0).
    pub page: Option<usize>,
    /// Cadre du widget, en points dans la page affichée (origine en bas à
    /// gauche de la zone visible).
    pub rect: Option<[f32; 4]>,
    /// `None` : champ vide, à signer.
    pub signature: Option<SignatureInfo>,
}

/// Champs de signature du PDF, dans l'ordre du formulaire.
pub fn signatures(pdf: &[u8]) -> Result<Vec<SignatureField>> {
    let source = Source::load(pdf)?;
    let certification = permissions::certification(&source);
    Ok(fields::all(&source)
        .into_iter()
        .filter(|field| field.is_signature)
        .map(|field| {
            let (page, rect) = fields::placement(&source, &field);
            // Cadre rendu dans l'espace de la page affichée, comme à la signature.
            let rect = match (page, rect) {
                (Some(page), Some(rect)) => source
                    .page_id(page)
                    .ok()
                    .map(|id| PageFrame::of(&source, id).rect_to_display(rect)),
                _ => rect,
            };
            let signature = signed(&source, &field, certification);
            SignatureField {
                name: field.name,
                page,
                rect,
                signature,
            }
        })
        .collect())
}

fn signed(
    source: &Source,
    field: &Field,
    certification: Option<(Option<lopdf::ObjectId>, i64)>,
) -> Option<SignatureInfo> {
    let value = field.value.as_ref()?;
    let dictionary = source.resolve(value).ok()?.as_dict().ok()?;
    let text = |key: &[u8]| {
        dictionary
            .get(key)
            .ok()
            .and_then(|v| lopdf::decode_text_string(v).ok())
            .filter(|v| !v.trim().is_empty())
    };
    let is_certification = match certification {
        Some((Some(id), _)) => value.as_reference().ok() == Some(id),
        _ => false,
    };
    let lock = if is_certification {
        certification.map(|(_, p)| Lock::from_permission(p))
    } else {
        permissions::permission(source, dictionary).map(Lock::from_permission)
    };
    let (status, signer, cms_time, covers) = check(source.bytes, dictionary);
    Some(SignatureInfo {
        status,
        signer,
        name: text(b"Name"),
        reason: text(b"Reason"),
        location: text(b"Location"),
        time: text(b"M").and_then(|m| date::parse(&m)).or(cms_time),
        covers_whole_document: covers,
        lock,
    })
}

type Check = (SignatureStatus, Option<CertificateInfo>, Option<u64>, bool);

fn check(bytes: &[u8], dictionary: &Dictionary) -> Check {
    let invalid = |reason: &str| (SignatureStatus::Invalid(reason.into()), None, None, false);
    let sub_filter = dictionary
        .get(b"SubFilter")
        .and_then(Object::as_name)
        .unwrap_or_default();
    if !matches!(sub_filter, b"ETSI.CAdES.detached" | b"adbe.pkcs7.detached") {
        let name = String::from_utf8_lossy(sub_filter).into_owned();
        return (
            SignatureStatus::Unsupported(format!("format de signature {name}")),
            None,
            None,
            false,
        );
    }
    let Some(range) = byte_range(dictionary) else {
        return invalid("zone signée illisible");
    };
    let [_, open, close, tail] = range;
    let end = close + tail;
    if !gap_holds_only_the_signature(bytes, open, close) || end > bytes.len() {
        return invalid("zone signée incohérente");
    }
    let Ok(contents) = dictionary.get(b"Contents").and_then(Object::as_str) else {
        return invalid("signature absente");
    };
    let covers = bytes[end..].iter().all(u8::is_ascii_whitespace);
    match cms::verify_detached(contents, &[&bytes[..open], &bytes[close..end]]) {
        Ok(verified) => {
            let signer = CertificateInfo::from_der(&verified.certificate).ok();
            (
                SignatureStatus::Valid,
                signer,
                verified.signing_time,
                covers,
            )
        }
        Err(failure) => {
            let status = match failure {
                CmsFailure::DigestMismatch => {
                    SignatureStatus::Invalid("le document a été modifié après la signature".into())
                }
                CmsFailure::BadSignature => {
                    SignatureStatus::Invalid("la signature ne correspond pas au certificat".into())
                }
                CmsFailure::CertificateMismatch => {
                    SignatureStatus::Invalid("le certificat joint a été remplacé".into())
                }
                CmsFailure::Malformed(detail) => {
                    SignatureStatus::Invalid(format!("signature illisible ({detail})"))
                }
                CmsFailure::Unsupported(detail) => SignatureStatus::Unsupported(detail),
            };
            (status, None, None, covers)
        }
    }
}

/// `[0 a b c]` : début à 0, trou de `a` à `b`.
fn byte_range(dictionary: &Dictionary) -> Option<[usize; 4]> {
    let values = dictionary.get(b"ByteRange").ok()?.as_array().ok()?;
    let numbers: Vec<usize> = values
        .iter()
        .map(|v| v.as_i64().ok().and_then(|n| usize::try_from(n).ok()))
        .collect::<Option<_>>()?;
    let range = <[usize; 4]>::try_from(numbers).ok()?;
    (range[0] == 0 && range[1] < range[2]).then_some(range)
}

/// Le trou ne doit contenir que `<hexadécimal>` : sinon des octets non
/// signés pourraient s'y cacher.
fn gap_holds_only_the_signature(bytes: &[u8], open: usize, close: usize) -> bool {
    let Some(gap) = bytes.get(open..close) else {
        return false;
    };
    gap.len() >= 2
        && gap[0] == b'<'
        && gap[gap.len() - 1] == b'>'
        && gap[1..gap.len() - 1].iter().all(u8::is_ascii_hexdigit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gap_must_be_pure_hex() {
        let bytes = b"abc<00FF>def<00 FF>";
        assert!(gap_holds_only_the_signature(bytes, 3, 9));
        assert!(!gap_holds_only_the_signature(bytes, 12, 19));
        assert!(!gap_holds_only_the_signature(bytes, 2, 9));
        assert!(!gap_holds_only_the_signature(bytes, 3, 99));
    }
}
