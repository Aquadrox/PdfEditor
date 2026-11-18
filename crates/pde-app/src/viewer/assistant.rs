//! Propositions de l'assistant (IA-15) sur le document : le texte visé est
//! cherché dans les pages de la portée, l'aperçu montre où l'action portera,
//! et les propositions acceptées sont appliquées en une seule opération de
//! l'historique (une annulation les défait toutes).

use pde_core::document::DocumentId;
use pde_core::edit::{Element, MarkKind};
use pde_core::search::{find, match_rects};
use serde::Serialize;
use serde_json::json;

use super::Viewer;
use super::edit::{invalid, view};
use super::pages::{PagesView, document_view};
use crate::actions::assistant::{Direction, Proposal};
use crate::error::AppError;
use crate::text_view::BoxView;

/// Zone d'une page où porte une proposition (origine en haut à gauche).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Preview {
    pub page: usize,
    pub area: BoxView,
}

/// Proposition vérifiée, prête à montrer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProposalView {
    pub proposal: Proposal,
    pub summary: String,
    /// Pages concernées (rangs à partir de 0).
    pub pages: Vec<usize>,
    pub previews: Vec<Preview>,
    /// Suppression réelle de contenu : confirmée à part.
    pub redaction: bool,
}

/// Ce qu'une proposition fera, en coordonnées de l'interface.
enum Resolved {
    Elements(Vec<Element>),
    Rotate(Vec<usize>, u8),
    Remove(Vec<usize>),
}

/// Taille de l'icône d'une note, en points.
const NOTE_SIZE: f64 = 20.0;

impl Viewer {
    /// Occurrences de `text` sur ces pages : page et zones de chacune.
    fn occurrences(
        &self,
        doc: DocumentId,
        text: &str,
        pages: &[usize],
    ) -> Result<Vec<(usize, Vec<BoxView>)>, AppError> {
        let sizes = self.page_sizes(doc)?;
        let mut found = Vec::new();
        for &page in pages {
            let Some(&size) = sizes.get(page) else {
                continue;
            };
            let content = self.text(doc, page)?;
            for range in find(&content, text) {
                let boxes = match_rects(&content, range)
                    .iter()
                    .map(|r| BoxView::new(r, size))
                    .collect();
                found.push((page, boxes));
            }
        }
        Ok(found)
    }

    /// Vérifie la proposition sur le document : pages dans la portée,
    /// texte trouvé ; renvoie ce qu'elle fera et son aperçu.
    fn resolve(
        &self,
        id: u64,
        proposal: &Proposal,
        scope: &[usize],
        author: &str,
    ) -> Result<(Resolved, Vec<Preview>), String> {
        let doc = DocumentId(id);
        let pages = proposal.pages_in(scope)?;
        let search = |text: &str| {
            self.occurrences(doc, text, &pages)
                .map_err(|e| e.message)
                .and_then(|found| {
                    if found.is_empty() {
                        Err(format!(
                            "« {} » introuvable dans la portée choisie",
                            text.trim()
                        ))
                    } else {
                        Ok(found)
                    }
                })
        };
        let previews = |found: &[(usize, Vec<BoxView>)]| {
            found
                .iter()
                .flat_map(|(page, boxes)| {
                    boxes.iter().map(|area| Preview {
                        page: *page,
                        area: area.clone(),
                    })
                })
                .collect::<Vec<_>>()
        };
        Ok(match proposal {
            Proposal::Redact { text, .. } | Proposal::Highlight { text, .. } => {
                let found = search(text)?;
                let elements = found
                    .iter()
                    .map(|(page, boxes)| match proposal {
                        Proposal::Highlight { style, .. } => element(json!({
                            "kind": "markup",
                            "page": page,
                            "mark": style.unwrap_or(MarkKind::Highlight),
                            "areas": areas(boxes),
                        })),
                        _ => element(
                            json!({ "kind": "redact", "page": page, "areas": areas(boxes) }),
                        ),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                (Resolved::Elements(elements), previews(&found))
            }
            Proposal::AddNote { text, near, .. } => {
                let page = pages.first().copied().unwrap_or_default();
                // Près du texte nommé s'il est sur la page, sinon en haut à gauche.
                let anchor = match near.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
                    Some(near) => self
                        .occurrences(doc, near, &[page])
                        .map_err(|e| e.message)?
                        .into_iter()
                        .next()
                        .and_then(|(_, boxes)| boxes.into_iter().next()),
                    None => None,
                };
                let width = self.page_sizes(doc).map_err(|e| e.message)?[page].width;
                let (x, y) = anchor.map_or((36.0, 36.0), |b| {
                    ((b.x + b.width + 4.0).min(width - NOTE_SIZE), b.y)
                });
                let note = element(json!({
                    "kind": "note",
                    "page": page,
                    "top_left": { "x": x, "y": y },
                    "text": text.trim(),
                    "author": author,
                }))?;
                let area = BoxView {
                    x,
                    y,
                    width: NOTE_SIZE,
                    height: NOTE_SIZE,
                };
                (Resolved::Elements(vec![note]), vec![Preview { page, area }])
            }
            Proposal::RotatePages { direction, .. } => {
                let turns = match direction {
                    Direction::Right => 1,
                    Direction::Left => 3,
                };
                (Resolved::Rotate(pages, turns), Vec::new())
            }
            Proposal::RemovePages { .. } => (Resolved::Remove(pages), Vec::new()),
        })
    }

    /// Proposition prête à montrer, ou la raison de son refus.
    pub fn preview_proposal(
        &self,
        id: u64,
        proposal: Proposal,
        scope: &[usize],
    ) -> Result<ProposalView, String> {
        let (resolved, previews) = self.resolve(id, &proposal, scope, "")?;
        let pages = match &resolved {
            Resolved::Rotate(pages, _) | Resolved::Remove(pages) => pages.clone(),
            Resolved::Elements(_) => {
                let mut pages: Vec<usize> = previews.iter().map(|p| p.page).collect();
                pages.dedup();
                pages
            }
        };
        Ok(ProposalView {
            summary: proposal.summary(),
            redaction: proposal.is_redaction(),
            proposal,
            pages,
            previews,
        })
    }

    /// Applique les propositions acceptées en une opération ; si l'une
    /// échoue, aucune n'est appliquée. `author` : auteur des notes.
    pub fn apply_proposals(
        &self,
        id: u64,
        proposals: &[Proposal],
        scope: &[usize],
        author: &str,
    ) -> Result<PagesView, AppError> {
        let mut elements = Vec::new();
        let mut rotations = Vec::new();
        let mut removed = Vec::new();
        for proposal in proposals {
            match self
                .resolve(id, proposal, scope, author)
                .map_err(invalid)?
                .0
            {
                Resolved::Elements(list) => elements.extend(list),
                Resolved::Rotate(pages, turns) => rotations.push((pages, turns)),
                Resolved::Remove(pages) => removed.extend(pages),
            }
        }
        self.with_document(id, |open, doc| {
            let checked = elements
                .iter()
                .map(|e| self.checked(open, e))
                .collect::<Result<Vec<_>, _>>()?;
            // Ajouts d'abord (leurs zones viennent des pages actuelles),
            // puis rotations, puis suppressions.
            let ((), change) = open
                .edits
                .grouped(|edits| {
                    if !checked.is_empty() {
                        edits.add_many(checked);
                    }
                    for (pages, turns) in &rotations {
                        edits.rotate_pages(pages, *turns)?;
                    }
                    if !removed.is_empty() {
                        edits.remove_pages(&removed)?;
                    }
                    Ok(())
                })
                .map_err(invalid)?;
            self.commit(open, doc, change)?;
            Ok(PagesView {
                document: document_view(open, doc),
                edit: view(open),
                selected: Vec::new(),
            })
        })
    }
}

/// Zones de texte trouvées → zones d'un caviardage ou d'un marquage.
fn areas(boxes: &[BoxView]) -> serde_json::Value {
    boxes
        .iter()
        .map(
            |b| json!({ "top_left": { "x": b.x, "y": b.y }, "width": b.width, "height": b.height }),
        )
        .collect()
}

/// Élément décrit en JSON (valeurs par défaut de ses champs comprises).
fn element(value: serde_json::Value) -> Result<Element, String> {
    serde_json::from_value(value).map_err(|e| format!("élément invalide : {e}"))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pde_core::edit::Element;

    use super::*;
    use crate::fake::{FakeEngine, SIMPLE_PDF};

    /// Document de deux pages : « Bonjour monde », « le monde entier ».
    fn open(name: &str) -> (Viewer, u64, std::path::PathBuf) {
        let viewer = Viewer::new(Arc::new(FakeEngine::default()), 1024);
        let path =
            std::env::temp_dir().join(format!("pde-assist-{name}-{}.pdf", std::process::id()));
        std::fs::write(&path, SIMPLE_PDF).unwrap();
        let id = viewer.open_file(&path, None).unwrap().id;
        (viewer, id, path)
    }

    fn redact(text: &str, pages: Vec<usize>) -> Proposal {
        Proposal::Redact {
            text: text.into(),
            pages,
        }
    }

    #[test]
    fn text_is_found_in_the_scope_and_previewed() {
        let (viewer, id, path) = open("preview");
        let view = viewer
            .preview_proposal(id, redact("monde", vec![]), &[0, 1])
            .unwrap();
        assert_eq!(view.pages, [0, 1]);
        assert_eq!(view.previews.len(), 2);
        assert!(view.redaction && view.summary.starts_with("Caviarder « monde »"));
        assert!(
            view.previews
                .iter()
                .all(|p| p.area.width > 0.0 && p.area.y < 842.0)
        );

        // Portée réduite à la première page : une seule occurrence.
        let view = viewer
            .preview_proposal(id, redact("monde", vec![]), &[0])
            .unwrap();
        assert_eq!(view.pages, [0]);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn missing_text_or_page_outside_scope_is_refused() {
        let (viewer, id, path) = open("refused");
        let missing = viewer.preview_proposal(id, redact("Dupont", vec![]), &[0, 1]);
        assert!(missing.unwrap_err().contains("« Dupont » introuvable"));
        let outside = viewer.preview_proposal(id, redact("monde", vec![2]), &[0]);
        assert!(outside.unwrap_err().contains("hors de la portée"));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn accepted_proposals_are_one_undoable_operation() {
        let (viewer, id, path) = open("apply");
        let proposals = [
            redact("Bonjour", vec![1]),
            Proposal::Highlight {
                text: "entier".into(),
                pages: vec![],
                style: Some(MarkKind::Underline),
            },
            Proposal::AddNote {
                page: 2,
                text: "À relire".into(),
                near: Some("entier".into()),
            },
            Proposal::RotatePages {
                pages: vec![1],
                direction: Direction::Left,
            },
        ];
        // La note reste sur la page 2 : la rotation vise la page 1, qui ne porte qu'un caviardage.
        let result = viewer.apply_proposals(id, &proposals, &[0, 1], "Claire");
        let view = result.unwrap();
        let kinds: Vec<&str> = view
            .edit
            .elements
            .iter()
            .map(|e| match &e.element {
                Element::Redact(_) => "caviardage",
                Element::Markup(_) => "marquage",
                Element::Note(n) => {
                    assert_eq!((n.author.as_str(), n.text.as_str()), ("Claire", "À relire"));
                    "note"
                }
                _ => "autre",
            })
            .collect();
        assert_eq!(kinds, ["caviardage", "marquage", "note"]);
        // Page tournée d'un quart de tour : largeur et hauteur échangées.
        assert_eq!(view.document.pages[0].width, 842.0);

        // Une annulation défait tout.
        let undone = viewer.undo(id).unwrap();
        assert!(undone.elements.is_empty() && !undone.can_undo);
        assert_eq!(viewer.document_view(id).unwrap().pages[0].width, 595.0);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn one_failing_proposal_applies_nothing() {
        let (viewer, id, path) = open("atomic");
        let proposals = [
            redact("Bonjour", vec![]),
            Proposal::RemovePages { pages: vec![1, 2] },
        ];
        let error = viewer
            .apply_proposals(id, &proposals, &[0, 1], "")
            .unwrap_err();
        assert!(error.message.contains("au moins une page"), "{error}");
        let view = viewer.edit_view(id).unwrap();
        assert!(view.elements.is_empty() && !view.can_undo);
        std::fs::remove_file(path).unwrap();
    }
}
