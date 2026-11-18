use pde_core::edit::Element;

use super::super::testing::Session;
use super::*;

/// Faux moteur : page 1, un bloc « Bonjour PdfEditor » de 218 pt en corps 24.
fn source(s: &Session) -> Vec<SourceBlock> {
    let blocks = s.viewer.source_blocks(s.id, &[0]).unwrap();
    assert_eq!(blocks.len(), 1);
    blocks
}

#[test]
fn longer_translations_are_shrunk_down_to_the_minimum() {
    let style = TextStyle {
        size: 24.0,
        ..TextStyle::default()
    };
    assert_eq!(fitted_size("Hello", 218.0, &style, 6.0), (24.0, false));
    let (size, overflow) = fitted_size("Hello PdfEditor, how are you today", 218.0, &style, 6.0);
    assert!((6.0..24.0).contains(&size) && !overflow, "{size}");
    let (size, overflow) = fitted_size(&"very long text ".repeat(40), 218.0, &style, 6.0);
    assert_eq!((size, overflow), (6.0, true));
    // Minimum au-dessus du corps d'origine : le corps d'origine sert de plancher.
    let small = TextStyle {
        size: 5.0,
        ..TextStyle::default()
    };
    assert_eq!(
        fitted_size(&"x".repeat(500), 10.0, &small, 6.0),
        (5.0, true)
    );
}

#[test]
fn replace_mode_rewrites_blocks_in_place_as_one_operation() {
    let s = Session::new("translate-replace");
    let blocks = source(&s);
    let translated = vec![(
        blocks[0].clone(),
        "Hello PdfEditor, how are you today".to_owned(),
    )];
    let view = s
        .viewer
        .apply_translation(s.id, TranslationMode::Replace, &translated, 6.0)
        .unwrap();
    assert_eq!((view.blocks, view.overflowing), (1, 0));
    let element = &view.pages.edit.elements[0].element;
    let Element::OriginalText(block) = element else {
        panic!("bloc d'origine attendu : {element:?}");
    };
    assert_eq!(block.text, "Hello PdfEditor, how are you today");
    assert_eq!(block.before, "Bonjour PdfEditor");
    assert!(
        block.style.size < 24.0,
        "corps réduit : {}",
        block.style.size
    );
    assert_eq!(view.pages.document.pages.len(), 2, "aucune page ajoutée");
    s.viewer.undo(s.id).unwrap();
    assert!(s.viewer.edit_view(s.id).unwrap().elements.is_empty());
}

#[test]
fn bilingual_mode_adds_a_translated_copy_after_each_page() {
    let s = Session::new("translate-bilingual");
    let blocks = source(&s);
    let translated = vec![(blocks[0].clone(), "Hello".to_owned())];
    let view = s
        .viewer
        .apply_translation(s.id, TranslationMode::Bilingual, &translated, 6.0)
        .unwrap();
    assert_eq!(
        view.pages.document.pages.len(),
        3,
        "copie ajoutée après la page 1"
    );
    let pages: Vec<usize> = view
        .pages
        .edit
        .elements
        .iter()
        .map(|e| e.element.page())
        .collect();
    assert_eq!(pages, [1], "traduction sur la copie, l'original intact");
    s.viewer.undo(s.id).unwrap();
    assert_eq!(s.viewer.document_view(s.id).unwrap().pages.len(), 2);
    assert!(!s.viewer.edit_view(s.id).unwrap().can_undo);
}

#[test]
fn text_mode_changes_nothing() {
    let s = Session::new("translate-text");
    let blocks = source(&s);
    let translated = vec![(blocks[0].clone(), "Hello".to_owned())];
    assert!(
        s.viewer
            .apply_translation(s.id, TranslationMode::Text, &translated, 6.0)
            .is_err()
    );
    assert!(s.viewer.edit_view(s.id).unwrap().elements.is_empty());
}
