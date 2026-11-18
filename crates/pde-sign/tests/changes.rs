//! Ce qui a changé après une signature, et si la certification le permettait.

mod common;

use common::{NOW, form_fixture, identity, two_pages};
use lopdf::{Dictionary, Document, Object, Stream, dictionary};
use pde_sign::{
    Appearance, Change, Lock, SignOptions, SignatureInfo, Target, append_revision, sign, signatures,
};

fn signed(pdf: &[u8], name: &str, lock: Lock) -> Vec<u8> {
    let signer = identity(name);
    sign(
        pdf,
        &SignOptions {
            identity: &signer,
            target: Target::Invisible,
            reason: None,
            location: None,
            lock,
            time: NOW,
            appearance: Appearance::default(),
            timestamp: None,
            password: None,
        },
    )
    .expect("signature de test")
}

/// Réécrit le document après `change` (comme le moteur), puis l'ajoute en révision.
fn revised(pdf: &[u8], change: impl FnOnce(&mut Document)) -> Vec<u8> {
    let mut doc = Document::load_mem(pdf).expect("relecture");
    change(&mut doc);
    let mut rewritten = Vec::new();
    doc.save_to(&mut rewritten).expect("réécriture");
    append_revision(pdf, &rewritten, None).expect("révision")
}

fn first(pdf: &[u8]) -> SignatureInfo {
    signatures(pdf, None).expect("données de test")[0]
        .signature
        .clone()
        .expect("données de test")
}

fn add_text(doc: &mut Document) {
    let page = *doc.get_pages().get(&1).expect("données de test");
    let added = doc.add_object(Stream::new(Dictionary::new(), b"BT (Ajout) Tj ET".to_vec()));
    let contents = doc
        .get_dictionary(page)
        .expect("données de test")
        .get(b"Contents")
        .expect("données de test")
        .clone();
    doc.get_dictionary_mut(page)
        .expect("données de test")
        .set("Contents", vec![contents, Object::Reference(added)]);
}

#[test]
fn a_second_signature_is_a_permitted_change() {
    let once = signed(&two_pages(false), "Claire", Lock::FormsAndSignatures);
    let twice = signed(&once, "Hugo", Lock::None);
    let info = first(&twice);
    assert_eq!(info.changes, vec![Change::Signature]);
    assert!(info.changes_permitted);
}

#[test]
fn filling_a_field_is_permitted_by_a_forms_lock() {
    let certified = signed(&form_fixture(), "Claire", Lock::FormsAndSignatures);
    let filled = revised(&certified, |doc| {
        let field = doc
            .objects
            .iter()
            .find(|(_, o)| {
                o.as_dict().is_ok_and(|d| {
                    d.get(b"FT").and_then(Object::as_name).ok() == Some(b"Tx".as_slice())
                })
            })
            .map(|(id, _)| *id)
            .expect("champ texte");
        doc.get_dictionary_mut(field)
            .unwrap()
            .set("V", Object::string_literal("Nouvelle valeur"));
    });
    let info = first(&filled);
    assert_eq!(info.changes, vec![Change::FormFill]);
    assert!(info.changes_permitted);
}

#[test]
fn page_content_breaks_a_forms_lock_but_not_an_approval() {
    let certified = signed(&two_pages(false), "Claire", Lock::FormsAndSignatures);
    let info = first(&revised(&certified, add_text));
    assert_eq!(info.changes, vec![Change::PageContent]);
    assert!(!info.changes_permitted);

    let approved = signed(&two_pages(false), "Claire", Lock::None);
    let info = first(&revised(&approved, add_text));
    assert_eq!(info.changes, vec![Change::PageContent]);
    assert!(
        info.changes_permitted,
        "sans certification, rien n'est interdit"
    );
}

#[test]
fn notes_and_properties_are_told_apart() {
    let certified = signed(&two_pages(false), "Claire", Lock::FormsAndSignatures);
    let annotated = revised(&certified, |doc| {
        let page = *doc.get_pages().get(&2).unwrap();
        let note = doc.add_object(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Text",
            "Rect" => vec![10.into(), 10.into(), 30.into(), 30.into()],
            "Contents" => Object::string_literal("Note"),
        });
        doc.get_dictionary_mut(page)
            .unwrap()
            .set("Annots", vec![Object::Reference(note)]);
        let info = doc.add_object(dictionary! { "Title" => Object::string_literal("Contrat") });
        doc.trailer.set("Info", info);
    });
    let info = first(&annotated);
    assert_eq!(info.changes, vec![Change::Annotation]);
    assert!(!info.changes_permitted);
}

#[test]
fn nothing_after_the_signature_means_no_change() {
    let info = first(&signed(&two_pages(false), "Claire", Lock::Everything));
    assert!(info.changes.is_empty());
    assert!(info.changes_permitted);
}
