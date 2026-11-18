//! Ce qu'affiche une signature visible (S-03) : une image manuscrite
//! (dessinée, tapée ou importée, décodée par l'appelant) et des lignes de
//! texte. Une signature invisible n'affiche rien.

use crate::error::{Result, SignError};

/// Plus grand côté accepté pour l'image, en pixels.
pub const MAX_IMAGE_SIDE: u32 = 4096;

/// Image de la signature, en pixels RGBA (8 bits par composante).
#[derive(Clone, PartialEq, Eq)]
pub struct SignatureImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl std::fmt::Debug for SignatureImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SignatureImage({}×{})", self.width, self.height)
    }
}

impl SignatureImage {
    /// Image `width` × `height` ; `rgba` doit en contenir tous les pixels.
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Self> {
        let expected = u64::from(width) * u64::from(height) * 4;
        if width == 0 || height == 0 || width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
            return Err(SignError::Target(format!(
                "image de signature de {width} × {height} pixels : de 1 à {MAX_IMAGE_SIDE} pixels de côté"
            )));
        }
        if rgba.len() as u64 != expected {
            return Err(SignError::Target("image de signature incomplète".into()));
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

    /// Composantes rouge, vert, bleu, puis la transparence (`None` si l'image est opaque).
    pub(crate) fn split(&self) -> (Vec<u8>, Option<Vec<u8>>) {
        let mut rgb = Vec::with_capacity(self.rgba.len() / 4 * 3);
        let mut alpha = Vec::with_capacity(self.rgba.len() / 4);
        for pixel in self.rgba.chunks_exact(4) {
            rgb.extend_from_slice(&pixel[..3]);
            alpha.push(pixel[3]);
        }
        let opaque = alpha.iter().all(|&a| a == 255);
        (rgb, (!opaque).then_some(alpha))
    }
}

/// Apparence d'une signature visible.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Appearance {
    /// Signature manuscrite, en haut du cadre.
    pub image: Option<SignatureImage>,
    /// Lignes de texte (nom, organisation, date), sous l'image.
    pub lines: Vec<String>,
}

impl Appearance {
    /// Texte seul.
    pub fn text(lines: Vec<String>) -> Self {
        Self { image: None, lines }
    }

    /// Rien à afficher.
    pub fn is_empty(&self) -> bool {
        self.image.is_none() && self.lines.iter().all(|l| l.trim().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_and_content_are_checked() {
        assert!(SignatureImage::new(2, 1, vec![0; 8]).is_ok());
        assert!(SignatureImage::new(2, 1, vec![0; 7]).is_err());
        assert!(SignatureImage::new(0, 1, vec![]).is_err());
        assert!(SignatureImage::new(MAX_IMAGE_SIDE + 1, 1, vec![0; 4 * 4097]).is_err());
    }

    #[test]
    fn opaque_images_have_no_mask() {
        let opaque = SignatureImage::new(1, 1, vec![1, 2, 3, 255]).unwrap();
        assert_eq!(opaque.split(), (vec![1, 2, 3], None));
        let ink = SignatureImage::new(2, 1, vec![9, 9, 9, 0, 1, 2, 3, 200]).unwrap();
        assert_eq!(ink.split(), (vec![9, 9, 9, 1, 2, 3], Some(vec![0, 200])));
    }

    #[test]
    fn blank_appearance_is_empty() {
        assert!(Appearance::text(vec![" ".into()]).is_empty());
        assert!(!Appearance::text(vec!["Claire".into()]).is_empty());
    }
}
