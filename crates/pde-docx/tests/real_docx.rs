//! Un vrai document Word (`fixtures/formulaire.docx`, écrit par LibreOffice
//! à partir d'un HTML fictif) : texte, titre, liste, tableau, case.

use pde_docx::{Block, layout, read};

const DOCX: &[u8] = include_bytes!("fixtures/formulaire.docx");

#[test]
fn a_real_docx_is_read_and_laid_out() {
    let doc = read(DOCX).expect("lecture");
    let texts: Vec<String> = doc
        .blocks
        .iter()
        .filter_map(|b| match b {
            Block::Paragraph(p) => Some(p.text()),
            _ => None,
        })
        .collect();
    let all = texts.join("\n");
    for expected in [
        "Demande d'inscription",
        "Nom :",
        "Date de naissance",
        "Premier point",
        "J'accepte",
    ] {
        assert!(all.contains(expected), "{expected} absent de :\n{all}");
    }
    assert!(doc.blocks.iter().any(|b| matches!(b, Block::Table(_))));
    let out = layout(&doc);
    assert_eq!(out.pages, 1);
    assert!(out.elements.len() >= 8, "{}", out.elements.len());
    assert!(
        out.elements
            .iter()
            .all(|e| e.validate(out.pages, None).is_ok())
    );
}
