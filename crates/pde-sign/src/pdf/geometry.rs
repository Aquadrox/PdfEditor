//! Cadre d'une signature : de la page affichée (origine en bas à gauche de
//! la zone visible, page tournée comme à l'écran) vers l'espace utilisateur
//! du PDF (`/CropBox` décalée, `/Rotate`).

use lopdf::{Dictionary, Object, ObjectId};

use super::source::Source;

/// Profondeur maximale de l'arbre des pages pour les attributs hérités.
const MAX_DEPTH: usize = 32;

/// Zone visible (`/CropBox`, sinon `/MediaBox`) et rotation d'une page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PageFrame {
    pub visible: [f32; 4],
    /// Quarts de tour dans le sens des aiguilles d'une montre (0 à 3).
    pub quarter_turns: u8,
}

impl PageFrame {
    pub fn of(source: &Source, page: ObjectId) -> PageFrame {
        let numbers = |key: &[u8]| {
            inherited(source, page, key).and_then(|o| {
                let values = source.resolve(&o).ok()?.as_array().ok()?.clone();
                let numbers: Vec<f32> = values
                    .iter()
                    .filter_map(|v| source.resolve(v).ok()?.as_float().ok())
                    .collect();
                <[f32; 4]>::try_from(numbers).ok()
            })
        };
        let visible = numbers(b"CropBox")
            .or_else(|| numbers(b"MediaBox"))
            .unwrap_or([0.0, 0.0, 612.0, 792.0]);
        let rotate = inherited(source, page, b"Rotate")
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        PageFrame {
            visible: [
                visible[0].min(visible[2]),
                visible[1].min(visible[3]),
                visible[0].max(visible[2]),
                visible[1].max(visible[3]),
            ],
            quarter_turns: (rotate.rem_euclid(360) / 90) as u8,
        }
    }

    /// Point de la page affichée → point de l'espace utilisateur.
    fn to_user(self, (u, v): (f32, f32)) -> (f32, f32) {
        let [x0, y0, x1, y1] = self.visible;
        match self.quarter_turns {
            1 => (x1 - v, y0 + u),
            2 => (x1 - u, y1 - v),
            3 => (x0 + v, y1 - u),
            _ => (x0 + u, y0 + v),
        }
    }

    /// Cadre affiché `[x0 y0 x1 y1]` → cadre de l'espace utilisateur.
    pub fn rect_to_user(self, rect: [f32; 4]) -> [f32; 4] {
        let (ax, ay) = self.to_user((rect[0], rect[1]));
        let (bx, by) = self.to_user((rect[2], rect[3]));
        [ax.min(bx), ay.min(by), ax.max(bx), ay.max(by)]
    }

    /// Cadre de l'espace utilisateur → cadre affiché (pour la vérification).
    pub fn rect_to_display(self, rect: [f32; 4]) -> [f32; 4] {
        let [x0, y0, x1, y1] = self.visible;
        let to_display = |(x, y): (f32, f32)| match self.quarter_turns {
            1 => (y - y0, x1 - x),
            2 => (x1 - x, y1 - y),
            3 => (y1 - y, x - x0),
            _ => (x - x0, y - y0),
        };
        let (au, av) = to_display((rect[0], rect[1]));
        let (bu, bv) = to_display((rect[2], rect[3]));
        [au.min(bu), av.min(bv), au.max(bu), av.max(bv)]
    }
}

/// Attribut de la page, ou hérité de ses `/Parent`.
fn inherited(source: &Source, page: ObjectId, key: &[u8]) -> Option<Object> {
    let mut current: Dictionary = source.document.get_dictionary(page).ok()?.clone();
    for _ in 0..MAX_DEPTH {
        if let Ok(value) = current.get(key) {
            return Some(value.clone());
        }
        let parent = current.get(b"Parent").ok()?.as_reference().ok()?;
        current = source.document.get_dictionary(parent).ok()?.clone();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(quarter_turns: u8) -> PageFrame {
        PageFrame {
            visible: [10.0, 20.0, 110.0, 220.0],
            quarter_turns,
        }
    }

    #[test]
    fn upright_page_is_shifted_by_its_crop_box() {
        assert_eq!(
            frame(0).rect_to_user([0.0, 0.0, 50.0, 10.0]),
            [10.0, 20.0, 60.0, 30.0]
        );
    }

    #[test]
    fn rotated_pages_round_trip() {
        for turns in 0..4 {
            let rect = [5.0, 7.0, 40.0, 30.0];
            let user = frame(turns).rect_to_user(rect);
            assert_eq!(frame(turns).rect_to_display(user), rect, "{turns}");
        }
    }

    #[test]
    fn quarter_turn_maps_bottom_left_to_top_left_of_the_page() {
        // Page tournée d'un quart de tour : le bas gauche affiché est le
        // coin (x1, y0) de l'espace utilisateur.
        assert_eq!(
            frame(1).rect_to_user([0.0, 0.0, 1.0, 1.0]),
            [109.0, 20.0, 110.0, 21.0]
        );
    }
}
