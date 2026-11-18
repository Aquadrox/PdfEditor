//! Traduction posée sur le document (IA-06, IA-07) : chaque bloc de texte
//! d'origine (une ligne) reçoit sa traduction dans son cadre, avec son
//! corps réduit si elle est plus longue, jusqu'à un minimum ; au-delà elle
//! déborde, et c'est compté. En mode bilingue, chaque page de la portée est
//! suivie d'une copie traduite. Une seule opération de l'historique.

use pde_core::document::PageSize;
use pde_core::edit::{Area, Element, OriginalText, TextStyle};
use pde_core::geometry::Point;
use serde::{Deserialize, Serialize};

use super::Viewer;
use super::edit::{invalid, view};
use super::original_text::TextBlockView;
use super::pages::{PagesView, document_view};
use crate::edit_view::flip;
use crate::error::AppError;

/// Où va la traduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationMode {
    /// Le texte des pages est remplacé (« Enregistrer sous » garde l'original).
    Replace,
    /// Chaque page est suivie de sa copie traduite.
    Bilingual,
    /// Texte traduit dans le panneau, document inchangé.
    Text,
}

/// Bloc de la portée à traduire.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceBlock {
    pub page: usize,
    pub block: TextBlockView,
}

/// Résultat posé : document décrit à nouveau, et blocs qui débordent.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TranslatedView {
    pub pages: PagesView,
    pub blocks: usize,
    /// Blocs dont la traduction dépasse leur cadre même au corps minimal.
    pub overflowing: usize,
}

/// Corps qui fait tenir `text` dans `width`, sans descendre sous `min`
/// (ni grossir) ; `true` si le texte déborde encore.
pub fn fitted_size(text: &str, width: f64, style: &TextStyle, min: f64) -> (f64, bool) {
    let font = pde_fonts::embedded(style.font, style.bold, style.italic);
    let measured = font.text_width(text, style.size);
    // Marge de 2 % : les largeurs Noto et celles de la police d'origine diffèrent un peu.
    if measured <= width * 1.02 || measured <= 0.0 {
        return (style.size, false);
    }
    let floor = min.min(style.size);
    let size = (style.size * width / measured).max(floor);
    let overflowing = style.size * width / measured < floor;
    // Au dixième de point, vers le bas.
    ((size * 10.0).floor() / 10.0, overflowing)
}

impl Viewer {
    /// Blocs de texte d'origine des pages (dans l'ordre des pages).
    pub fn source_blocks(&self, id: u64, pages: &[usize]) -> Result<Vec<SourceBlock>, AppError> {
        let mut blocks = Vec::new();
        for &page in pages {
            for block in self.page_text_blocks(id, page)? {
                blocks.push(SourceBlock { page, block });
            }
        }
        Ok(blocks)
    }

    /// Pose les traductions (`(bloc, texte)`) selon `mode` (pas `Text`).
    pub fn apply_translation(
        &self,
        id: u64,
        mode: TranslationMode,
        translated: &[(SourceBlock, String)],
        min_size: f64,
    ) -> Result<TranslatedView, AppError> {
        if mode == TranslationMode::Text {
            return Err(invalid("traduction en texte seul : rien à poser"));
        }
        let mut overflowing = 0;
        let mut elements = Vec::with_capacity(translated.len());
        for (source, text) in translated {
            let block = &source.block;
            let mut style = block.style.clone();
            let (size, overflow) = fitted_size(text, block.bounds.width, &style, min_size);
            style.size = size;
            overflowing += usize::from(overflow);
            elements.push(Element::OriginalText(OriginalText {
                page: source.page,
                objects: block.objects.clone(),
                original: Area {
                    top_left: Point::new(block.bounds.x, block.bounds.y),
                    width: block.bounds.width,
                    height: block.bounds.height,
                },
                before: block.text.clone(),
                text: text.trim().to_owned(),
                style,
            }));
        }
        let mut pages: Vec<usize> = translated.iter().map(|(s, _)| s.page).collect();
        pages.sort_unstable();
        pages.dedup();
        self.with_document(id, |open, doc| {
            let ((), change) = open
                .edits
                .grouped(|edits| {
                    // Bilingue : les copies (juste après chaque page) reçoivent la traduction.
                    let target: Vec<usize> = match mode {
                        TranslationMode::Bilingual => edits.duplicate_pages(&pages)?.0,
                        _ => pages.clone(),
                    };
                    let sizes: Vec<PageSize> = edits.page_sizes();
                    let mut placed = Vec::with_capacity(elements.len());
                    for element in &elements {
                        let index = pages
                            .iter()
                            .position(|p| *p == element.page())
                            .unwrap_or_default();
                        let page = target[index];
                        let element = element.clone().with_page(page);
                        element.validate(sizes.len(), None)?;
                        placed.push(flip(&element, sizes[page]));
                    }
                    edits.add_many(placed);
                    Ok(())
                })
                .map_err(invalid)?;
            self.commit(open, doc, change)?;
            Ok(TranslatedView {
                pages: PagesView {
                    document: document_view(open, doc),
                    edit: view(open),
                    selected: Vec::new(),
                },
                blocks: elements.len(),
                overflowing,
            })
        })
    }
}

#[cfg(test)]
mod tests;
