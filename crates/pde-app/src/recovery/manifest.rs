//! Contenu d'une sauvegarde automatique : un dossier par document, avec
//! `manifest.json` (fichier d'origine, plan des pages s'il a changé, PDF
//! insérés désignés par leur chemin et leur empreinte, éléments ajoutés) et
//! une image par
//! fichier (JPEG tel quel, sinon PNG). Le PDF lui-même n'est jamais copié.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pde_core::edit::{Element, ImageData, ImageId, Images, PagePlan};
use pde_core::render::Bitmap;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, ErrorKind};
use crate::files::write_atomic;
use crate::images::decode;
use crate::page_png::encode_png;

/// Version du format : une version inconnue n'est pas relue. Le format 2
/// ajoute le plan des pages ; il n'est écrit que si ce plan a changé, pour
/// qu'une version précédente de l'application ne l'ignore pas sans le dire.
const FORMAT: u32 = 1;
const FORMAT_WITH_PAGES: u32 = 2;
const MANIFEST: &str = "manifest.json";

/// État d'un document à sauvegarder (pris sous le verrou des documents).
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub path: PathBuf,
    pub file_name: String,
    /// SHA-256 du PDF d'origine, en hexadécimal.
    pub digest: String,
    /// Pages du document d'origine.
    pub page_count: usize,
    /// Plan des pages, s'il diffère de l'original.
    pub pages: Option<PagePlan>,
    /// PDF insérés que le plan désigne (jamais copiés).
    pub imports: Vec<ImportRecord>,
    /// Éléments, coordonnées de `pde-core` (origine en bas).
    pub elements: Vec<Element>,
    /// Images désignées par les éléments.
    pub images: Images,
}

/// Sauvegarde relue, prête à être réappliquée.
#[derive(Debug, Clone)]
pub struct Recovered {
    pub path: PathBuf,
    pub file_name: String,
    pub digest: String,
    pub page_count: usize,
    pub saved_at: u64,
    pub pages: Option<PagePlan>,
    pub imports: Vec<ImportRecord>,
    pub elements: Vec<Element>,
    pub images: Images,
}

/// PDF inséré : repris seulement si le fichier a le même contenu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportRecord {
    pub id: u64,
    pub path: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: u32,
    path: PathBuf,
    file_name: String,
    source_sha256: String,
    page_count: usize,
    /// Secondes depuis 1970.
    saved_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pages: Option<PagePlan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    imports: Vec<ImportRecord>,
    elements: Vec<Element>,
    images: Vec<u64>,
}

fn io_error(e: std::io::Error) -> AppError {
    AppError::new(ErrorKind::Io, format!("sauvegarde automatique : {e}"))
}

fn image_file(dir: &Path, id: ImageId) -> PathBuf {
    dir.join(format!("image-{}.bin", id.0))
}

/// Écrit la sauvegarde : images d'abord (une fois chacune), puis le
/// manifeste, remplacé d'un coup : il ne désigne jamais une image absente.
pub fn write(dir: &Path, snapshot: &Snapshot, saved_at: u64) -> Result<(), AppError> {
    fs::create_dir_all(dir).map_err(io_error)?;
    for (id, asset) in &snapshot.images {
        let file = image_file(dir, *id);
        if file.exists() {
            continue;
        }
        let bytes = match asset.data() {
            ImageData::Jpeg(bytes) => bytes.clone(),
            ImageData::Rgba(rgba) => {
                let bitmap = Bitmap::new(asset.width(), asset.height(), rgba.clone())?;
                encode_png(&bitmap)?
            }
        };
        write_atomic(&file, &bytes)?;
    }
    let manifest = Manifest {
        format: if snapshot.pages.is_some() {
            FORMAT_WITH_PAGES
        } else {
            FORMAT
        },
        path: snapshot.path.clone(),
        file_name: snapshot.file_name.clone(),
        source_sha256: snapshot.digest.clone(),
        page_count: snapshot.page_count,
        saved_at,
        pages: snapshot.pages.clone(),
        imports: snapshot.imports.clone(),
        elements: snapshot.elements.clone(),
        images: snapshot.images.keys().map(|id| id.0).collect(),
    };
    let json = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| AppError::new(ErrorKind::Io, e.to_string()))?;
    write_atomic(&dir.join(MANIFEST), &json)?;
    remove_unused_images(dir, &snapshot.images);
    Ok(())
}

/// Images supprimées du document depuis : leur fichier n'a plus d'usage.
fn remove_unused_images(dir: &Path, images: &Images) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let id = name
            .to_str()
            .and_then(|n| n.strip_prefix("image-"))
            .and_then(|n| n.strip_suffix(".bin"))
            .and_then(|n| n.parse::<u64>().ok());
        if id.is_some_and(|id| !images.contains_key(&ImageId(id))) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn read_manifest(dir: &Path) -> Result<Manifest, AppError> {
    let invalid = |m: String| AppError::new(ErrorKind::InvalidFile, m);
    let bytes = fs::read(dir.join(MANIFEST)).map_err(io_error)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|e| invalid(format!("sauvegarde automatique illisible : {e}")))?;
    if manifest.format != FORMAT && manifest.format != FORMAT_WITH_PAGES {
        return Err(invalid(format!(
            "sauvegarde automatique d'un autre format ({})",
            manifest.format
        )));
    }
    Ok(manifest)
}

/// Résumé pour la liste des récupérations (sans lire les images).
pub fn summary(dir: &Path) -> Option<(String, PathBuf, u64, usize)> {
    let m = read_manifest(dir).ok()?;
    Some((m.file_name, m.path, m.saved_at, m.elements.len()))
}

pub fn read(dir: &Path) -> Result<Recovered, AppError> {
    let manifest = read_manifest(dir)?;
    let mut images = Images::new();
    for id in manifest.images {
        let id = ImageId(id);
        let bytes = fs::read(image_file(dir, id)).map_err(io_error)?;
        images.insert(id, Arc::new(decode(bytes)?));
    }
    Ok(Recovered {
        path: manifest.path,
        file_name: manifest.file_name,
        digest: manifest.source_sha256,
        page_count: manifest.page_count,
        saved_at: manifest.saved_at,
        pages: manifest.pages,
        imports: manifest.imports,
        elements: manifest.elements,
        images,
    })
}

#[cfg(test)]
mod tests {
    use pde_core::edit::{ImageAsset, ImageBox, TextBox, TextStyle};
    use pde_core::geometry::Point;

    use super::*;

    fn snapshot() -> Snapshot {
        let mut images = Images::new();
        let pixels = (0..4 * 3 * 2).map(|i| i as u8).collect();
        images.insert(
            ImageId(2),
            Arc::new(ImageAsset::rgba(3, 2, pixels).unwrap()),
        );
        Snapshot {
            path: PathBuf::from("/docs/contrat.pdf"),
            file_name: "contrat.pdf".into(),
            digest: "ab".repeat(32),
            page_count: 3,
            pages: Some(PagePlan::new(vec![
                pde_core::edit::PlannedPage::original(2),
                pde_core::edit::PlannedPage::imported(pde_core::edit::ImportId(4), 0),
            ])),
            imports: vec![ImportRecord {
                id: 4,
                path: PathBuf::from("/docs/annexe.pdf"),
                sha256: "cd".repeat(32),
            }],
            elements: vec![
                Element::Text(TextBox {
                    page: 0,
                    top_left: Point::new(72.0, 700.0),
                    width: 200.0,
                    text: "Annexe B".into(),
                    style: TextStyle::default(),
                }),
                Element::Image(ImageBox {
                    page: 1,
                    top_left: Point::new(10.0, 500.0),
                    width: 30.0,
                    height: 20.0,
                    image: ImageId(2),
                    rotation: 1,
                    opacity: 0.5,
                    keep_ratio: false,
                }),
            ],
            images,
        }
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pde-manifest-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn written_snapshot_reads_back_identical() {
        let dir = temp("aller-retour");
        let s = snapshot();
        write(&dir, &s, 1_700_000_000).unwrap();
        let r = read(&dir).unwrap();
        assert_eq!(
            (r.path, r.file_name, r.digest.clone()),
            (s.path, s.file_name, s.digest)
        );
        assert_eq!((r.page_count, r.saved_at), (3, 1_700_000_000));
        assert_eq!(r.elements, s.elements);
        assert_eq!(r.pages, s.pages);
        assert_eq!(r.imports, s.imports);
        // Pixels relus à l'identique (PNG sans perte).
        assert_eq!(r.images[&ImageId(2)], s.images[&ImageId(2)]);
        assert_eq!(
            summary(&dir),
            Some((
                "contrat.pdf".into(),
                PathBuf::from("/docs/contrat.pdf"),
                1_700_000_000,
                2
            ))
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn plan_only_when_pages_changed_with_its_format() {
        let dir = temp("format");
        let mut s = snapshot();
        write(&dir, &s, 1).unwrap();
        let text = fs::read_to_string(dir.join(MANIFEST)).unwrap();
        assert!(text.contains("\"format\": 2"));
        s.pages = None;
        s.imports.clear();
        write(&dir, &s, 1).unwrap();
        let text = fs::read_to_string(dir.join(MANIFEST)).unwrap();
        assert!(text.contains("\"format\": 1") && !text.contains("\"pages\""));
        assert_eq!(read(&dir).unwrap().pages, None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn removed_images_lose_their_file() {
        let dir = temp("images");
        let mut s = snapshot();
        write(&dir, &s, 1).unwrap();
        assert!(image_file(&dir, ImageId(2)).exists());
        s.images.clear();
        s.elements.truncate(1);
        write(&dir, &s, 2).unwrap();
        assert!(!image_file(&dir, ImageId(2)).exists());
        assert!(read(&dir).unwrap().images.is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn broken_or_foreign_manifests_are_refused() {
        let dir = temp("illisible");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(MANIFEST), b"{").unwrap();
        assert_eq!(read(&dir).unwrap_err().kind, ErrorKind::InvalidFile);
        let s = snapshot();
        write(&dir, &s, 1).unwrap();
        let text = fs::read_to_string(dir.join(MANIFEST)).unwrap();
        fs::write(
            dir.join(MANIFEST),
            text.replace("\"format\": 2", "\"format\": 9"),
        )
        .unwrap();
        assert!(read(&dir).unwrap_err().message.contains("autre format"));
        assert_eq!(summary(&dir), None);
        fs::remove_dir_all(dir).unwrap();
    }
}
