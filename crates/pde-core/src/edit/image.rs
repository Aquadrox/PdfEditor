//! Images ajoutées (E-02) : le cadre posé sur la page, et le contenu de
//! l'image, importé une fois puis partagé par toutes ses copies.
//!
//! Coordonnées : page affichée, points, origine en bas à gauche, comme le
//! reste de `pde-core`.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::document::PageIndex;
use crate::geometry::{Point, Rect};

/// Identifiant d'une image importée dans un document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ImageId(pub u64);

/// Plus petit côté accepté (points).
pub const MIN_IMAGE_SIDE: f64 = 4.0;
/// Opacité acceptée : en dessous, l'image serait invisible.
pub const OPACITY_RANGE: std::ops::RangeInclusive<f64> = 0.05..=1.0;

fn opaque() -> f64 {
    1.0
}

fn yes() -> bool {
    true
}

/// Image posée sur une page : cadre, rotation, opacité.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageBox {
    pub page: PageIndex,
    /// Coin haut gauche du cadre.
    pub top_left: Point,
    /// Taille du cadre tel qu'affiché (rotation comprise).
    pub width: f64,
    pub height: f64,
    pub image: ImageId,
    /// Quarts de tour dans le sens des aiguilles d'une montre (0 à 3).
    #[serde(default)]
    pub rotation: u8,
    #[serde(default = "opaque")]
    pub opacity: f64,
    /// Proportions gardées quand on tire un coin (choix de l'utilisateur).
    #[serde(default = "yes")]
    pub keep_ratio: bool,
}

/// Matrice PDF `[a b c d e f]` : le carré unité de l'image (u vers la
/// droite, v vers le haut) → la page affichée.
pub type Placement = [f64; 6];

impl ImageBox {
    pub fn bounds(&self) -> Rect {
        Rect::from_corners(
            Point::new(self.top_left.x, self.top_left.y - self.height),
            Point::new(self.top_left.x + self.width, self.top_left.y),
        )
    }

    /// Place de l'image dans son cadre, tournée de `rotation` quarts de
    /// tour : le haut de l'image va à droite (1), en bas (2), à gauche (3).
    pub fn placement(&self) -> Placement {
        let (x, top, w, h) = (self.top_left.x, self.top_left.y, self.width, self.height);
        let bottom = top - h;
        match self.rotation % 4 {
            0 => [w, 0.0, 0.0, h, x, bottom],
            1 => [0.0, -h, w, 0.0, x, top],
            2 => [-w, 0.0, 0.0, -h, x + w, top],
            _ => [0.0, h, -w, 0.0, x + w, bottom],
        }
    }

    pub(crate) fn validate(&self, images: Option<&Images>) -> Result<(), String> {
        let numbers = [
            self.top_left.x,
            self.top_left.y,
            self.width,
            self.height,
            self.opacity,
        ];
        if numbers.iter().any(|v| !v.is_finite()) {
            return Err("position ou taille invalide".into());
        }
        if self.width < MIN_IMAGE_SIDE || self.height < MIN_IMAGE_SIDE {
            return Err(format!("image trop petite (minimum {MIN_IMAGE_SIDE} pt)"));
        }
        if self.rotation > 3 {
            return Err("rotation invalide".into());
        }
        if !OPACITY_RANGE.contains(&self.opacity) {
            return Err("opacité entre 5 et 100 %".into());
        }
        if images.is_some_and(|images| !images.contains_key(&self.image)) {
            return Err("image inconnue : importez-la d'abord".into());
        }
        Ok(())
    }
}

/// Contenu d'une image importée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageData {
    /// Fichier JPEG, écrit tel quel dans le PDF (aucune recompression).
    Jpeg(Vec<u8>),
    /// Pixels RVBA 8 bits, ligne par ligne (PNG, WebP), transparence comprise.
    Rgba(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAsset {
    width: u32,
    height: u32,
    data: ImageData,
}

impl ImageAsset {
    /// Pixels RVBA ; refusés si leur nombre ne correspond pas à la taille.
    pub fn rgba(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        let expected = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        (width > 0 && height > 0 && rgba.len() == expected).then_some(Self {
            width,
            height,
            data: ImageData::Rgba(rgba),
        })
    }

    /// JPEG dont la taille a été lue par le décodeur.
    pub fn jpeg(width: u32, height: u32, bytes: Vec<u8>) -> Option<Self> {
        (width > 0 && height > 0 && !bytes.is_empty()).then_some(Self {
            width,
            height,
            data: ImageData::Jpeg(bytes),
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn data(&self) -> &ImageData {
        &self.data
    }
}

/// Images importées d'un document, partagées avec le moteur.
pub type Images = BTreeMap<ImageId, Arc<ImageAsset>>;

#[cfg(test)]
mod tests {
    use super::*;

    fn image_box() -> ImageBox {
        ImageBox {
            page: 0,
            top_left: Point::new(100.0, 500.0),
            width: 200.0,
            height: 100.0,
            image: ImageId(1),
            rotation: 0,
            opacity: 1.0,
            keep_ratio: true,
        }
    }

    /// Applique la matrice à un point du carré unité.
    fn apply(m: Placement, u: f64, v: f64) -> (f64, f64) {
        (m[0] * u + m[2] * v + m[4], m[1] * u + m[3] * v + m[5])
    }

    #[test]
    fn placement_fills_the_frame_for_each_rotation() {
        // Coin haut gauche de l'image (u = 0, v = 1) selon la rotation.
        let expected = [
            (100.0, 500.0),
            (300.0, 500.0),
            (300.0, 400.0),
            (100.0, 400.0),
        ];
        for (rotation, corner) in expected.into_iter().enumerate() {
            let mut b = image_box();
            b.rotation = rotation as u8;
            let m = b.placement();
            assert_eq!(apply(m, 0.0, 1.0), corner, "rotation {rotation}");
            // Les quatre coins de l'image restent dans le cadre.
            for (u, v) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let (x, y) = apply(m, u, v);
                assert!((100.0..=300.0).contains(&x) && (400.0..=500.0).contains(&y));
            }
        }
        assert_eq!(
            image_box().bounds(),
            Rect::from_corners(Point::new(100.0, 400.0), Point::new(300.0, 500.0))
        );
    }

    #[test]
    fn invalid_frames_are_explained() {
        let mut images = Images::new();
        images.insert(
            ImageId(1),
            Arc::new(ImageAsset::rgba(1, 1, vec![0; 4]).unwrap()),
        );
        assert_eq!(image_box().validate(Some(&images)), Ok(()));
        let cases: [fn(&mut ImageBox); 5] = [
            |b| b.width = 1.0,
            |b| b.height = f64::INFINITY,
            |b| b.rotation = 4,
            |b| b.opacity = 0.0,
            |b| b.image = ImageId(9),
        ];
        for change in cases {
            let mut b = image_box();
            change(&mut b);
            assert!(b.validate(Some(&images)).is_err());
        }
    }

    #[test]
    fn assets_check_their_size() {
        assert!(ImageAsset::rgba(2, 2, vec![0; 16]).is_some());
        assert!(ImageAsset::rgba(2, 2, vec![0; 15]).is_none());
        assert!(ImageAsset::rgba(0, 2, Vec::new()).is_none());
        assert!(ImageAsset::rgba(u32::MAX, u32::MAX, Vec::new()).is_none());
        assert!(ImageAsset::jpeg(3, 2, vec![0xff]).is_some());
        assert!(ImageAsset::jpeg(3, 2, Vec::new()).is_none());
    }

    #[test]
    fn json_defaults() {
        let json = r#"{"page":0,"top_left":{"x":1,"y":2},"width":10,"height":10,"image":3}"#;
        let b: ImageBox = serde_json::from_str(json).unwrap();
        assert_eq!((b.rotation, b.opacity, b.keep_ratio), (0, 1.0, true));
        assert_eq!(b.image, ImageId(3));
    }
}
