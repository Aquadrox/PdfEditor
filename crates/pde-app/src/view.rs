//! Description d'un document telle que l'interface la reçoit (JSON).

use pde_core::document::{DocumentInfo, OutlineItem};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageView {
    /// Largeur affichée en points (rotation appliquée).
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutlineView {
    pub title: String,
    /// Page à partir de 0, si le signet mène à une page du document.
    pub page: Option<usize>,
    pub children: Vec<OutlineView>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DocumentView {
    pub id: u64,
    /// Nom du fichier, sans le dossier.
    pub file_name: String,
    /// Titre des métadonnées s'il existe, sinon le nom du fichier.
    pub title: String,
    pub pages: Vec<PageView>,
    pub outline: Vec<OutlineView>,
}

impl DocumentView {
    pub fn new(id: u64, file_name: String, info: &DocumentInfo) -> Self {
        Self {
            id,
            title: info.title.clone().unwrap_or_else(|| file_name.clone()),
            file_name,
            pages: info
                .page_sizes
                .iter()
                .map(|s| PageView {
                    width: s.width,
                    height: s.height,
                })
                .collect(),
            outline: info.outline.iter().map(outline).collect(),
        }
    }
}

fn outline(item: &OutlineItem) -> OutlineView {
    OutlineView {
        title: item.title.clone(),
        page: item.page,
        children: item.children.iter().map(outline).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pde_core::document::PageSize;

    #[test]
    fn title_falls_back_to_file_name() {
        let mut info = DocumentInfo {
            page_sizes: vec![PageSize::new(595.0, 842.0)],
            title: None,
            outline: vec![OutlineItem {
                title: "Partie".into(),
                page: Some(0),
                children: vec![OutlineItem {
                    title: "Sous-partie".into(),
                    page: None,
                    children: Vec::new(),
                }],
            }],
        };
        let view = DocumentView::new(7, "devis.pdf".into(), &info);
        assert_eq!(view.title, "devis.pdf");
        assert_eq!(view.pages[0].width, 595.0);
        assert_eq!(view.outline[0].children[0].title, "Sous-partie");

        info.title = Some("Devis cuisine".into());
        assert_eq!(
            DocumentView::new(7, "devis.pdf".into(), &info).title,
            "Devis cuisine"
        );
    }

    #[test]
    fn json_shape() {
        let info = DocumentInfo {
            page_sizes: vec![PageSize::new(10.0, 20.0)],
            title: Some("T".into()),
            outline: Vec::new(),
        };
        let json = serde_json::to_string(&DocumentView::new(1, "a.pdf".into(), &info)).unwrap();
        assert_eq!(
            json,
            r#"{"id":1,"file_name":"a.pdf","title":"T","pages":[{"width":10.0,"height":20.0}],"outline":[]}"#
        );
    }
}
