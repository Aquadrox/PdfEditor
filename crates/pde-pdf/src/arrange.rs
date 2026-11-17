//! Pages rangées selon le plan (P-01, P-02), avant de poser les éléments.
//!
//! Une page d'origine reste le même objet PDF tant que c'est possible :
//! ses signets, liens internes et champs de formulaire la suivent. On garde
//! donc la plus longue suite de pages d'origine déjà dans le bon ordre ;
//! les autres pages (déplacées au-delà de cette suite, ou dupliquées) sont
//! des copies prises dans un second exemplaire de l'original, les pages
//! des PDF insérés sont copiées de ceux-ci, et les pages
//! blanches sont créées. Aucun appel `unsafe` : PDFium-render ne donne pas
//! accès au déplacement de pages sans recopie.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use pde_core::edit::{ImportId, Imports, PagePlan, PageSource, PageSources};
use pde_core::engine::EngineError;
use pdfium_render::prelude::{
    PdfDocument, PdfPagePaperSize, PdfPageRenderRotation, PdfPoints, Pdfium,
};

use crate::convert::internal;
use crate::reader;

/// Range les pages de `doc` (l'original tout juste chargé) selon `plan`.
/// `source` : l'original et son mot de passe, rechargé seulement si une
/// page doit être copiée ; `imports` : PDF insérés que le plan désigne.
pub(crate) fn arrange<'a>(
    pdfium: &'a Pdfium,
    doc: &mut PdfDocument<'a>,
    plan: &PagePlan,
    source: (&[u8], Option<&str>),
    imports: &Imports,
) -> Result<(), EngineError> {
    let count = page_count(doc);
    if plan.is_identity(count) {
        return Ok(());
    }
    let sources = PageSources::with_imports(reader::document_info(doc)?.page_sizes, imports);
    plan.validate(&sources).map_err(EngineError::Internal)?;
    let kept = kept_pages(plan);

    // 1. Pages d'origine absentes de la suite gardée : retirées, de la
    //    dernière à la première pour ne pas décaler les suivantes.
    let mut keep = vec![false; count];
    kept.iter().flatten().for_each(|&index| keep[index] = true);
    for index in (0..count).rev() {
        if !keep[index] {
            doc.pages()
                .get(page_index(index)?)
                .and_then(|page| page.delete())
                .map_err(internal)?;
        }
    }

    // 2. Les pages restantes sont dans l'ordre du plan : chaque autre page
    //    est insérée à sa place, de la première à la dernière.
    let mut copies: Option<PdfDocument<'a>> = None;
    let mut imported: HashMap<ImportId, PdfDocument<'a>> = HashMap::new();
    for (at, (planned, kept)) in plan.pages().iter().zip(&kept).enumerate() {
        if kept.is_some() {
            continue;
        }
        let at = page_index(at)?;
        match planned.source {
            PageSource::Original { index } => {
                if copies.is_none() {
                    let (bytes, password) = source;
                    copies = Some(
                        pdfium
                            .load_pdf_from_byte_vec(bytes.to_vec(), password)
                            .map_err(internal)?,
                    );
                }
                if let Some(copies) = &copies {
                    doc.pages_mut()
                        .copy_page_from_document(copies, page_index(index)?, at)
                        .map_err(internal)?;
                }
            }
            PageSource::Blank { width, height } => {
                let size = PdfPagePaperSize::Custom(points(width), points(height));
                doc.pages_mut()
                    .create_page_at_index(size, at)
                    .map_err(internal)?;
            }
            PageSource::Imported { file, index } => {
                // Chargé une fois par rangement, quel que soit le nombre de pages.
                let from = match imported.entry(file) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => {
                        let pdf = imports.get(&file).ok_or_else(|| {
                            EngineError::Internal(format!("PDF inséré {} absent", file.0))
                        })?;
                        entry.insert(
                            pdfium
                                .load_pdf_from_byte_vec(pdf.bytes.clone(), None)
                                .map_err(internal)?,
                        )
                    }
                };
                doc.pages_mut()
                    .copy_page_from_document(from, page_index(index)?, at)
                    .map_err(internal)?;
            }
        }
    }

    // 3. Rotations ajoutées à celle que la page avait déjà.
    for (at, planned) in plan.pages().iter().enumerate() {
        if planned.rotation == 0 {
            continue;
        }
        let mut page = doc.pages().get(page_index(at)?).map_err(internal)?;
        let current = quarter_turns(page.rotation().map_err(internal)?);
        page.set_rotation(rotation((current + planned.rotation) % 4));
    }

    let arranged = page_count(doc);
    if arranged != plan.len() {
        return Err(EngineError::Internal(format!(
            "rangement des pages : {arranged} pages au lieu de {}",
            plan.len()
        )));
    }
    Ok(())
}

/// Pour chaque page du plan, la page d'origine gardée telle quelle
/// (`Some(index)`), ou `None` si elle est à copier ou à créer : la plus
/// longue suite croissante de pages d'origine, chacune une seule fois.
pub(crate) fn kept_pages(plan: &PagePlan) -> Vec<Option<usize>> {
    let originals: Vec<(usize, usize)> = plan
        .pages()
        .iter()
        .enumerate()
        .filter_map(|(at, p)| match p.source {
            PageSource::Original { index } => Some((at, index)),
            PageSource::Blank { .. } | PageSource::Imported { .. } => None,
        })
        .collect();
    // Plus longue suite strictement croissante (algorithme « patience ») :
    // `tails[k]` = position, dans `originals`, de la fin de la meilleure
    // suite de longueur k + 1 ; `previous` permet de la reconstituer.
    let mut tails: Vec<usize> = Vec::new();
    let mut previous: Vec<Option<usize>> = vec![None; originals.len()];
    for (i, &(_, index)) in originals.iter().enumerate() {
        let length = tails.partition_point(|&t| originals[t].1 < index);
        // Page déjà vue au même rang (doublon) : la première occurrence
        // reste l'originale, la suivante sera la copie.
        if tails.get(length).is_some_and(|&t| originals[t].1 == index) {
            continue;
        }
        previous[i] = length.checked_sub(1).map(|l| tails[l]);
        if length == tails.len() {
            tails.push(i);
        } else {
            tails[length] = i;
        }
    }
    let mut kept = vec![None; plan.len()];
    let mut cursor = tails.last().copied();
    while let Some(i) = cursor {
        let (at, index) = originals[i];
        kept[at] = Some(index);
        cursor = previous[i];
    }
    kept
}

fn page_count(doc: &PdfDocument) -> usize {
    usize::try_from(doc.pages().len()).unwrap_or(0)
}

fn page_index(index: usize) -> Result<i32, EngineError> {
    i32::try_from(index).map_err(|_| EngineError::Internal(format!("page {index} hors limites")))
}

fn points(value: f64) -> PdfPoints {
    // Borné par `PagePlan::validate` (14 400 points au plus).
    PdfPoints::new(value as f32)
}

fn quarter_turns(rotation: PdfPageRenderRotation) -> u8 {
    match rotation {
        PdfPageRenderRotation::None => 0,
        PdfPageRenderRotation::Degrees90 => 1,
        PdfPageRenderRotation::Degrees180 => 2,
        PdfPageRenderRotation::Degrees270 => 3,
    }
}

fn rotation(turns: u8) -> PdfPageRenderRotation {
    match turns % 4 {
        1 => PdfPageRenderRotation::Degrees90,
        2 => PdfPageRenderRotation::Degrees180,
        3 => PdfPageRenderRotation::Degrees270,
        _ => PdfPageRenderRotation::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pde_core::document::PageSize;
    use pde_core::edit::PlannedPage;

    fn plan(pages: &[Option<usize>]) -> PagePlan {
        PagePlan::new(
            pages
                .iter()
                .map(|p| match p {
                    Some(i) => PlannedPage::original(*i),
                    None => PlannedPage::blank(PageSize::new(100.0, 100.0)),
                })
                .collect(),
        )
    }

    #[test]
    fn the_longest_ordered_run_of_pages_is_kept() {
        // Page 4 ramenée en tête : seule elle est recopiée.
        assert_eq!(
            kept_pages(&plan(&[Some(3), Some(0), Some(1), Some(2)])),
            [None, Some(0), Some(1), Some(2)]
        );
        // Doublon : la copie n'est jamais gardée deux fois.
        assert_eq!(
            kept_pages(&plan(&[Some(0), Some(0), Some(1)])),
            [Some(0), None, Some(1)]
        );
        // Pages blanches : toujours créées.
        assert_eq!(
            kept_pages(&plan(&[Some(0), None, Some(1)])),
            [Some(0), None, Some(1)]
        );
        // Ordre inversé : une seule page reste.
        let reversed = kept_pages(&plan(&[Some(2), Some(1), Some(0)]));
        assert_eq!(reversed.iter().flatten().count(), 1);
    }

    #[test]
    fn rotations_add_up() {
        for turns in 0..4 {
            assert_eq!(quarter_turns(rotation(turns)), turns);
        }
        assert_eq!(quarter_turns(rotation(5)), 1);
    }
}
