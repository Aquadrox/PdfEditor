//! Plan des pages (P-01 à P-03) : ordre des pages, pages blanches et
//! pages d'autres PDF ajoutées, rotations.
//!
//! Comme les éléments, le plan décrit le résultat à partir du document
//! d'origine, qui n'est jamais modifié : le moteur repart de l'original et
//! range ses pages selon le plan avant d'y poser les éléments.

use serde::{Deserialize, Serialize};

use crate::document::{PageIndex, PageSize};

use super::import::{ImportId, PageSources};

/// Côté d'une page blanche accepté (points) : de 1 mm environ à 5 m
/// (limite des lecteurs PDF).
pub const BLANK_SIDE_RANGE: std::ops::RangeInclusive<f64> = 3.0..=14_400.0;
/// Pages au plus dans un document arrangé.
pub const MAX_PAGES: usize = 100_000;

/// D'où vient une page du document arrangé.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PageSource {
    /// Page du document d'origine (numéro dans l'original, à partir de 0).
    Original { index: PageIndex },
    /// Page blanche ; taille en points.
    Blank { width: f64, height: f64 },
    /// Page d'un PDF inséré (numéro dans ce PDF, à partir de 0).
    Imported { file: ImportId, index: PageIndex },
}

/// Page du document arrangé.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlannedPage {
    pub source: PageSource,
    /// Quarts de tour ajoutés, dans le sens des aiguilles d'une montre (0 à 3).
    #[serde(default)]
    pub rotation: u8,
}

impl PlannedPage {
    pub const fn original(index: PageIndex) -> Self {
        Self {
            source: PageSource::Original { index },
            rotation: 0,
        }
    }

    pub const fn blank(size: PageSize) -> Self {
        Self {
            source: PageSource::Blank {
                width: size.width,
                height: size.height,
            },
            rotation: 0,
        }
    }

    pub const fn imported(file: ImportId, index: PageIndex) -> Self {
        Self {
            source: PageSource::Imported { file, index },
            rotation: 0,
        }
    }

    /// Taille affichée, rotation ajoutée comprise ; `None` si la page
    /// désignée n'existe pas.
    pub fn size(&self, sources: &PageSources) -> Option<PageSize> {
        let size = match self.source {
            PageSource::Original { index } => *sources.originals.get(index)?,
            PageSource::Blank { width, height } => PageSize::new(width, height),
            PageSource::Imported { file, index } => sources.imported_page(file, index)?,
        };
        Some(if self.rotation % 2 == 1 {
            PageSize::new(size.height, size.width)
        } else {
            size
        })
    }
}

/// Pages du document arrangé, dans l'ordre.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PagePlan {
    pages: Vec<PlannedPage>,
}

impl PagePlan {
    /// Les pages d'origine, dans leur ordre, sans rotation ajoutée.
    pub fn identity(count: usize) -> Self {
        Self {
            pages: (0..count).map(PlannedPage::original).collect(),
        }
    }

    pub fn new(pages: Vec<PlannedPage>) -> Self {
        Self { pages }
    }

    pub fn pages(&self) -> &[PlannedPage] {
        &self.pages
    }

    pub fn len(&self) -> usize {
        self.pages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Le plan laisse le document d'origine tel quel.
    pub fn is_identity(&self, original_count: usize) -> bool {
        *self == Self::identity(original_count)
    }

    /// Taille affichée de chaque page.
    pub fn sizes(&self, sources: &PageSources) -> Vec<PageSize> {
        self.pages.iter().filter_map(|p| p.size(sources)).collect()
    }

    /// Place actuelle d'une page d'origine (sa première occurrence) ;
    /// `None` si elle a été retirée. Sert aux signets.
    pub fn position_of(&self, original: PageIndex) -> Option<PageIndex> {
        self.pages
            .iter()
            .position(|p| matches!(p.source, PageSource::Original { index } if index == original))
    }

    /// Vérifie un plan (relu d'une sauvegarde automatique, ou reçu par le
    /// moteur) : chaque page désignée existe.
    pub fn validate(&self, sources: &PageSources) -> Result<(), String> {
        if self.pages.is_empty() {
            return Err("un document garde au moins une page".into());
        }
        if self.pages.len() > MAX_PAGES {
            return Err(format!("plus de {MAX_PAGES} pages"));
        }
        for page in &self.pages {
            if page.rotation > 3 {
                return Err("rotation invalide".into());
            }
            match page.source {
                PageSource::Blank { width, height } => check_blank_size(width, height)?,
                PageSource::Original { index } if index >= sources.originals.len() => {
                    return Err(format!("la page d'origine {} n'existe pas", index + 1));
                }
                PageSource::Imported { file, index }
                    if sources.imported_page(file, index).is_none() =>
                {
                    return Err(format!("la page {} du PDF inséré n'existe pas", index + 1));
                }
                PageSource::Original { .. } | PageSource::Imported { .. } => {}
            }
        }
        Ok(())
    }
}

/// Taille d'une page blanche acceptée.
pub fn check_blank_size(width: f64, height: f64) -> Result<(), String> {
    if [width, height].iter().all(|v| BLANK_SIDE_RANGE.contains(v)) {
        Ok(())
    } else {
        Err(format!(
            "taille de page entre {} et {} points",
            BLANK_SIDE_RANGE.start(),
            BLANK_SIDE_RANGE.end()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources(originals: &[PageSize]) -> PageSources {
        PageSources::new(originals.to_vec())
    }

    const A4: PageSize = PageSize::new(595.0, 842.0);
    const LANDSCAPE: PageSize = PageSize::new(842.0, 595.0);

    #[test]
    fn identity_keeps_the_original() {
        let plan = PagePlan::identity(3);
        assert!(plan.is_identity(3));
        assert!(!plan.is_identity(2));
        assert_eq!(
            plan.sizes(&sources(&[A4, LANDSCAPE, A4])),
            [A4, LANDSCAPE, A4]
        );
        assert_eq!(plan.position_of(2), Some(2));
    }

    #[test]
    fn sizes_follow_order_rotation_and_blank_pages() {
        let mut turned = PlannedPage::original(0);
        turned.rotation = 1;
        let plan = PagePlan::new(vec![
            PlannedPage::original(1),
            turned,
            PlannedPage::blank(PageSize::new(100.0, 200.0)),
        ]);
        assert!(!plan.is_identity(2));
        assert_eq!(
            plan.sizes(&sources(&[A4, LANDSCAPE])),
            [LANDSCAPE, LANDSCAPE, PageSize::new(100.0, 200.0)]
        );
        assert_eq!(plan.position_of(0), Some(1));
        assert_eq!(plan.position_of(5), None);
    }

    #[test]
    fn invalid_plans_are_explained() {
        let one = sources(&[A4]);
        assert!(PagePlan::new(Vec::new()).validate(&one).is_err());
        assert!(PagePlan::identity(2).validate(&one).is_err());
        let mut turned = PlannedPage::original(0);
        turned.rotation = 4;
        assert!(PagePlan::new(vec![turned]).validate(&one).is_err());
        let tiny = PlannedPage::blank(PageSize::new(1.0, 100.0));
        assert!(PagePlan::new(vec![tiny]).validate(&one).is_err());
        assert_eq!(PagePlan::identity(2).validate(&sources(&[A4, A4])), Ok(()));
        // Page d'un PDF inséré : le PDF et la page doivent exister.
        let imported = PagePlan::new(vec![PlannedPage::imported(ImportId(1), 1)]);
        assert!(imported.validate(&one).is_err());
        let mut with_import = sources(&[A4]);
        with_import.imported.insert(ImportId(1), vec![A4]);
        assert!(imported.validate(&with_import).is_err());
        with_import
            .imported
            .insert(ImportId(1), vec![A4, LANDSCAPE]);
        assert_eq!(imported.validate(&with_import), Ok(()));
        assert_eq!(imported.sizes(&with_import), [LANDSCAPE]);
    }

    #[test]
    fn json_for_the_recovery_manifest() {
        let plan = PagePlan::new(vec![
            PlannedPage::original(1),
            PlannedPage::blank(PageSize::new(100.0, 200.0)),
            PlannedPage::imported(ImportId(3), 0),
        ]);
        let json = serde_json::to_value(&plan).unwrap();
        assert_eq!(json[0]["source"]["kind"], "original");
        assert_eq!(json[0]["source"]["index"], 1);
        assert_eq!(json[1]["source"]["kind"], "blank");
        assert_eq!(json[2]["source"]["kind"], "imported");
        assert_eq!(json[2]["source"]["file"], 3);
        let back: PagePlan = serde_json::from_value(json).unwrap();
        assert_eq!(back, plan);
    }
}
