//! Rendu d'une page en image : taille en pixels, limites, image produite.

use crate::document::{PageIndex, PageSize};
use crate::engine::EngineError;

/// Facteurs de zoom acceptés (1.0 = 1 pixel par point, soit 72 ppp).
pub const SCALE_RANGE: std::ops::RangeInclusive<f64> = 0.05..=16.0;
/// Plus grand côté d'une image rendue, en pixels.
pub const MAX_SIDE_PX: u32 = 16_384;
/// Plus grande image rendue (en pixels) : 64 Mpx, soit 256 Mo en RGBA.
/// Au-delà, l'interface rend la page par tuiles.
pub const MAX_AREA_PX: u64 = 64 * 1024 * 1024;

/// Demande de rendu d'une page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderRequest {
    pub page: PageIndex,
    /// Pixels par point : 1.0 = 72 ppp ; l'interface multiplie le zoom par
    /// la densité de l'écran.
    pub scale: f64,
}

/// Taille en pixels de l'image d'une page, ou erreur si la demande est hors limites.
pub fn pixel_size(page: PageSize, scale: f64) -> Result<(u32, u32), EngineError> {
    if !scale.is_finite() || !SCALE_RANGE.contains(&scale) {
        return Err(EngineError::InvalidRender(format!(
            "zoom {scale} hors de l'intervalle {} à {}",
            SCALE_RANGE.start(),
            SCALE_RANGE.end()
        )));
    }
    let side = |points: f64| (points * scale).round().max(1.0);
    let (width, height) = (side(page.width), side(page.height));
    if width > f64::from(MAX_SIDE_PX) || height > f64::from(MAX_SIDE_PX) {
        return Err(EngineError::InvalidRender(format!(
            "image de {width} × {height} px, plus grand côté limité à {MAX_SIDE_PX} px"
        )));
    }
    // Les deux côtés tiennent dans un u32 (vérifié ci-dessus).
    let (width, height) = (width as u32, height as u32);
    if u64::from(width) * u64::from(height) > MAX_AREA_PX {
        return Err(EngineError::InvalidRender(format!(
            "image de {width} × {height} px trop grande"
        )));
    }
    Ok((width, height))
}

/// Image RGBA (8 bits par canal, lignes de haut en bas, sans remplissage).
#[derive(Clone, PartialEq, Eq)]
pub struct Bitmap {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Bitmap {
    /// Vérifie que le tampon correspond exactement à la taille annoncée.
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self, EngineError> {
        let expected = u64::from(width) * u64::from(height) * 4;
        if rgba.len() as u64 != expected {
            return Err(EngineError::Internal(format!(
                "image de {width} × {height} px : {} octets reçus, {expected} attendus",
                rgba.len()
            )));
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }

    /// Couleur `[r, g, b, a]` du pixel (x, y), origine en haut à gauche.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let start = (y as usize * self.width as usize + x as usize) * 4;
        let p = self.rgba.get(start..start + 4)?;
        Some([p[0], p[1], p[2], p[3]])
    }
}

/// Le contenu de l'image n'est pas affiché : il ferait des Mo de texte.
impl std::fmt::Debug for Bitmap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bitmap({} × {})", self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A4: PageSize = PageSize::new(595.0, 842.0);

    #[test]
    fn a4_at_100_and_200_percent() {
        assert_eq!(pixel_size(A4, 1.0), Ok((595, 842)));
        assert_eq!(pixel_size(A4, 2.0), Ok((1190, 1684)));
    }

    #[test]
    fn tiny_page_keeps_at_least_one_pixel() {
        assert_eq!(pixel_size(PageSize::new(1.0, 1.0), 0.05), Ok((1, 1)));
    }

    #[test]
    fn scale_out_of_range_or_not_a_number() {
        for scale in [0.0, -1.0, 16.5, f64::NAN, f64::INFINITY] {
            assert!(pixel_size(A4, scale).is_err(), "zoom {scale}");
        }
    }

    #[test]
    fn huge_pages_are_refused() {
        // Format A0 au zoom maximal : plus de 16 384 px de côté.
        assert!(pixel_size(PageSize::new(2384.0, 3370.0), 16.0).is_err());
        // Côtés acceptables mais surface excessive (> 64 Mpx).
        assert!(pixel_size(PageSize::new(1000.0, 1000.0), 9.0).is_err());
    }

    #[test]
    fn bitmap_checks_buffer_length() {
        assert!(Bitmap::new(2, 2, vec![0; 16]).is_ok());
        assert!(Bitmap::new(2, 2, vec![0; 15]).is_err());
    }

    #[test]
    fn bitmap_pixel_access() {
        let mut rgba = vec![255; 2 * 2 * 4];
        rgba[12..16].copy_from_slice(&[1, 2, 3, 4]); // pixel (1, 1)
        let bitmap = Bitmap::new(2, 2, rgba).unwrap();
        assert_eq!(bitmap.pixel(1, 1), Some([1, 2, 3, 4]));
        assert_eq!(bitmap.pixel(0, 0), Some([255; 4]));
        assert_eq!(bitmap.pixel(2, 0), None);
        assert_eq!(format!("{bitmap:?}"), "Bitmap(2 × 2)");
    }
}
