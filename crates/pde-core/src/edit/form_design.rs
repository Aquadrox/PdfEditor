//! Formulaire tel qu'il sera écrit (E-12) : champs du document, modifiés
//! ou supprimés par les éléments de conception, plus les champs créés.
//! C'est la liste que l'interface montre et remplit.

use std::collections::BTreeSet;

use super::element::ElementId;
use super::field_box::FieldBox;
use super::form::{FieldKind, FormField, Widget};

/// Les champs du document (`existing`) et les éléments de conception
/// (`boxes`, par identifiant croissant) réunis ; refuse deux champs de même
/// nom, ou un champ du document qui changerait de genre.
pub fn designed_fields(
    existing: &[FormField],
    boxes: &[(ElementId, &FieldBox)],
) -> Result<Vec<FormField>, String> {
    let mut out = Vec::with_capacity(existing.len());
    for field in existing {
        let edits: Vec<&(ElementId, &FieldBox)> = boxes
            .iter()
            .filter(|(_, b)| b.origin.as_ref().is_some_and(|o| o.name == field.name))
            .collect();
        if edits.is_empty() {
            out.push(field.clone());
        } else if let Some(changed) = changed_field(field, &edits)? {
            out.push(changed);
        }
    }
    let mut created: Vec<FormField> = Vec::new();
    for (id, b) in boxes
        .iter()
        .filter(|(_, b)| b.origin.is_none() && !b.deleted)
    {
        match created.iter_mut().find(|f| f.name == b.name) {
            Some(field) => {
                if !field.kind.same_family(&b.kind) {
                    return Err(format!(
                        "les emplacements du champ « {} » doivent être du même genre",
                        b.name
                    ));
                }
                add_widget(field, *id, b)?;
            }
            None => {
                let mut field = settings_of(FormField::new(b.name.clone(), b.kind.clone()), b);
                field.value.clone_from(&b.value);
                if let FieldKind::Radio { options } = &mut field.kind {
                    options.clear();
                }
                add_widget(&mut field, *id, b)?;
                created.push(field);
            }
        }
    }
    out.extend(created);
    let mut names = BTreeSet::new();
    for field in &out {
        if !names.insert(field.name.as_str()) {
            return Err(format!("deux champs s'appellent « {} »", field.name));
        }
    }
    Ok(out)
}

/// Réglages communs à tous les emplacements d'un champ, pris de `b`.
fn settings_of(mut field: FormField, b: &FieldBox) -> FormField {
    field.name.clone_from(&b.name);
    field.read_only = b.read_only;
    field.required = b.required;
    field.tooltip.clone_from(&b.tooltip);
    field.look = b.look.clone();
    field
}

fn add_widget(field: &mut FormField, id: ElementId, b: &FieldBox) -> Result<(), String> {
    let index = field.widgets.len();
    let mut widget = Widget::new(b.page, b.rect(), index);
    widget.element = Some(id);
    if b.is_button() {
        widget.export = Some(b.export.clone());
    }
    if let FieldKind::Radio { options } = &mut field.kind {
        if options.contains(&b.export) {
            return Err(format!(
                "deux boutons du groupe « {} » ont la valeur « {} »",
                b.name, b.export
            ));
        }
        widget.option = Some(options.len());
        options.push(b.export.clone());
    }
    field.widgets.push(widget);
    Ok(())
}

/// Champ du document après ses éléments ; `None` s'il est supprimé.
fn changed_field(
    field: &FormField,
    edits: &[&(ElementId, &FieldBox)],
) -> Result<Option<FormField>, String> {
    let widgets: Vec<Widget> = field
        .widgets
        .iter()
        .filter_map(|w| {
            let Some((id, b)) = edits
                .iter()
                .find(|(_, b)| b.origin.as_ref().is_some_and(|o| o.widget == w.index))
            else {
                return Some(w.clone());
            };
            (!b.deleted).then(|| Widget {
                page: b.page,
                bounds: b.rect(),
                element: Some(*id),
                ..w.clone()
            })
        })
        .collect();
    if widgets.is_empty() {
        return Ok(None);
    }
    let mut changed = FormField {
        widgets,
        ..field.clone()
    };
    if let Some((_, b)) = edits.iter().find(|(_, b)| !b.deleted) {
        if !b.kind.same_family(&field.kind) {
            return Err(format!(
                "le champ « {} » ne peut pas changer de genre",
                field.name
            ));
        }
        changed = settings_of(changed, b);
        // Les options d'un groupe radio sont celles de ses boutons.
        if !matches!(field.kind, FieldKind::Radio { .. }) {
            changed.kind = b.kind.clone();
        }
    }
    Ok(Some(changed))
}

#[cfg(test)]
mod tests {
    use super::super::field_box::FieldOrigin;
    use super::super::form::{FieldLook, FieldValue};
    use super::*;
    use crate::geometry::{Point, Rect};

    fn new_box(name: &str, kind: FieldKind, export: &str, x: f64) -> FieldBox {
        FieldBox {
            page: 0,
            top_left: Point::new(x, 700.0),
            width: 15.0,
            height: 15.0,
            name: name.into(),
            kind,
            tooltip: String::new(),
            required: false,
            read_only: false,
            look: FieldLook::default(),
            value: None,
            export: export.into(),
            origin: None,
            deleted: false,
        }
    }

    fn existing() -> Vec<FormField> {
        let mut name = FormField::new("client.nom", FieldKind::text());
        name.value = Some(FieldValue::Text("Ancien".into()));
        name.widgets = vec![
            Widget::new(
                0,
                Rect::from_corners(Point::new(0.0, 0.0), Point::new(10.0, 10.0)),
                0,
            ),
            Widget::new(
                1,
                Rect::from_corners(Point::new(0.0, 0.0), Point::new(10.0, 10.0)),
                1,
            ),
        ];
        let mut ok = FormField::new("accord", FieldKind::Checkbox);
        ok.widgets = vec![Widget::new(0, Rect::default(), 0)];
        vec![name, ok]
    }

    #[test]
    fn untouched_document_fields_stay_as_they_are() {
        assert_eq!(designed_fields(&existing(), &[]).unwrap(), existing());
    }

    #[test]
    fn created_radio_buttons_form_one_group() {
        let m = new_box(
            "civilite",
            FieldKind::Radio {
                options: Vec::new(),
            },
            "M",
            100.0,
        );
        let mme = new_box(
            "civilite",
            FieldKind::Radio {
                options: Vec::new(),
            },
            "Mme",
            140.0,
        );
        let date = new_box("date", FieldKind::text(), "", 200.0);
        let boxes = [
            (ElementId(1), &m),
            (ElementId(2), &mme),
            (ElementId(3), &date),
        ];
        let fields = designed_fields(&existing(), &boxes).unwrap();
        assert_eq!(fields.len(), 4);
        let group = &fields[2];
        assert_eq!(
            group.kind,
            FieldKind::Radio {
                options: vec!["M".into(), "Mme".into()]
            }
        );
        assert_eq!(group.widgets[1].option, Some(1));
        assert_eq!(group.widgets[1].element, Some(ElementId(2)));
        assert_eq!(group.widgets[1].bounds.min(), Point::new(140.0, 685.0));
        let same = new_box(
            "civilite",
            FieldKind::Radio {
                options: Vec::new(),
            },
            "M",
            180.0,
        );
        let error = designed_fields(&[], &[(ElementId(1), &m), (ElementId(2), &same)]);
        assert!(error.unwrap_err().contains("« M »"));
        let mixed = new_box("civilite", FieldKind::Checkbox, "Oui", 180.0);
        assert!(designed_fields(&[], &[(ElementId(1), &m), (ElementId(2), &mixed)]).is_err());
    }

    #[test]
    fn names_stay_unique() {
        let clash = new_box("accord", FieldKind::text(), "", 0.0);
        let error = designed_fields(&existing(), &[(ElementId(1), &clash)]).unwrap_err();
        assert!(error.contains("accord"), "{error}");
    }

    #[test]
    fn document_fields_are_moved_renamed_and_deleted() {
        let mut moved = new_box("client.nom_complet", FieldKind::text(), "", 300.0);
        moved.page = 1;
        moved.required = true;
        moved.origin = Some(FieldOrigin {
            name: "client.nom".into(),
            widget: 1,
        });
        let fields = designed_fields(&existing(), &[(ElementId(7), &moved)]).unwrap();
        let name = &fields[0];
        assert_eq!(name.name, "client.nom_complet");
        assert!(name.required);
        // Valeur d'origine gardée ; premier emplacement inchangé.
        assert_eq!(name.value, Some(FieldValue::Text("Ancien".into())));
        assert_eq!(name.widgets[0].element, None);
        assert_eq!(name.widgets[1].element, Some(ElementId(7)));
        assert_eq!(name.widgets[1].bounds.min().x, 300.0);

        let mut gone = moved.clone();
        gone.deleted = true;
        gone.origin = Some(FieldOrigin {
            name: "accord".into(),
            widget: 0,
        });
        let fields = designed_fields(&existing(), &[(ElementId(8), &gone)]).unwrap();
        assert_eq!(fields.len(), 1);

        let mut changed = new_box("accord", FieldKind::text(), "", 0.0);
        changed.origin = gone.origin.clone();
        assert!(designed_fields(&existing(), &[(ElementId(9), &changed)]).is_err());
    }
}
