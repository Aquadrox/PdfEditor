//! Enregistrement d'un document signé (F-03) : le document réécrit en
//! entier (comme le fait le moteur PDF) n'ajoute à la fin du fichier signé
//! que ce qui a changé ; la signature reste valide.

mod common;

use common::{NOW, assert_qpdf_accepts, identity, pdfsig, two_pages};
use lopdf::{Dictionary, Document, Object, Stream};
use pde_sign::{
    Appearance, Lock, SignOptions, SignatureStatus, Target, append_revision, sign, signatures,
};

fn signed() -> Vec<u8> {
    let signer = identity("Claire Martin");
    sign(
        &two_pages(false),
        &SignOptions {
            identity: &signer,
            target: Target::Invisible,
            reason: None,
            location: None,
            lock: Lock::None,
            time: NOW,
            appearance: Appearance::default(),
            timestamp: None,
        },
    )
    .expect("données de test")
}

/// Réécriture complète, comme un moteur : un texte ajouté sur la page 1,
/// et tous les autres objets réécrits (clés dans un autre ordre).
fn rewritten_with_addition(bytes: &[u8], tamper_signature: bool) -> Vec<u8> {
    let mut doc = Document::load_mem(bytes).expect("données de test");
    let page = *doc.get_pages().get(&1).expect("données de test");
    let added = doc.add_object(Stream::new(
        Dictionary::new(),
        b"BT /F1 12 Tf 72 500 Td (Ajout) Tj ET".to_vec(),
    ));
    let contents = doc
        .get_dictionary(page)
        .expect("données de test")
        .get(b"Contents")
        .expect("données de test")
        .clone();
    doc.get_dictionary_mut(page)
        .expect("données de test")
        .set("Contents", vec![contents, Object::Reference(added)]);
    for object in doc.objects.values_mut() {
        if let Object::Dictionary(dict) = object {
            let mut reversed = Dictionary::new();
            for (k, v) in dict.iter().rev() {
                reversed.set(k.clone(), v.clone());
            }
            *dict = reversed;
        }
    }
    if tamper_signature {
        for object in doc.objects.values_mut() {
            if let Object::Dictionary(dict) = object
                && dict.has(b"ByteRange")
            {
                dict.set("Reason", Object::string_literal("falsifié"));
            }
        }
    }
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("données de test");
    out
}

#[test]
fn changes_are_appended_and_the_signature_stays_valid() {
    let original = signed();
    let saved = append_revision(&original, &rewritten_with_addition(&original, false)).unwrap();
    assert!(
        saved.starts_with(&original),
        "les octets signés ne bougent pas"
    );
    // Seuls la page et le nouveau flux sont ajoutés (pas tout le document).
    assert!(
        saved.len() - original.len() < 1500,
        "{} octets ajoutés",
        saved.len() - original.len()
    );

    let found = signatures(&saved).unwrap();
    let info = found[0].signature.as_ref().unwrap();
    assert_eq!(info.status, SignatureStatus::Valid);
    assert!(!info.covers_whole_document);
    let text = Document::load_mem(&saved)
        .unwrap()
        .extract_text(&[1])
        .unwrap();
    assert!(text.contains("Ajout"), "{text}");

    if let Some(report) = pdfsig("revision.pdf", &saved) {
        assert!(report.contains("Signature is Valid"), "{report}");
    }
    assert_qpdf_accepts("revision.pdf", &saved);
}

#[test]
fn signature_dictionaries_are_never_rewritten() {
    let original = signed();
    let saved = append_revision(&original, &rewritten_with_addition(&original, true)).unwrap();
    let info = signatures(&saved).unwrap()[0].signature.clone().unwrap();
    assert_eq!(info.status, SignatureStatus::Valid);
    assert_eq!(
        info.reason, None,
        "le dictionnaire de signature d'origine est gardé"
    );
}

#[test]
fn nothing_changed_gives_the_same_file() {
    let original = signed();
    let mut doc = Document::load_mem(&original).unwrap();
    let mut rewritten = Vec::new();
    doc.save_to(&mut rewritten).unwrap();
    assert_eq!(append_revision(&original, &rewritten).unwrap(), original);
}

#[test]
fn a_second_revision_follows_the_first() {
    let original = signed();
    let once = append_revision(&original, &rewritten_with_addition(&original, false)).unwrap();
    let twice = append_revision(&once, &rewritten_with_addition(&once, false)).unwrap();
    assert!(twice.starts_with(&once));
    let info = signatures(&twice).unwrap()[0].signature.clone().unwrap();
    assert_eq!(info.status, SignatureStatus::Valid);
    if let Some(report) = pdfsig("revision-2.pdf", &twice) {
        assert!(report.contains("Signature is Valid"), "{report}");
    }
}
