//! Documents protégés par mot de passe (AES-256) : signer, vérifier,
//! enregistrer après signature, toujours avec le mot de passe.

mod common;

use common::{
    NOW, TEST_PASSWORD, assert_qpdf_accepts_with, encrypted_fixture, identity, pdfsig_with,
};
use lopdf::{Dictionary, Object, Stream};
use pde_sign::{
    Appearance, Lock, SignError, SignOptions, SignatureStatus, Target, append_revision, sign,
    signatures,
};

fn options<'a>(signer: &'a pde_sign::Identity, password: Option<&'a str>) -> SignOptions<'a> {
    SignOptions {
        identity: signer,
        target: Target::NewField {
            page: 0,
            rect: [300.0, 60.0, 520.0, 120.0],
        },
        reason: Some("Accord confidentiel".into()),
        location: None,
        lock: Lock::FormsAndSignatures,
        time: NOW,
        appearance: Appearance::text(vec!["Signé par Hélène".into()]),
        timestamp: None,
        password,
    }
}

#[test]
fn protected_document_is_signed_and_stays_protected() {
    let original = encrypted_fixture();
    let signer = identity("Hélène Dupré");
    let signed = sign(&original, &options(&signer, Some(TEST_PASSWORD))).unwrap();
    assert!(signed.starts_with(&original));

    // Toujours protégé : sans mot de passe, ni lu ni signé.
    assert!(matches!(
        signatures(&signed, None),
        Err(SignError::Encrypted)
    ));
    assert!(matches!(
        signatures(&signed, Some("faux")),
        Err(SignError::Encrypted)
    ));

    let found = signatures(&signed, Some(TEST_PASSWORD)).unwrap();
    let info = found[0].signature.as_ref().unwrap();
    assert_eq!(info.status, SignatureStatus::Valid);
    // Les chaînes du dictionnaire sont chiffrées puis bien relues.
    assert_eq!(info.reason.as_deref(), Some("Accord confidentiel"));
    assert_eq!(info.name.as_deref(), Some("Hélène Dupré"));
    // Rien de lisible en clair dans les octets ajoutés.
    let added = &signed[original.len()..];
    assert!(!added.windows(7).any(|w| w == b"Accord "), "motif chiffré");

    if let Some(report) = pdfsig_with("encrypted.pdf", &signed, &["-upw", TEST_PASSWORD]) {
        assert!(report.contains("Signature is Valid"), "{report}");
        assert!(report.contains("Hélène Dupré"), "{report}");
    }
    assert_qpdf_accepts_with(
        "encrypted.pdf",
        &signed,
        &[&format!("--password={TEST_PASSWORD}")],
    );
}

#[test]
fn signing_a_protected_document_needs_its_password() {
    let signer = identity("Hélène");
    let error = sign(&encrypted_fixture(), &options(&signer, None)).unwrap_err();
    assert!(matches!(error, SignError::Encrypted), "{error:?}");
    let error = sign(&encrypted_fixture(), &options(&signer, Some("faux"))).unwrap_err();
    assert!(matches!(error, SignError::Encrypted), "{error:?}");
}

#[test]
fn saving_a_signed_protected_document_keeps_signature_and_protection() {
    let signer = identity("Hélène");
    let signed = sign(&encrypted_fixture(), &options(&signer, Some(TEST_PASSWORD))).unwrap();
    // Réécriture complète (déchiffrée) avec un texte ajouté, comme un moteur.
    let mut doc = lopdf::Document::load_mem_with_options(
        &signed,
        lopdf::LoadOptions::with_password(TEST_PASSWORD),
    )
    .unwrap();
    let page = *doc.get_pages().get(&1).unwrap();
    let added = doc.add_object(Stream::new(
        Dictionary::new(),
        b"BT (Secret ajout) Tj ET".to_vec(),
    ));
    let contents = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .clone();
    doc.get_dictionary_mut(page)
        .unwrap()
        .set("Contents", vec![contents, Object::Reference(added)]);
    let mut rewritten = Vec::new();
    doc.save_to(&mut rewritten).unwrap();

    let saved = append_revision(&signed, &rewritten, Some(TEST_PASSWORD)).unwrap();
    assert!(saved.starts_with(&signed));
    assert!(
        !saved[signed.len()..].windows(6).any(|w| w == b"Secret"),
        "ajout chiffré"
    );
    let info = signatures(&saved, Some(TEST_PASSWORD)).unwrap()[0]
        .signature
        .clone()
        .unwrap();
    assert_eq!(info.status, SignatureStatus::Valid);
    assert_eq!(info.changes, vec![pde_sign::Change::PageContent]);
    if let Some(report) = pdfsig_with("encrypted-revision.pdf", &saved, &["-upw", TEST_PASSWORD]) {
        assert!(report.contains("Signature is Valid"), "{report}");
    }
    assert_qpdf_accepts_with(
        "encrypted-revision.pdf",
        &saved,
        &[&format!("--password={TEST_PASSWORD}")],
    );
}
