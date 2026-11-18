//! Signatures manuscrites (S-03) : l'image envoyée par l'interface (PNG en
//! base64, dessinée, tapée ou importée, déjà détourée) et les modèles
//! enregistrés (« Mes signatures »), un PNG par modèle dans le dossier de
//! données.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::{ImageFormat, ImageReader, Limits};
use pde_sign::{MAX_IMAGE_SIDE, SignatureImage};
use serde::Serialize;

use crate::error::{AppError, ErrorKind};
use crate::files::write_atomic;

/// Plus gros PNG de signature accepté (une signature détourée fait
/// quelques dizaines de Ko).
const MAX_PNG_BYTES: usize = 4 * 1024 * 1024;
/// Nombre de modèles gardés.
pub const MAX_DRAWINGS: usize = 24;

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new(ErrorKind::InvalidRequest, message)
}

/// PNG venu de l'interface, en base64 (avec ou sans préfixe `data:`).
pub fn png_from_base64(text: &str) -> Result<Vec<u8>, AppError> {
    let data = text.split_once(',').map_or(text, |(_, data)| data).trim();
    if data.len() > MAX_PNG_BYTES / 3 * 4 + 4 {
        return Err(invalid("image de signature trop grande"));
    }
    STANDARD
        .decode(data)
        .map_err(|_| invalid("image de signature illisible"))
}

/// Pixels d'un PNG de signature, taille bornée avant décodage.
pub fn decode_png(png: &[u8]) -> Result<SignatureImage, AppError> {
    let mut reader = ImageReader::with_format(Cursor::new(png), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|e| invalid(format!("image de signature illisible ({e})")))?
        .into_rgba8();
    let (width, height) = decoded.dimensions();
    SignatureImage::new(width, height, decoded.into_raw()).map_err(|e| invalid(e.to_string()))
}

/// Modèle enregistré, tel que l'interface le reçoit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DrawingView {
    pub id: String,
    /// PNG en base64.
    pub png: String,
}

/// « Mes signatures » : PNG rangés dans `signatures/` du dossier de données.
pub struct DrawingStore {
    dir: PathBuf,
}

impl DrawingStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join("signatures"),
        }
    }

    /// Modèles, du plus récent au plus ancien ; un fichier illisible est ignoré.
    pub fn list(&self) -> Vec<DrawingView> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "png"))
            .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
            .collect();
        files.sort_by_key(|f| std::cmp::Reverse(f.0));
        files
            .into_iter()
            .take(MAX_DRAWINGS)
            .filter_map(|(_, path)| {
                let id = path.file_stem()?.to_string_lossy().into_owned();
                let png = std::fs::read(&path).ok()?;
                Some(DrawingView {
                    id,
                    png: STANDARD.encode(png),
                })
            })
            .collect()
    }

    /// Enregistre un modèle (le même PNG n'est gardé qu'une fois).
    pub fn save(&self, png: &[u8]) -> Result<DrawingView, AppError> {
        decode_png(png)?;
        if self.list().len() >= MAX_DRAWINGS {
            return Err(invalid(format!(
                "{MAX_DRAWINGS} signatures enregistrées au plus : supprimez-en une"
            )));
        }
        use sha2::{Digest, Sha256};
        let id: String = Sha256::digest(png)
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect();
        write_atomic(&self.dir.join(format!("{id}.png")), png)?;
        Ok(DrawingView {
            id,
            png: STANDARD.encode(png),
        })
    }

    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("signature enregistrée inconnue"));
        }
        match std::fs::remove_file(self.dir.join(format!("{id}.png"))) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(AppError::new(ErrorKind::Io, e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PNG 3 × 2 transparent avec un pixel d'encre.
    fn png() -> Vec<u8> {
        let mut pixels = image::RgbaImage::new(3, 2);
        pixels.put_pixel(1, 1, image::Rgba([20, 30, 90, 255]));
        let mut out = Vec::new();
        pixels
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn base64_png_decodes_to_rgba() {
        let encoded = format!("data:image/png;base64,{}", STANDARD.encode(png()));
        let image = decode_png(&png_from_base64(&encoded).unwrap()).unwrap();
        assert_eq!((image.width(), image.height()), (3, 2));
        assert!(png_from_base64("pas du base64 !").is_err());
        assert!(decode_png(b"\x89PNG faux").is_err());
    }

    #[test]
    fn drawings_are_saved_listed_and_deleted() {
        let dir = std::env::temp_dir().join(format!("pde-drawings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = DrawingStore::new(&dir);
        assert!(store.list().is_empty());
        let saved = store.save(&png()).unwrap();
        // Le même dessin n'est gardé qu'une fois.
        assert_eq!(store.save(&png()).unwrap().id, saved.id);
        assert_eq!(store.list(), vec![saved.clone()]);
        assert!(store.save(b"pas un png").is_err());
        store.delete(&saved.id).unwrap();
        assert!(store.list().is_empty());
        assert!(store.delete("../x").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
