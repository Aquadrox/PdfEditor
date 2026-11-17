//! Texte d'origine récrit (E-05). Le bloc est vérifié (objets texte à la
//! place attendue), puis :
//! - texte vide : ses objets sont retirés ;
//! - police d'origine capable d'écrire le nouveau texte : le premier objet
//!   le reçoit (même police, corps, couleur, position), les autres sont
//!   retirés ;
//! - sinon : tous ses objets sont retirés et le texte est écrit au même
//!   endroit avec la police standard la plus proche (police remplacée).
//!
//! Une police embarquée est souvent un sous-ensemble : elle ne sait écrire
//! à coup sûr que les caractères déjà écrits avec elle sur la page. Le
//! texte relu après l'écriture confirme que tout a été encodé.

use std::collections::{HashMap, HashSet};

use pde_core::edit::{MIN_WIDTH, OriginalText, TextBox};
use pde_core::engine::EngineError;
use pde_core::geometry::{Point, Rect};
use pdfium_render::prelude::{PdfPage, PdfPageObjectCommon, PdfPageObjectsCommon};

use super::blocks::run;
use super::existing::close;
use crate::convert::{internal, rect};
use crate::display::DisplayTransform;

/// Ce que devient un bloc récrit.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Rewritten {
    /// Récrit avec sa police : sa nouvelle place.
    Kept(Rect),
    /// Retiré : sa place d'origine.
    Removed(Rect),
    /// Retiré ; le texte est à écrire avec la police de remplacement.
    Replaced(TextBox),
}

/// Caractères déjà écrits sur la page avec chaque police (nom complet).
pub(super) fn known_chars(page: &PdfPage) -> HashMap<String, HashSet<char>> {
    let mut known: HashMap<String, HashSet<char>> = HashMap::new();
    for object in page.objects().iter() {
        if let Some(text) = object.as_text_object() {
            known
                .entry(text.font().name())
                .or_default()
                .extend(text.text().chars());
        }
    }
    known
}

/// La police sait écrire `text` : police non embarquée (le lecteur la
/// fournit), ou tous les caractères visibles déjà écrits avec elle.
pub(super) fn can_keep(embedded: bool, known: Option<&HashSet<char>>, text: &str) -> bool {
    !embedded
        || text
            .chars()
            .filter(|c| !c.is_whitespace())
            .all(|c| known.is_some_and(|k| k.contains(&c)))
}

/// Les objets du bloc sont des objets texte droits, à la place attendue.
pub(super) fn check(
    page: &PdfPage,
    display: &DisplayTransform,
    block: &OriginalText,
) -> Result<(), EngineError> {
    let moved = || {
        EngineError::Internal(format!(
            "le texte d'origine de la page {} n'est plus à sa place",
            block.page + 1
        ))
    };
    let mut bounds: Option<Rect> = None;
    for &index in &block.objects {
        let object = page.objects().get(index as usize).map_err(|_| moved())?;
        let found = run(&object, index, display)?.ok_or_else(moved)?;
        bounds = Some(bounds.map_or(found.bounds, |b| b.union(&found.bounds)));
    }
    match bounds {
        Some(found) if close(&found, &block.original.rect()) => Ok(()),
        _ => Err(moved()),
    }
}

/// Récrit le bloc (déjà vérifié) ; renvoie les rangs des objets à retirer
/// et ce qu'il devient. Les retraits sont faits ensuite, de la fin de la
/// page vers le début, avec ceux des autres modifications.
pub(super) fn rewrite(
    page: &mut PdfPage,
    display: &DisplayTransform,
    block: &OriginalText,
    known: &HashMap<String, HashSet<char>>,
) -> Result<(Vec<u32>, Rewritten), EngineError> {
    let original = block.original.rect();
    if block.is_removed() {
        return Ok((block.objects.clone(), Rewritten::Removed(original)));
    }
    let page_width = f64::from(page.width().value);
    let first = block.objects[0];
    let mut object = page.objects_mut().get(first as usize).map_err(internal)?;
    let text_object = object
        .as_text_object_mut()
        .ok_or_else(|| EngineError::Internal("objet texte attendu".into()))?;
    let (name, embedded) = {
        let font = text_object.font();
        (font.name(), font.is_embedded().unwrap_or(true))
    };
    // Ligne de base d'origine (le texte part de l'origine de l'objet).
    let matrix = text_object.matrix().map_err(internal)?;
    let baseline = display.display_point(f64::from(matrix.e()), f64::from(matrix.f()));
    if can_keep(embedded, known.get(&name), &block.text) {
        text_object.set_text(&block.text).map_err(internal)?;
        if text_object.text() == block.text {
            let bounds = display.rect(rect(object.bounds().map_err(internal)?.to_rect()));
            return Ok((block.objects[1..].to_vec(), Rewritten::Kept(bounds)));
        }
    }
    let ascent = super::text::ascent(block.style.font);
    let replacement = TextBox {
        page: block.page,
        top_left: Point::new(baseline.x, baseline.y + ascent * block.style.size),
        // Jusqu'au bord de la page : le texte reste sur une ligne.
        width: (page_width - baseline.x)
            .max(original.width())
            .max(MIN_WIDTH),
        text: block.text.clone(),
        style: block.style.clone(),
    };
    Ok((block.objects.clone(), Rewritten::Replaced(replacement)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_embedded_font_keeps_only_the_characters_it_already_wrote() {
        let known: HashSet<char> = "Bonjour".chars().collect();
        assert!(can_keep(true, Some(&known), "Bon jour"));
        assert!(!can_keep(true, Some(&known), "Bonsoir"));
        assert!(!can_keep(true, None, "B"));
        // Police fournie par le lecteur : toujours essayée (le texte relu tranche).
        assert!(can_keep(false, None, "Bonsoir €"));
    }
}
