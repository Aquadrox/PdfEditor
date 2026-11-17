//! Caviardage (E-07) : zones dont le contenu est réellement supprimé du
//! document — texte, images, tracés et annotations qui les touchent — et
//! remplacé par un aplat noir. Le moteur l'applique à chaque pose, après
//! les autres éléments : ce qui est affiché est ce qui sera enregistré.

use serde::{Deserialize, Serialize};

use super::annotation::{Area, MAX_MARKUP_AREAS};
use crate::document::PageIndex;
use crate::geometry::Rect;

/// Côté minimal d'une zone caviardée (points).
pub const MIN_REDACT_SIDE: f64 = 1.0;

/// Zones caviardées d'une page (une par morceau de ligne choisi, ou un
/// rectangle tracé).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedactBox {
    pub page: PageIndex,
    pub areas: Vec<Area>,
}

impl RedactBox {
    pub fn bounds(&self) -> Rect {
        self.areas
            .iter()
            .map(Area::rect)
            .reduce(|a, b| a.union(&b))
            .unwrap_or_default()
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.areas.is_empty() || self.areas.len() > MAX_MARKUP_AREAS {
            return Err(format!("de 1 à {MAX_MARKUP_AREAS} zones à caviarder"));
        }
        let valid = self.areas.iter().all(|a| {
            [a.top_left.x, a.top_left.y, a.width, a.height]
                .iter()
                .all(|v| v.is_finite())
                && a.width >= MIN_REDACT_SIDE
                && a.height >= MIN_REDACT_SIDE
        });
        if valid {
            Ok(())
        } else {
            Err("zone à caviarder invalide".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn area(x: f64, y: f64, width: f64, height: f64) -> Area {
        Area {
            top_left: Point::new(x, y),
            width,
            height,
        }
    }

    #[test]
    fn bounds_cover_every_area() {
        let redact = RedactBox {
            page: 0,
            areas: vec![area(10.0, 100.0, 50.0, 10.0), area(40.0, 80.0, 100.0, 20.0)],
        };
        let bounds = redact.bounds();
        assert_eq!(bounds.min(), Point::new(10.0, 60.0));
        assert_eq!(bounds.max(), Point::new(140.0, 100.0));
        assert_eq!(redact.validate(), Ok(()));
    }

    #[test]
    fn empty_tiny_or_invalid_areas_are_refused() {
        let with = |areas| RedactBox { page: 0, areas }.validate();
        assert!(with(Vec::new()).is_err());
        assert!(with(vec![area(0.0, 0.0, 0.5, 10.0)]).is_err());
        assert!(with(vec![area(f64::NAN, 0.0, 10.0, 10.0)]).is_err());
    }
}
