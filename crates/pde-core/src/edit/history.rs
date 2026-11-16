//! Éléments ajoutés et historique des opérations (annuler, rétablir),
//! avec le suivi « modifié depuis le dernier enregistrement ».

use std::collections::BTreeMap;

use crate::document::PageIndex;

use super::element::{Element, ElementId};

/// Opération sur les éléments ; chacune connaît son inverse.
#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    Add {
        id: ElementId,
        element: Element,
    },
    Update {
        id: ElementId,
        before: Element,
        after: Element,
    },
    Remove {
        id: ElementId,
        element: Element,
    },
}

impl Operation {
    fn inverse(&self) -> Operation {
        match self.clone() {
            Operation::Add { id, element } => Operation::Remove { id, element },
            Operation::Remove { id, element } => Operation::Add { id, element },
            Operation::Update { id, before, after } => Operation::Update {
                id,
                before: after,
                after: before,
            },
        }
    }

    /// Pages dont l'affichage change (deux pour un élément changé de page).
    fn pages(&self) -> Vec<PageIndex> {
        let mut pages = match self {
            Operation::Add { element, .. } | Operation::Remove { element, .. } => {
                vec![element.page()]
            }
            Operation::Update { before, after, .. } => vec![before.page(), after.page()],
        };
        pages.dedup();
        pages
    }
}

/// Effet d'une opération : pages à redessiner.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Change {
    pub pages: Vec<PageIndex>,
}

/// Éléments du document et historique.
#[derive(Debug, Clone, Default)]
pub struct Edits {
    elements: BTreeMap<ElementId, Element>,
    next_id: u64,
    /// Opérations faites, chacune avec son numéro (pour l'état enregistré).
    done: Vec<(u64, Operation)>,
    undone: Vec<(u64, Operation)>,
    next_seq: u64,
    /// Numéro de la dernière opération au moment de l'enregistrement.
    saved: u64,
}

impl Edits {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn elements(&self) -> impl Iterator<Item = (ElementId, &Element)> {
        self.elements.iter().map(|(id, e)| (*id, e))
    }

    pub fn get(&self, id: ElementId) -> Option<&Element> {
        self.elements.get(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn can_undo(&self) -> bool {
        !self.done.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// Le document diffère de son dernier enregistrement (ou de l'ouverture).
    pub fn is_dirty(&self) -> bool {
        self.current_seq() != self.saved
    }

    /// Le document vient d'être enregistré dans son état actuel.
    pub fn mark_saved(&mut self) {
        self.saved = self.current_seq();
    }

    fn current_seq(&self) -> u64 {
        self.done.last().map_or(0, |(seq, _)| *seq)
    }

    pub fn add(&mut self, element: Element) -> (ElementId, Change) {
        self.next_id += 1;
        let id = ElementId(self.next_id);
        let change = self.record(Operation::Add { id, element });
        (id, change)
    }

    /// Remplace un élément ; sans effet (ni historique) s'il est identique.
    pub fn update(&mut self, id: ElementId, element: Element) -> Result<Change, String> {
        let before = self.get(id).ok_or_else(|| unknown(id))?.clone();
        if before == element {
            return Ok(Change::default());
        }
        Ok(self.record(Operation::Update {
            id,
            before,
            after: element,
        }))
    }

    pub fn remove(&mut self, id: ElementId) -> Result<Change, String> {
        let element = self.get(id).ok_or_else(|| unknown(id))?.clone();
        Ok(self.record(Operation::Remove { id, element }))
    }

    /// Annule la dernière opération ; `None` s'il n'y en a pas.
    pub fn undo(&mut self) -> Option<Change> {
        let (seq, operation) = self.done.pop()?;
        let change = self.apply(&operation.inverse());
        self.undone.push((seq, operation));
        Some(change)
    }

    /// Refait la dernière opération annulée ; `None` s'il n'y en a pas.
    pub fn redo(&mut self) -> Option<Change> {
        let (seq, operation) = self.undone.pop()?;
        let change = self.apply(&operation);
        self.done.push((seq, operation));
        Some(change)
    }

    /// Défait la dernière opération et l'oublie (elle n'a pas pu être
    /// appliquée au document) : ni « rétablir », ni trace dans l'historique.
    pub fn rollback(&mut self) -> Option<Change> {
        let (_, operation) = self.done.pop()?;
        Some(self.apply(&operation.inverse()))
    }

    /// Nouvelle opération : appliquée, gardée, et les opérations annulées
    /// ne peuvent plus être refaites.
    fn record(&mut self, operation: Operation) -> Change {
        let change = self.apply(&operation);
        self.next_seq += 1;
        self.done.push((self.next_seq, operation));
        self.undone.clear();
        change
    }

    fn apply(&mut self, operation: &Operation) -> Change {
        match operation.clone() {
            Operation::Add { id, element } => {
                self.elements.insert(id, element);
            }
            Operation::Update { id, after, .. } => {
                self.elements.insert(id, after);
            }
            Operation::Remove { id, .. } => {
                self.elements.remove(&id);
            }
        }
        Change {
            pages: operation.pages(),
        }
    }
}

fn unknown(id: ElementId) -> String {
    format!("élément {} introuvable", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{TextBox, TextStyle};
    use crate::geometry::Point;

    fn text(page: PageIndex, content: &str) -> Element {
        Element::Text(TextBox {
            page,
            top_left: Point::new(72.0, 700.0),
            width: 200.0,
            text: content.into(),
            style: TextStyle::default(),
        })
    }

    fn texts(edits: &Edits) -> Vec<String> {
        edits
            .elements()
            .map(|(_, e)| match e {
                Element::Text(t) => t.text.clone(),
            })
            .collect()
    }

    #[test]
    fn add_update_remove_with_pages_to_redraw() {
        let mut edits = Edits::new();
        let (a, change) = edits.add(text(0, "a"));
        assert_eq!(change.pages, [0]);
        // Déplacé sur une autre page : les deux pages changent.
        assert_eq!(edits.update(a, text(2, "a")).unwrap().pages, [0, 2]);
        assert_eq!(edits.remove(a).unwrap().pages, [2]);
        assert!(edits.is_empty());
        assert!(edits.remove(a).is_err());
    }

    #[test]
    fn undo_and_redo_restore_each_state() {
        let mut edits = Edits::new();
        let (a, _) = edits.add(text(0, "un"));
        edits.update(a, text(0, "deux")).unwrap();
        let (_, _) = edits.add(text(1, "trois"));
        assert_eq!(texts(&edits), ["deux", "trois"]);

        edits.undo();
        assert_eq!(texts(&edits), ["deux"]);
        edits.undo();
        assert_eq!(texts(&edits), ["un"]);
        edits.undo();
        assert!(edits.is_empty());
        assert_eq!(edits.undo(), None);

        edits.redo();
        edits.redo();
        assert_eq!(texts(&edits), ["deux"]);
        edits.redo();
        assert_eq!(texts(&edits), ["deux", "trois"]);
        assert_eq!(edits.redo(), None);
    }

    #[test]
    fn new_operation_clears_redo() {
        let mut edits = Edits::new();
        edits.add(text(0, "a"));
        edits.undo();
        assert!(edits.can_redo());
        edits.add(text(0, "b"));
        assert!(!edits.can_redo());
        assert_eq!(texts(&edits), ["b"]);
    }

    #[test]
    fn identical_update_is_not_recorded() {
        let mut edits = Edits::new();
        let (a, _) = edits.add(text(0, "a"));
        assert_eq!(edits.update(a, text(0, "a")).unwrap(), Change::default());
        edits.undo();
        assert!(edits.is_empty(), "un seul « annuler » suffit");
    }

    #[test]
    fn dirty_follows_the_saved_state() {
        let mut edits = Edits::new();
        assert!(!edits.is_dirty());
        let (a, _) = edits.add(text(0, "a"));
        assert!(edits.is_dirty());
        edits.mark_saved();
        assert!(!edits.is_dirty());
        edits.update(a, text(0, "b")).unwrap();
        assert!(edits.is_dirty());
        edits.undo();
        assert!(!edits.is_dirty(), "retour à l'état enregistré");
        // Même nombre d'opérations, mais une autre : modifié.
        edits.update(a, text(0, "c")).unwrap();
        assert!(edits.is_dirty());
    }

    #[test]
    fn rollback_forgets_the_failed_operation() {
        let mut edits = Edits::new();
        let (a, _) = edits.add(text(0, "a"));
        edits.update(a, text(0, "b")).unwrap();
        assert_eq!(edits.rollback().unwrap().pages, [0]);
        assert_eq!(texts(&edits), ["a"]);
        assert!(!edits.can_redo());
        edits.undo();
        assert!(edits.is_empty());
    }

    #[test]
    fn ids_are_never_reused() {
        let mut edits = Edits::new();
        let (a, _) = edits.add(text(0, "a"));
        edits.undo();
        let (b, _) = edits.add(text(0, "b"));
        assert_ne!(a, b);
    }
}
