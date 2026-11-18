//! Outils communs aux tests de signature : PDF fabriqués, identités de test
//! générées à l'exécution (aucune clé dans le dépôt), outils indépendants
//! (`pdfsig` de Poppler, `qpdf`) pour contre-vérifier nos fichiers.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;

use lopdf::{Dictionary, Document, Object, Stream, dictionary};
use pde_sign::{Identity, Subject};

/// 18/11/2026 12:00 UTC.
pub const NOW: u64 = 1_795_003_200;

pub fn identity(name: &str) -> Identity {
    Identity::self_signed(
        &Subject {
            name: name.into(),
            email: Some("test@example.org".into()),
            organization: Some("Essais".into()),
        },
        NOW,
    )
    .expect("identité de test")
}

/// PDF de deux pages ; `modern` : flux d'objets et flux de références croisées.
pub fn two_pages(modern: bool) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let mut kids = Vec::new();
    for text in ["Contrat de prestation", "Page deux"] {
        let content = format!("BT /F1 24 Tf 72 720 Td ({text}) Tj ET");
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Contents" => content_id,
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font_id } },
        });
        kids.push(Object::Reference(page_id));
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => 2,
        }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let mut out = Vec::new();
    if modern {
        doc.save_modern(&mut out).expect("enregistrement");
    } else {
        doc.save_to(&mut out).expect("enregistrement");
    }
    out
}

/// PDF d'une page avec un champ de signature vide « Client » (widget
/// fusionné au champ) placé en bas de page.
pub fn with_empty_field() -> Vec<u8> {
    let mut doc = lopdf::Document::load_mem(&two_pages(false)).expect("relecture");
    let page_id = *doc.get_pages().get(&1).expect("page 1");
    let field_id = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "FT" => "Sig",
        "T" => Object::string_literal("Client"),
        "F" => 4,
        "Rect" => vec![300.into(), 60.into(), 520.into(), 120.into()],
        "P" => page_id,
    });
    doc.get_dictionary_mut(page_id)
        .expect("page")
        .set("Annots", vec![Object::Reference(field_id)]);
    let form_id = doc.add_object(dictionary! { "Fields" => vec![Object::Reference(field_id)] });
    let root = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .expect("racine");
    doc.get_dictionary_mut(root)
        .expect("catalogue")
        .set("AcroForm", form_id);
    let mut out = Vec::new();
    doc.save_to(&mut out).expect("enregistrement");
    out
}

/// PDF de référence de pde-pdf (formulaire écrit à la main).
pub fn form_fixture() -> Vec<u8> {
    std::fs::read(fixtures_dir().join("form.pdf")).expect("form.pdf")
}

pub fn simple_fixture() -> Vec<u8> {
    std::fs::read(fixtures_dir().join("simple.pdf")).expect("simple.pdf")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pde-pdf/tests/fixtures")
}

/// Fichier temporaire propre au test.
pub fn write_temp(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pde-sign-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dossier temporaire");
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("écriture");
    path
}

/// Outil externe disponible ? Absent : test sauté, sauf si la CI l'exige
/// (`PDE_REQUIRE_PDF_TOOLS=1`).
fn tool_available(tool: &str) -> bool {
    let found = Command::new(tool).arg("--version").output().is_ok();
    if !found && std::env::var_os("PDE_REQUIRE_PDF_TOOLS").is_some() {
        panic!("{tool} est exigé (PDE_REQUIRE_PDF_TOOLS) mais introuvable");
    }
    found
}

/// Sortie de `pdfsig` (Poppler), ou `None` s'il n'est pas installé.
pub fn pdfsig(name: &str, bytes: &[u8]) -> Option<String> {
    if !tool_available("pdfsig") {
        return None;
    }
    let path = write_temp(name, bytes);
    let output = Command::new("pdfsig").arg(&path).output().expect("pdfsig");
    let _ = std::fs::remove_file(&path);
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `qpdf --check` accepte le fichier (code 0, ou 3 = avertissements seulement).
pub fn assert_qpdf_accepts(name: &str, bytes: &[u8]) {
    if !tool_available("qpdf") {
        return;
    }
    let path = write_temp(name, bytes);
    let output = Command::new("qpdf")
        .arg("--check")
        .arg(&path)
        .output()
        .expect("qpdf");
    let _ = std::fs::remove_file(&path);
    let code = output.status.code();
    assert!(
        matches!(code, Some(0)),
        "qpdf --check refuse {name} ({code:?}) :\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
