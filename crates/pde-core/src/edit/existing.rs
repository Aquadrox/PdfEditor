//! Images d'origine du document (E-06) : déplacées, redimensionnées ou
//! supprimées. Une image est désignée par sa page et son rang parmi les
//! objets de la page arrangée, avant tout ajout : le moteur repartant de
//! l'original à chaque pose, ce rang ne change pas. Son cadre d'origine est
//! gardé pour vérifier que c'est bien elle, et pour l'annulation.
//!
//! Remplacer une image, c'est la supprimer et poser une image ajoutée
//! (`ImageBox`) dans son cadre, en une opération.

use serde::{Deserialize, Serialize};

use super::annotation::Area;
use crate::document::PageIndex;
use crate::geometry::Rect;

/// Côté minimal d'une image d'origine redimensionnée (points).
pub const MIN_EXISTING_SIDE: f64 = 2.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExistingImage {
    pub page: PageIndex,
    /// Rang de l'objet image parmi les objets de la page.
    pub object: u32,
    /// Cadre de l'image dans le document d'origine.
    pub original: Area,
    /// Nouveau cadre ; `None` : image supprimée.
    pub frame: Option<Area>,
}

impl ExistingImage {
    /// Place occupée : le nouveau cadre, ou l'ancien si l'image est supprimée.
    pub fn bounds(&self) -> Rect {
        self.frame.as_ref().unwrap_or(&self.original).rect()
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        let valid = |a: &Area| {
            [a.top_left.x, a.top_left.y, a.width, a.height]
                .iter()
                .all(|v| v.is_finite())
                && a.width > 0.0
                && a.height > 0.0
        };
        if !valid(&self.original) {
            return Err("cadre d'origine invalide".into());
        }
        match &self.frame {
            Some(frame)
                if !valid(frame)
                    || frame.width < MIN_EXISTING_SIDE
                    || frame.height < MIN_EXISTING_SIDE =>
            {
                Err(format!(
                    "image trop petite (au moins {MIN_EXISTING_SIDE} pt de côté)"
                ))
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn area(x: f64, y: f64, w: f64, h: f64) -> Area {
        Area {
            top_left: Point::new(x, y),
            width: w,
            height: h,
        }
    }

    fn image(frame: Option<Area>) -> ExistingImage {
        ExistingImage {
            page: 0,
            object: 3,
            original: area(100.0, 700.0, 200.0, 100.0),
            frame,
        }
    }

    #[test]
    fn bounds_follow_the_new_frame_or_the_original() {
        let moved = image(Some(area(50.0, 500.0, 100.0, 50.0)));
        assert_eq!(moved.bounds().min(), Point::new(50.0, 450.0));
        let removed = image(None);
        assert_eq!(removed.bounds().max(), Point::new(300.0, 700.0));
        assert_eq!(moved.validate(), Ok(()));
        assert_eq!(removed.validate(), Ok(()));
    }

    #[test]
    fn tiny_or_invalid_frames_are_refused() {
        assert!(image(Some(area(0.0, 10.0, 1.0, 50.0))).validate().is_err());
        assert!(
            image(Some(area(f64::NAN, 10.0, 10.0, 50.0)))
                .validate()
                .is_err()
        );
        let mut bad = image(None);
        bad.original.width = 0.0;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn json_for_the_interface() {
        let json = serde_json::to_value(super::super::Element::ExistingImage(image(None))).unwrap();
        assert_eq!(json["kind"], "existing_image");
        assert_eq!(json["object"], 3);
        assert!(json["frame"].is_null());
    }
}
