//! Champs de signature existants, formulaires et identités importées.

mod common;

use common::{NOW, assert_qpdf_accepts, form_fixture, identity, pdfsig, with_empty_field};
use pde_sign::{
    Appearance, Identity, Lock, PrivateKey, SignOptions, SignatureStatus, Subject, Target, sign,
    signatures,
};

fn options(identity: &Identity, target: Target) -> SignOptions<'_> {
    SignOptions {
        identity,
        target,
        reason: None,
        location: None,
        lock: Lock::None,
        time: NOW,
        appearance: Appearance::text(vec!["Hugo Bernard".into()]),
        timestamp: None,
    }
}

#[test]
fn empty_field_is_listed_then_signed_in_place() {
    let original = with_empty_field();
    let before = signatures(&original).unwrap();
    assert_eq!(before.len(), 1);
    assert_eq!(before[0].name, "Client");
    assert_eq!(before[0].page, Some(0));
    assert_eq!(before[0].rect, Some([300.0, 60.0, 520.0, 120.0]));
    assert!(before[0].signature.is_none());

    let signer = identity("Hugo Bernard");
    let signed = sign(&original, &options(&signer, Target::Field("Client".into()))).unwrap();
    let after = signatures(&signed).unwrap();
    assert_eq!(after.len(), 1, "aucun champ ajouté : {after:?}");
    let info = after[0].signature.as_ref().unwrap();
    assert_eq!(info.status, SignatureStatus::Valid);
    assert!(info.covers_whole_document);
    if let Some(report) = pdfsig("field.pdf", &signed) {
        assert!(report.contains("Signature Field Name: Client"), "{report}");
        assert!(report.contains("Signature is Valid"), "{report}");
    }
    assert_qpdf_accepts("field.pdf", &signed);

    // Un champ signé ne se signe pas deux fois ; un champ inconnu est signalé.
    let again = sign(&signed, &options(&signer, Target::Field("Client".into()))).unwrap_err();
    assert!(again.to_string().contains("déjà signé"), "{again}");
    let unknown = sign(&original, &options(&signer, Target::Field("Autre".into()))).unwrap_err();
    assert!(unknown.to_string().contains("n'existe pas"), "{unknown}");
}

#[test]
fn new_field_name_does_not_collide_with_existing_ones() {
    let signer = identity("Claire Martin");
    let signed = sign(&with_empty_field(), &options(&signer, Target::Invisible)).unwrap();
    let names: Vec<String> = signatures(&signed)
        .unwrap()
        .into_iter()
        .map(|f| f.name)
        .collect();
    assert_eq!(names, ["Client", "Signature1"]);
}

#[test]
fn form_fields_survive_a_signature() {
    let original = form_fixture();
    let signer = identity("Claire Martin");
    let signed = sign(&original, &options(&signer, Target::Invisible)).unwrap();
    let found = signatures(&signed).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].signature.as_ref().unwrap().status,
        SignatureStatus::Valid
    );
    // Les champs du formulaire sont toujours là, suivis du champ de signature.
    let document = lopdf::Document::load_mem(&signed).unwrap();
    let catalog = document.catalog().unwrap();
    let form = document
        .dereference(catalog.get(b"AcroForm").unwrap())
        .unwrap()
        .1
        .as_dict()
        .unwrap();
    let fields = document
        .dereference(form.get(b"Fields").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap();
    assert!(fields.len() > 1);
    assert_eq!(form.get(b"SigFlags").unwrap().as_i64().unwrap(), 3);
    if let Some(report) = pdfsig("form.pdf", &signed) {
        assert!(report.contains("Signature is Valid"), "{report}");
    }
    assert_qpdf_accepts("form.pdf", &signed);
}

/// PKCS#12 RSA fabriqué pour le test (aucune clé réelle dans le dépôt).
fn rsa_pkcs12(password: &str) -> Vec<u8> {
    let key = rsa::PrivateKey::new(&mut rsa::rand_core::UnwrapErr(getrandom::SysRng), 2048)
        .expect("PKCS#12 de test");
    let identity = Identity::self_signed_with_key(
        &Subject {
            name: "Société Exemple".into(),
            ..Subject::default()
        },
        NOW,
        PrivateKey::Rsa(Box::new(key)),
    )
    .expect("PKCS#12 de test");
    let key =
        p12_keystore::PrivateKey::from_der(&identity.private_key_pkcs8().expect("PKCS#12 de test"))
            .expect("PKCS#12 de test");
    let certs = identity
        .chain_der()
        .expect("PKCS#12 de test")
        .iter()
        .map(|der| p12_keystore::Certificate::from_der(der).expect("PKCS#12 de test"))
        .collect::<Vec<_>>();
    let mut store = p12_keystore::KeyStore::new();
    store.add_entry(
        "rsa",
        p12_keystore::KeyStoreEntry::PrivateKeyChain(p12_keystore::PrivateKeyChain::new(
            b"k".to_vec(),
            key,
            certs,
        )),
    );
    store.writer(password).write().expect("PKCS#12 de test")
}

#[test]
fn imported_rsa_identity_signs_valid_documents() {
    let data = rsa_pkcs12("mot de passe de test");
    let signer = Identity::from_pkcs12(&data, "mot de passe de test").unwrap();
    let signed = sign(
        &common::two_pages(false),
        &options(&signer, Target::Invisible),
    )
    .unwrap();
    let info = signatures(&signed).unwrap()[0].signature.clone().unwrap();
    assert_eq!(info.status, SignatureStatus::Valid);
    assert_eq!(info.signer.unwrap().common_name, "Société Exemple");
    if let Some(report) = pdfsig("rsa.pdf", &signed) {
        assert!(report.contains("Signature is Valid"), "{report}");
    }
}
