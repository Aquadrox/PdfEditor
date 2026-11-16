//! Outils partagés par les tests d'intégration de pde-pdf.

#![allow(dead_code)] // chaque fichier de test n'utilise pas tout

use std::path::PathBuf;

use pde_core::engine::{OpenedDocument, PdfEngine};
use pde_core::render::Bitmap;
use pde_pdf::PdfiumEngine;

pub const SIMPLE: &[u8] = include_bytes!("../fixtures/simple.pdf");
pub const ENCRYPTED: &[u8] = include_bytes!("../fixtures/encrypted.pdf");
/// Mot de passe fictif du PDF chiffré de test (voir make_fixtures.py).
pub const TEST_PASSWORD: &str = "mot-de-passe-de-test";

pub fn engine() -> &'static PdfiumEngine {
    PdfiumEngine::global().expect("PDFium absent : lancer scripts/fetch-pdfium.sh")
}

pub fn open_simple() -> OpenedDocument {
    engine()
        .open(SIMPLE.to_vec(), None)
        .expect("simple.pdf doit s'ouvrir")
}

/// Part des pixels nettement différents tolérée (anticrénelage selon le
/// système : quelques pixels de bord de lettre peuvent varier).
const MAX_DIFFERENT_RATIO: f64 = 0.002;
/// Écart d'une composante au-delà duquel un pixel compte comme différent.
const CHANNEL_TOLERANCE: u8 = 48;

fn reference_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/expected")
        .join(name)
}

/// Compare un rendu à son image de référence ; `PDE_UPDATE_EXPECTED=1`
/// l'enregistre comme nouvelle référence.
pub fn assert_matches_reference(bitmap: &Bitmap, name: &str) {
    let path = reference_path(name);
    if std::env::var_os("PDE_UPDATE_EXPECTED").is_some() {
        let file = std::fs::File::create(&path).expect("écriture de la référence");
        let mut encoder = png::Encoder::new(file, bitmap.width(), bitmap.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("en-tête PNG");
        writer.write_image_data(bitmap.rgba()).expect("pixels PNG");
        return;
    }
    let file = std::fs::File::open(&path)
        .unwrap_or_else(|_| panic!("référence {name} absente : PDE_UPDATE_EXPECTED=1 cargo test"));
    let mut reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .expect("PNG");
    let mut expected = vec![0; reader.output_buffer_size().expect("taille PNG")];
    let info = reader.next_frame(&mut expected).expect("pixels PNG");
    assert_eq!(
        (info.width, info.height),
        (bitmap.width(), bitmap.height()),
        "taille de {name}"
    );
    let different = expected
        .chunks(4)
        .zip(bitmap.rgba().chunks(4))
        .filter(|(a, b)| {
            a.iter()
                .zip(b.iter())
                .any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE)
        })
        .count();
    let ratio = different as f64 / f64::from(info.width * info.height);
    assert!(
        ratio <= MAX_DIFFERENT_RATIO,
        "{name} : {:.3} % de pixels différents (tolérance {:.3} %)",
        ratio * 100.0,
        MAX_DIFFERENT_RATIO * 100.0
    );
}
