//! Formulaire (E-08, E-12) et propriétés du document (E-11) vus de la
//! visionneuse : champs du document et champs conçus réunis, placés sur
//! les pages (origine en haut) ; valeurs vérifiées et mises en forme
//! (date, montant, masque…) avant d'être gardées, champs calculés tenus à
//! jour ; aplatissement, propriétés.

use pde_core::document::{DocumentId, PageIndex};
use pde_core::edit::{
    Edits, Element, ElementId, FieldBox, FieldKind, FieldLook, FieldValue, FormField, Metadata,
    designed_fields,
};
use pde_core::engine::Edition;
use pde_core::formats::{FieldFormat, plain_number};
use serde::Serialize;

use super::edit::{invalid, view};
use super::{OpenDocument, Viewer};
use crate::edit_view::EditView;
use crate::error::AppError;
use crate::text_view::BoxView;

/// Passes au plus pour tenir à jour des calculs qui dépendent d'autres calculs.
const CALC_PASSES: usize = 8;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WidgetView {
    pub page: PageIndex,
    pub bounds: BoxView,
    /// Bouton radio : rang de son option.
    pub option: Option<usize>,
    /// Rang de l'emplacement parmi ceux du champ.
    pub index: usize,
    /// Case à cocher, bouton radio : valeur quand il est coché.
    pub export: Option<String>,
    /// Élément de conception qui le décrit (champ créé ou modifié).
    pub element: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldView {
    pub name: String,
    pub kind: FieldKind,
    pub read_only: bool,
    pub required: bool,
    pub tooltip: String,
    pub look: FieldLook,
    /// Valeur dans le document d'origine, ou valeur initiale d'un champ
    /// créé (la valeur saisie est dans EditView).
    pub value: Option<FieldValue>,
    pub widgets: Vec<WidgetView>,
    /// Champ créé dans PdfEditor (pas encore dans le fichier).
    pub created: bool,
}

/// Éléments de conception du document, par identifiant croissant.
pub(super) fn design_boxes(edits: &Edits) -> Vec<(ElementId, &FieldBox)> {
    edits
        .elements()
        .filter_map(|(id, e)| match e {
            Element::Field(field) => Some((id, &**field)),
            _ => None,
        })
        .collect()
}

impl Viewer {
    /// Champs du document arrangé, lus une fois par plan des pages.
    pub(super) fn document_fields(
        &self,
        open: &mut OpenDocument,
        doc: DocumentId,
    ) -> Result<(), AppError> {
        if open.form_fields.is_none() {
            let edition = Edition {
                plan: open.edits.plan().clone(),
                imports: open.imports.clone(),
                ..Edition::default()
            };
            open.form_fields = Some(self.engine.form_fields(doc, edition)?);
        }
        Ok(())
    }

    /// Champs du document et champs conçus réunis.
    pub(super) fn merged_fields(
        &self,
        open: &mut OpenDocument,
        doc: DocumentId,
    ) -> Result<Vec<FormField>, AppError> {
        self.document_fields(open, doc)?;
        merged(open)
    }

    /// Champs du formulaire, tels qu'ils seront écrits.
    pub fn form_fields(&self, id: u64) -> Result<Vec<FieldView>, AppError> {
        self.with_document(id, |open, doc| {
            let fields = self.merged_fields(open, doc)?;
            Ok(fields.iter().map(|f| field_view(open, f)).collect())
        })
    }

    /// Valeur saisie dans un champ (`None` : celle de l'original). Le texte
    /// d'un champ mis en forme est vérifié et réécrit (« 19/11/26 » →
    /// « 19/11/2026 ») ; les champs calculés suivent.
    pub fn set_field(
        &self,
        id: u64,
        name: &str,
        value: Option<FieldValue>,
    ) -> Result<EditView, AppError> {
        self.with_document(id, |open, doc| {
            let fields = self.merged_fields(open, doc)?;
            let field = fields
                .iter()
                .find(|f| f.name == name)
                .ok_or_else(|| invalid(format!("champ « {name} » introuvable")))?;
            let value = checked_value(field, value).map_err(invalid)?;
            let (_, change) = open
                .edits
                .grouped(|edits| {
                    edits.set_field(name, value)?;
                    recalculate(edits, &fields)
                })
                .map_err(invalid)?;
            self.commit(open, doc, change)?;
            Ok(view(open))
        })
    }

    /// Propriétés du document : celles changées, sinon celles de l'original.
    pub fn metadata(&self, id: u64) -> Result<Metadata, AppError> {
        self.with_document(id, |open, _| {
            Ok(open
                .edits
                .metadata()
                .cloned()
                .unwrap_or_else(|| open.original.metadata.clone()))
        })
    }

    pub fn set_metadata(&self, id: u64, metadata: Metadata) -> Result<EditView, AppError> {
        self.with_document(id, |open, doc| {
            let original = open.original.metadata.clone();
            let change = open
                .edits
                .set_metadata(metadata, &original)
                .map_err(invalid)?;
            self.commit(open, doc, change)?;
            Ok(view(open))
        })
    }

    /// Aplatir le formulaire (ou revenir aux champs, par annuler).
    pub fn set_flatten(&self, id: u64, flatten: bool) -> Result<EditView, AppError> {
        self.with_document(id, |open, doc| {
            let change = open.edits.set_flatten(flatten);
            self.commit(open, doc, change)?;
            Ok(view(open))
        })
    }
}

/// Champs du document (déjà lus) et champs conçus réunis.
pub(super) fn merged(open: &OpenDocument) -> Result<Vec<FormField>, AppError> {
    let existing = open.form_fields.as_deref().unwrap_or_default();
    designed_fields(existing, &design_boxes(&open.edits)).map_err(invalid)
}

/// La valeur vérifiée pour ce champ, mise en forme s'il en a une.
fn checked_value(
    field: &FormField,
    value: Option<FieldValue>,
) -> Result<Option<FieldValue>, String> {
    let name = &field.name;
    if field.read_only {
        return Err(format!("le champ « {name} » est en lecture seule"));
    }
    match &field.kind {
        FieldKind::Signature => {
            return Err("un champ de signature se signe dans le mode Signer".into());
        }
        FieldKind::Text { password: true, .. } => {
            return Err(
                "champ de mot de passe : sa valeur ne s'enregistre jamais dans le fichier".into(),
            );
        }
        FieldKind::Text { calc: Some(_), .. } => {
            return Err(format!("le champ « {name} » est calculé"));
        }
        _ => {}
    }
    let Some(value) = value else {
        return Ok(None);
    };
    value.validate()?;
    let value = match (field.kind.format(), value) {
        (Some(format), FieldValue::Text(text)) => {
            let text = format
                .normalize(&text)
                .map_err(|e| format!("« {} » : {e}", label(field)))?;
            FieldValue::Text(text)
        }
        (_, value) => value,
    };
    field.kind.accepts(&value)?;
    Ok(Some(value))
}

/// Nom lisible d'un champ dans un message : son infobulle, sinon son nom.
fn label(field: &FormField) -> &str {
    if field.tooltip.trim().is_empty() {
        &field.name
    } else {
        field.tooltip.trim()
    }
}

/// Recalcule les champs calculés (somme, moyenne…) d'après les valeurs
/// saisies ; chaque résultat changé est une valeur de plus dans la même
/// opération.
pub(super) fn recalculate(edits: &mut Edits, fields: &[FormField]) -> Result<(), String> {
    for _ in 0..CALC_PASSES {
        let mut changed = false;
        for field in fields {
            let Some(calc) = field.kind.calc() else {
                continue;
            };
            let current = |name: &str| -> Option<String> {
                edits
                    .fields()
                    .get(name)
                    .or_else(|| fields.iter().find(|f| f.name == name)?.value.as_ref())
                    .map(|v| v.text().to_string())
            };
            let values: Vec<(String, Option<String>)> = calc
                .fields
                .iter()
                .map(|n| (n.clone(), current(n)))
                .collect();
            let result = calc.evaluate(|name| {
                values
                    .iter()
                    .find(|(n, _)| n == name)
                    .and_then(|(_, v)| v.as_deref())
            });
            let text = field.kind.format().map_or_else(
                || plain_number(result),
                |f: &FieldFormat| f.canonical(result),
            );
            if current(&field.name).as_deref() != Some(text.as_str()) {
                edits.set_field(&field.name, Some(FieldValue::Text(text)))?;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    Ok(())
}

pub(super) fn field_view(open: &OpenDocument, field: &FormField) -> FieldView {
    let widgets = field
        .widgets
        .iter()
        .filter_map(|w| {
            let size = open.info.page_size(w.page).ok()?;
            Some(WidgetView {
                page: w.page,
                bounds: BoxView::new(&w.bounds, size),
                option: w.option,
                index: w.index,
                export: w.export.clone(),
                element: w.element.map(|e| e.0),
            })
        })
        .collect();
    // Créé : chaque emplacement vient d'un élément sans champ d'origine.
    let created = !field.widgets.is_empty()
        && field.widgets.iter().all(|w| {
            w.element
                .and_then(|e| open.edits.get(e))
                .is_some_and(|e| matches!(e, Element::Field(b) if b.origin.is_none()))
        });
    FieldView {
        name: field.name.clone(),
        kind: field.kind.clone(),
        read_only: field.read_only,
        required: field.required,
        tooltip: field.tooltip.clone(),
        look: field.look.clone(),
        value: field.value.clone(),
        widgets,
        created,
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Session;
    use super::*;
    use crate::error::ErrorKind;

    #[test]
    fn fields_are_listed_once_and_filled_with_checks() {
        let s = Session::new("formulaire");
        let fields = s.viewer.form_fields(s.id).unwrap();
        assert_eq!(fields.len(), 3);
        assert!(!fields[0].created);
        // Faux moteur : « nom » de (100, 700) à (300, 720), origine en bas.
        assert_eq!(fields[0].widgets[0].bounds.y, 842.0 - 720.0);
        let view = s
            .viewer
            .set_field(s.id, "nom", Some(FieldValue::Text("Jean".into())))
            .unwrap();
        assert!(view.dirty);
        assert_eq!(view.fields["nom"], FieldValue::Text("Jean".into()));
        let refused = |name: &str, value| {
            s.viewer
                .set_field(s.id, name, Some(value))
                .unwrap_err()
                .kind
        };
        assert_eq!(
            refused("absent", FieldValue::Checked(true)),
            ErrorKind::InvalidRequest
        );
        assert_eq!(
            refused("nom", FieldValue::Checked(true)),
            ErrorKind::InvalidRequest
        );
        assert_eq!(
            refused("ville", FieldValue::Choice("Rome".into())),
            ErrorKind::InvalidRequest
        );
        assert_eq!(
            refused("nom", FieldValue::Text("x".repeat(31))),
            ErrorKind::InvalidRequest
        );
        s.viewer
            .set_field(s.id, "ville", Some(FieldValue::Choice("Lyon".into())))
            .unwrap();
        // Une annulation par valeur.
        s.viewer.undo(s.id).unwrap();
        let view = s.viewer.undo(s.id).unwrap();
        assert!(view.fields.is_empty() && !view.dirty);
    }

    #[test]
    fn properties_and_flattening_are_undoable() {
        let s = Session::new("proprietes");
        assert_eq!(s.viewer.metadata(s.id).unwrap(), Metadata::default());
        let renamed = Metadata {
            title: "Contrat".into(),
            ..Metadata::default()
        };
        let view = s.viewer.set_metadata(s.id, renamed.clone()).unwrap();
        assert_eq!(view.metadata, Some(renamed.clone()));
        assert_eq!(s.viewer.metadata(s.id).unwrap(), renamed);
        let view = s.viewer.set_flatten(s.id, true).unwrap();
        assert!(view.flattened);
        // Toutes les pages sont redessinées.
        assert!(view.revisions.iter().all(|r| *r > 0));
        assert!(
            s.viewer
                .set_field(s.id, "nom", Some(FieldValue::Text("a".into())))
                .is_err()
        );
        s.viewer.undo(s.id).unwrap();
        let view = s.viewer.undo(s.id).unwrap();
        assert!(view.metadata.is_none() && !view.flattened);
    }
}
