//! Signature de bout en bout : signer, relire avec notre vérificateur, puis
//! contre-vérifier avec `pdfsig` (Poppler) et `qpdf`.

mod common;

use common::{NOW, assert_qpdf_accepts, identity, pdfsig, two_pages};
use pde_sign::{Lock, SignOptions, SignatureStatus, Target, sign, signatures};

fn options<'a>(identity: &'a pde_sign::Identity, target: Target, lock: Lock) -> SignOptions<'a> {
    SignOptions {
        identity,
        target,
        reason: Some("J'approuve ce document".into()),
        location: Some("Lyon".into()),
        lock,
        time: NOW,
        appearance: vec![
            "Signé numériquement par Claire Martin".into(),
            "18/11/2026 13:00".into(),
        ],
    }
}

#[test]
fn invisible_signature_is_valid_for_us_and_for_poppler() {
    let original = two_pages(false);
    let signer = identity("Claire Martin");
    let signed = sign(&original, &options(&signer, Target::Invisible, Lock::None)).unwrap();

    // Les octets d'origine sont intacts : la signature est ajoutée à la fin.
    assert_eq!(&signed[..original.len()], &original[..]);

    let found = signatures(&signed).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Signature1");
    assert_eq!(found[0].page, Some(0));
    let info = found[0].signature.as_ref().unwrap();
    assert_eq!(info.status, SignatureStatus::Valid);
    assert!(info.covers_whole_document);
    assert_eq!(info.name.as_deref(), Some("Claire Martin"));
    assert_eq!(info.reason.as_deref(), Some("J'approuve ce document"));
    assert_eq!(info.location.as_deref(), Some("Lyon"));
    assert_eq!(info.time, Some(NOW));
    assert_eq!(info.lock, None);
    let certificate = info.signer.as_ref().unwrap();
    assert_eq!(certificate.common_name, "Claire Martin");
    assert!(certificate.self_signed);

    if let Some(report) = pdfsig("invisible.pdf", &signed) {
        assert!(report.contains("Signature is Valid"), "{report}");
        assert!(report.contains("Claire Martin"), "{report}");
        assert!(report.contains("ETSI.CAdES.detached"), "{report}");
    }
    assert_qpdf_accepts("invisible.pdf", &signed);
}

#[test]
fn visible_signature_on_a_cross_reference_stream_file() {
    let original = two_pages(true);
    let signer = identity("Hélène Dupré");
    let target = Target::NewField {
        page: 1,
        rect: [350.0, 80.0, 550.0, 130.0],
    };
    let signed = sign(&original, &options(&signer, target, Lock::None)).unwrap();
    let found = signatures(&signed).unwrap();
    assert_eq!(found[0].page, Some(1));
    assert_eq!(found[0].rect, Some([350.0, 80.0, 550.0, 130.0]));
    let info = found[0].signature.as_ref().unwrap();
    assert_eq!(info.status, SignatureStatus::Valid);
    assert_eq!(info.name.as_deref(), Some("Hélène Dupré"));
    if let Some(report) = pdfsig("visible-xref-stream.pdf", &signed) {
        assert!(report.contains("Signature is Valid"), "{report}");
    }
    assert_qpdf_accepts("visible-xref-stream.pdf", &signed);
}

#[test]
fn second_signature_keeps_the_first_one_valid() {
    let original = two_pages(false);
    let first = identity("Claire Martin");
    let second = identity("Hugo Bernard");
    let once = sign(&original, &options(&first, Target::Invisible, Lock::None)).unwrap();
    let target = Target::NewField {
        page: 0,
        rect: [72.0, 72.0, 272.0, 122.0],
    };
    let twice = sign(&once, &options(&second, target, Lock::None)).unwrap();

    let found = signatures(&twice).unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[1].name, "Signature2");
    let older = found[0].signature.as_ref().unwrap();
    let newer = found[1].signature.as_ref().unwrap();
    assert_eq!(older.status, SignatureStatus::Valid);
    // La première ne couvre plus tout le fichier : une révision a suivi.
    assert!(!older.covers_whole_document);
    assert_eq!(newer.status, SignatureStatus::Valid);
    assert!(newer.covers_whole_document);

    if let Some(report) = pdfsig("two-signatures.pdf", &twice) {
        assert_eq!(report.matches("Signature is Valid").count(), 2, "{report}");
    }
    assert_qpdf_accepts("two-signatures.pdf", &twice);
}

#[test]
fn certification_locks_the_document() {
    let original = two_pages(false);
    let signer = identity("Claire Martin");
    let signed = sign(
        &original,
        &options(&signer, Target::Invisible, Lock::Everything),
    )
    .unwrap();
    let found = signatures(&signed).unwrap();
    let info = found[0].signature.as_ref().unwrap();
    assert_eq!(info.status, SignatureStatus::Valid);
    assert_eq!(info.lock, Some(Lock::Everything));

    // Plus aucune signature possible sur un document verrouillé.
    let other = identity("Hugo Bernard");
    let error = sign(&signed, &options(&other, Target::Invisible, Lock::None)).unwrap_err();
    assert!(matches!(error, pde_sign::SignError::Locked), "{error:?}");

    if let Some(report) = pdfsig("certified.pdf", &signed) {
        assert!(report.contains("Signature is Valid"), "{report}");
    }
}

#[test]
fn forms_and_signatures_lock_allows_another_signature() {
    let original = two_pages(false);
    let first = identity("Claire Martin");
    let signed = sign(
        &original,
        &options(&first, Target::Invisible, Lock::FormsAndSignatures),
    )
    .unwrap();
    let second = identity("Hugo Bernard");
    let twice = sign(&signed, &options(&second, Target::Invisible, Lock::None)).unwrap();
    let found = signatures(&twice).unwrap();
    assert_eq!(
        found[0].signature.as_ref().unwrap().lock,
        Some(Lock::FormsAndSignatures)
    );
    assert_eq!(
        found[1].signature.as_ref().unwrap().status,
        SignatureStatus::Valid
    );
    // Verrouiller exige d'être la première signature.
    let third = identity("Zoé");
    let error = sign(
        &twice,
        &options(&third, Target::Invisible, Lock::FormsAndSignatures),
    )
    .unwrap_err();
    assert!(
        matches!(error, pde_sign::SignError::AlreadySigned),
        "{error:?}"
    );
}

#[test]
fn a_changed_byte_breaks_the_signature() {
    let original = two_pages(false);
    let signer = identity("Claire Martin");
    let mut signed = sign(&original, &options(&signer, Target::Invisible, Lock::None)).unwrap();
    // Le texte de la première page change d'une lettre.
    let at = signed
        .windows(7)
        .position(|w| w == b"Contrat")
        .expect("texte présent");
    signed[at] = b'K';
    let found = signatures(&signed).unwrap();
    assert_eq!(
        found[0].signature.as_ref().unwrap().status,
        SignatureStatus::Invalid("le document a été modifié après la signature".into())
    );
    if let Some(report) = pdfsig("tampered.pdf", &signed) {
        assert!(!report.contains("Signature is Valid"), "{report}");
    }
}

#[test]
fn appended_bytes_are_reported_as_later_changes() {
    let original = two_pages(false);
    let signer = identity("Claire Martin");
    let mut signed = sign(&original, &options(&signer, Target::Invisible, Lock::None)).unwrap();
    signed.extend_from_slice(b"\n% ajout\n");
    let info = signatures(&signed).unwrap()[0].signature.clone().unwrap();
    assert_eq!(info.status, SignatureStatus::Valid);
    assert!(!info.covers_whole_document);
}

#[test]
fn unknown_page_is_refused() {
    let signer = identity("Claire Martin");
    let target = Target::NewField {
        page: 5,
        rect: [0.0, 0.0, 10.0, 10.0],
    };
    let error = sign(&two_pages(false), &options(&signer, target, Lock::None)).unwrap_err();
    assert!(error.to_string().contains("page 6"), "{error}");
}

#[test]
fn unsigned_document_has_no_signature() {
    assert!(signatures(&two_pages(false)).unwrap().is_empty());
    assert!(signatures(&common::simple_fixture()).unwrap().is_empty());
}

#[test]
fn frame_follows_crop_box_and_rotation() {
    let mut doc = lopdf::Document::load_mem(&two_pages(false)).unwrap();
    let pages = doc.get_pages();
    let first = doc.get_dictionary_mut(pages[&1]).unwrap();
    first.set(
        "CropBox",
        vec![50.into(), 50.into(), 545.into(), 792.into()],
    );
    doc.get_dictionary_mut(pages[&2]).unwrap().set("Rotate", 90);
    let mut original = Vec::new();
    doc.save_to(&mut original).unwrap();

    let signer = identity("Claire Martin");
    let shown = [10.0, 10.0, 110.0, 60.0];
    let once = sign(
        &original,
        &options(
            &signer,
            Target::NewField {
                page: 0,
                rect: shown,
            },
            Lock::None,
        ),
    )
    .unwrap();
    let twice = sign(
        &once,
        &options(
            &signer,
            Target::NewField {
                page: 1,
                rect: shown,
            },
            Lock::None,
        ),
    )
    .unwrap();
    let found = signatures(&twice).unwrap();
    // Relu dans l'espace affiché : le même cadre qu'à la signature.
    assert_eq!(found[0].rect, Some(shown));
    assert_eq!(found[1].rect, Some(shown));
    // Dans le fichier : décalé de la CropBox sur la page 1.
    let saved = lopdf::Document::load_mem(&twice).unwrap();
    let widget = saved
        .objects
        .values()
        .filter_map(|o| o.as_dict().ok())
        .find(|d| d.get(b"T").and_then(|t| t.as_str()).ok() == Some(b"Signature1".as_slice()))
        .unwrap();
    let rect: Vec<f32> = widget
        .get(b"Rect")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_float().unwrap())
        .collect();
    assert_eq!(rect, [60.0, 60.0, 160.0, 110.0]);
}
