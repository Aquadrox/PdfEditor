//! Formulaire (E-08) et propriétés du document (E-11) vus de la
//! visionneuse : champs placés sur les pages (origine en haut), valeurs
//! vérifiées avant d'être gardées, aplatissement, propriétés.

use pde_core::document::PageIndex;
use pde_core::edit::{FieldKind, FieldValue, FormField, Metadata};
use pde_core::engine::Edition;
use serde::Serialize;

use super::edit::{invalid, view};
use super::{OpenDocument, Viewer};
use crate::edit_view::EditView;
use crate::error::AppError;
use crate::text_view::BoxView;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WidgetView {
    pub page: PageIndex,
    pub bounds: BoxView,
    /// Bouton radio : rang de son option.
    pub option: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FieldView {
    pub name: String,
    pub kind: FieldKind,
    pub read_only: bool,
    /// Valeur dans le document d'origine (la valeur saisie est dans EditView).
    pub value: Option<FieldValue>,
    pub widgets: Vec<WidgetView>,
}

impl Viewer {
    /// Champs du formulaire, lus une fois par plan des pages.
    pub fn form_fields(&self, id: u64) -> Result<Vec<FieldView>, AppError> {
        self.with_document(id, |open, doc| {
            if open.form_fields.is_none() {
                let edition = Edition {
                    plan: open.edits.plan().clone(),
                    imports: open.imports.clone(),
                    ..Edition::default()
                };
                open.form_fields = Some(self.engine.form_fields(doc, edition)?);
            }
            let fields = open.form_fields.as_deref().unwrap_or_default();
            Ok(fields.iter().map(|f| field_view(open, f)).collect())
        })
    }

    /// Valeur saisie dans un champ (`None` : celle de l'original).
    pub fn set_field(
        &self,
        id: u64,
        name: &str,
        value: Option<FieldValue>,
    ) -> Result<EditView, AppError> {
        self.form_fields(id)?;
        self.with_document(id, |open, doc| {
            let fields = open.form_fields.as_deref().unwrap_or_default();
            let field = fields
                .iter()
                .find(|f| f.name == name)
                .ok_or_else(|| invalid(format!("champ « {name} » introuvable")))?;
            if field.read_only {
                return Err(invalid(format!("le champ « {name} » est en lecture seule")));
            }
            if let Some(value) = &value {
                accepts(&field.kind, value).map_err(invalid)?;
            }
            let change = open.edits.set_field(name, value).map_err(invalid)?;
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

fn field_view(open: &OpenDocument, field: &FormField) -> FieldView {
    let widgets = field
        .widgets
        .iter()
        .filter_map(|w| {
            let size = open.info.page_size(w.page).ok()?;
            Some(WidgetView {
                page: w.page,
                bounds: BoxView::new(&w.bounds, size),
                option: w.option,
            })
        })
        .collect();
    FieldView {
        name: field.name.clone(),
        kind: field.kind.clone(),
        read_only: field.read_only,
        value: field.value.clone(),
        widgets,
    }
}

/// La valeur convient au genre du champ (option existante, longueur).
fn accepts(kind: &FieldKind, value: &FieldValue) -> Result<(), String> {
    match (kind, value) {
        (FieldKind::Text { max_len, .. }, FieldValue::Text(text)) => match max_len {
            Some(max) if text.chars().count() > *max as usize => {
                Err(format!("{max} caractères au plus"))
            }
            _ => Ok(()),
        },
        (FieldKind::Checkbox, FieldValue::Checked(_)) => Ok(()),
        (FieldKind::Radio { options }, FieldValue::Choice(choice))
        | (
            FieldKind::Choice {
                options,
                editable: false,
            },
            FieldValue::Choice(choice),
        ) if options.contains(choice) => Ok(()),
        (FieldKind::Choice { editable: true, .. }, FieldValue::Choice(_)) => Ok(()),
        _ => Err("valeur qui ne convient pas à ce champ".into()),
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
