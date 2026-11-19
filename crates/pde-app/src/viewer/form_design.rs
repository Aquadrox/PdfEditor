//! Conception de formulaires (E-12) vue de la visionneuse : créer des
//! champs, régler un champ (tous ses emplacements à la fois), déplacer ou
//! redimensionner des emplacements, en supprimer, et proposer les champs
//! d'un formulaire « papier ». Chaque action est une seule opération de
//! l'historique ; le formulaire obtenu est vérifié (noms uniques, genres)
//! avant d'être gardé. Coordonnées échangées avec l'interface : origine
//! en haut.

use pde_core::document::{DocumentId, PageIndex};
use pde_core::edit::{
    Edits, Element, FieldBox, FieldKind, FieldOrigin, FormField, Widget, designed_fields,
    detect_fields, name_for_label,
};
use pde_core::geometry::Point;
use serde::Deserialize;

use super::edit::{invalid, view};
use super::form::{design_boxes, recalculate};
use super::{OpenDocument, Viewer};
use crate::edit_view::{EditView, flip};
use crate::error::AppError;

/// Nouvelle place d'un emplacement (origine en haut).
#[derive(Debug, Clone, Deserialize)]
pub struct WidgetPlace {
    pub name: String,
    pub index: usize,
    pub page: PageIndex,
    pub top_left: Point,
    pub width: f64,
    pub height: f64,
}

/// Champ entier (`index` absent) ou un de ses emplacements.
#[derive(Debug, Clone, Deserialize)]
pub struct WidgetTarget {
    pub name: String,
    #[serde(default)]
    pub index: Option<usize>,
}

/// Élément qui décrit l'emplacement `widget` d'un champ du document.
fn box_from_field(field: &FormField, widget: &Widget) -> FieldBox {
    let b = widget.bounds;
    FieldBox {
        page: widget.page,
        top_left: Point::new(b.min().x, b.max().y),
        width: b.width(),
        height: b.height(),
        name: field.name.clone(),
        kind: field.kind.clone(),
        tooltip: field.tooltip.clone(),
        required: field.required,
        read_only: field.read_only,
        look: field.look.clone(),
        value: None,
        export: widget.export.clone().unwrap_or_default(),
        origin: Some(FieldOrigin {
            name: field.name.clone(),
            widget: widget.index,
        }),
        deleted: false,
    }
}

/// Réglages de `template` appliqués à l'emplacement `b` (sa place, sa
/// valeur de bouton et son origine restent).
fn with_settings(mut b: FieldBox, template: &FieldBox) -> FieldBox {
    b.name = template.name.trim().to_string();
    if !matches!(b.kind, FieldKind::Radio { .. }) {
        b.kind = template.kind.clone();
    }
    b.tooltip.clone_from(&template.tooltip);
    b.required = template.required;
    b.read_only = template.read_only;
    b.look = template.look.clone();
    if b.origin.is_none() {
        b.value.clone_from(&template.value);
        if matches!(b.kind, FieldKind::Checkbox) && !template.export.is_empty() {
            b.export.clone_from(&template.export);
        }
    }
    b
}

fn field<'a>(fields: &'a [FormField], name: &str) -> Result<&'a FormField, String> {
    fields
        .iter()
        .find(|f| f.name == name)
        .ok_or_else(|| format!("champ « {name} » introuvable"))
}

fn element_box(edits: &Edits, id: pde_core::edit::ElementId) -> Result<FieldBox, String> {
    match edits.get(id) {
        Some(Element::Field(b)) => Ok((**b).clone()),
        _ => Err("élément de formulaire introuvable".into()),
    }
}

/// Un nom libre : `name`, sinon `name_2`, `name_3`…
fn free_name(name: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == name) {
        return name.to_string();
    }
    (2..)
        .map(|n| format!("{name}_{n}"))
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or_else(|| name.to_string())
}

impl Viewer {
    /// Fait `f` sur l'historique en une opération, après lecture des
    /// champs ; le formulaire obtenu doit être cohérent.
    fn design<T>(
        &self,
        id: u64,
        f: impl FnOnce(&mut Edits, &[FormField], &OpenDocumentInfo) -> Result<T, String>,
    ) -> Result<(T, EditView), AppError> {
        self.with_document(id, |open, doc| {
            let fields = self.merged_fields(open, doc)?;
            let existing = open.form_fields.clone().unwrap_or_default();
            let info = OpenDocumentInfo::of(open);
            let (value, change) = open
                .edits
                .grouped(|edits| {
                    let value = f(edits, &fields, &info)?;
                    let after = designed_fields(&existing, &design_boxes(edits))?;
                    recalculate(edits, &after)?;
                    Ok(value)
                })
                .map_err(invalid)?;
            self.commit(open, doc, change)?;
            Ok((value, view(open)))
        })
    }

    /// Crée des champs (coordonnées avec l'origine en haut). Un nom déjà
    /// pris reçoit un suffixe, sauf pour un bouton radio qui rejoint le
    /// groupe créé du même nom.
    pub fn add_form_fields(
        &self,
        id: u64,
        boxes: Vec<FieldBox>,
    ) -> Result<(Vec<u64>, EditView), AppError> {
        self.design(id, |edits, fields, info| {
            let mut taken: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
            let mut groups: Vec<String> = fields
                .iter()
                .filter(|f| matches!(f.kind, FieldKind::Radio { .. }))
                .filter(|f| {
                    f.widgets.iter().all(|w| {
                        w.element
                            .and_then(|e| edits.get(e))
                            .is_some_and(|e| matches!(e, Element::Field(b) if b.origin.is_none()))
                    })
                })
                .map(|f| f.name.clone())
                .collect();
            let mut ids = Vec::new();
            for b in boxes {
                let Element::Field(mut b) = info.checked(Element::Field(Box::new(b)))? else {
                    continue;
                };
                if b.origin.is_some() || b.deleted {
                    return Err("un champ créé ne désigne pas de champ du document".into());
                }
                let radio = matches!(b.kind, FieldKind::Radio { .. });
                if !(radio && groups.contains(&b.name)) {
                    b.name = free_name(b.name.trim(), &taken);
                    taken.push(b.name.clone());
                    if radio {
                        groups.push(b.name.clone());
                    }
                }
                ids.push(edits.add(Element::Field(b)).0.0);
            }
            Ok(ids)
        })
    }

    /// Réglages d'un champ (nom, genre, format, obligatoire…) appliqués à
    /// tous ses emplacements ; la valeur saisie suit un changement de nom.
    pub fn set_form_field(
        &self,
        id: u64,
        name: &str,
        template: FieldBox,
    ) -> Result<EditView, AppError> {
        self.design(id, |edits, fields, info| {
            let field = field(fields, name)?;
            for widget in &field.widgets {
                let (element, b) = match widget.element {
                    Some(element) => (Some(element), element_box(edits, element)?),
                    None => (None, box_from_field(field, widget)),
                };
                let b = info.validated(with_settings(b, &template))?;
                match element {
                    Some(element) => {
                        edits.update(element, Element::Field(Box::new(b)))?;
                    }
                    None => {
                        edits.add(Element::Field(Box::new(b)));
                    }
                }
            }
            let renamed = template.name.trim();
            if renamed != name
                && let Some(value) = edits.fields().get(name).cloned()
            {
                edits.set_field(name, None)?;
                edits.set_field(renamed, Some(value))?;
            }
            Ok(())
        })
        .map(|(_, view)| view)
    }

    /// Déplace ou redimensionne des emplacements (une opération).
    pub fn place_form_widgets(
        &self,
        id: u64,
        places: Vec<WidgetPlace>,
    ) -> Result<EditView, AppError> {
        self.design(id, |edits, fields, info| {
            for place in places {
                let field = field(fields, &place.name)?;
                let widget = field
                    .widgets
                    .iter()
                    .find(|w| w.index == place.index)
                    .ok_or("emplacement introuvable")?;
                let mut b = match widget.element {
                    Some(element) => element_box(edits, element)?,
                    None => box_from_field(field, widget),
                };
                b.page = place.page;
                b.top_left = place.top_left;
                b.width = place.width;
                b.height = place.height;
                let Element::Field(b) = info.checked(Element::Field(Box::new(b)))? else {
                    continue;
                };
                match widget.element {
                    Some(element) => {
                        edits.update(element, Element::Field(b))?;
                    }
                    None => {
                        edits.add(Element::Field(b));
                    }
                }
            }
            Ok(())
        })
        .map(|(_, view)| view)
    }

    /// Supprime des champs entiers ou certains de leurs emplacements.
    pub fn delete_form_widgets(
        &self,
        id: u64,
        targets: Vec<WidgetTarget>,
    ) -> Result<EditView, AppError> {
        self.design(id, |edits, fields, _| {
            for target in targets {
                let field = field(fields, &target.name)?;
                let chosen: Vec<&Widget> = field
                    .widgets
                    .iter()
                    .filter(|w| target.index.is_none_or(|i| i == w.index))
                    .collect();
                for widget in &chosen {
                    match widget.element {
                        Some(element) => {
                            let mut b = element_box(edits, element)?;
                            if b.origin.is_some() {
                                b.deleted = true;
                                edits.update(element, Element::Field(Box::new(b)))?;
                            } else {
                                edits.remove(element)?;
                            }
                        }
                        None => {
                            let mut b = box_from_field(field, widget);
                            b.deleted = true;
                            edits.add(Element::Field(Box::new(b)));
                        }
                    }
                }
                if chosen.len() == field.widgets.len() && edits.fields().contains_key(&field.name) {
                    edits.set_field(&field.name, None)?;
                }
            }
            Ok(())
        })
        .map(|(_, view)| view)
    }

    /// Champs proposés pour la page `page` d'un formulaire « papier »
    /// (traits, pointillés, cases), hors des champs déjà là ; rien n'est
    /// ajouté (origine en haut).
    pub fn suggest_form_fields(&self, id: u64, page: PageIndex) -> Result<Vec<FieldBox>, AppError> {
        let text = self.text(DocumentId(id), page)?;
        self.with_document(id, |open, doc| {
            let size = open.info.page_size(page)?;
            let fields = self.merged_fields(open, doc)?;
            let occupied: Vec<pde_core::geometry::Rect> = fields
                .iter()
                .flat_map(|f| f.widgets.iter())
                .filter(|w| w.page == page)
                .map(|w| w.bounds)
                .collect();
            let mut taken: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
            let mut out = Vec::new();
            for s in detect_fields(&text, size.width) {
                let overlaps = occupied.iter().any(|o| {
                    o.intersection(&s.rect).is_some_and(|i| {
                        i.width() * i.height() > 0.3 * s.rect.width() * s.rect.height()
                    })
                });
                if overlaps {
                    continue;
                }
                let name = free_name(&name_for_label(&s.label), &taken);
                taken.push(name.clone());
                let checkbox = matches!(s.kind, FieldKind::Checkbox);
                let b = FieldBox {
                    page,
                    top_left: Point::new(s.rect.min().x, s.rect.max().y),
                    width: s.rect.width(),
                    height: s.rect.height(),
                    name,
                    kind: s.kind,
                    tooltip: s.label,
                    required: false,
                    read_only: false,
                    look: Default::default(),
                    value: None,
                    export: if checkbox {
                        "Oui".into()
                    } else {
                        String::new()
                    },
                    origin: None,
                    deleted: false,
                };
                out.push(flip(&Element::Field(Box::new(b)), size));
            }
            Ok(out
                .into_iter()
                .filter_map(|e| match e {
                    Element::Field(b) => Some(*b),
                    _ => None,
                })
                .collect())
        })
    }
}

/// Pages du document ouvert : de quoi vérifier un élément.
pub(super) struct OpenDocumentInfo {
    sizes: Vec<pde_core::document::PageSize>,
}

impl OpenDocumentInfo {
    fn of(open: &OpenDocument) -> Self {
        Self {
            sizes: open.info.page_sizes.clone(),
        }
    }

    /// Élément venu de l'interface (origine en haut) : vérifié, ramené à
    /// l'origine en bas.
    fn checked(&self, element: Element) -> Result<Element, String> {
        element.validate(self.sizes.len(), None)?;
        let size = self.sizes[element.page()];
        Ok(flip(&element, size))
    }

    /// Élément déjà à l'origine en bas : vérifié.
    fn validated(&self, b: FieldBox) -> Result<FieldBox, String> {
        Element::Field(Box::new(b.clone())).validate(self.sizes.len(), None)?;
        Ok(b)
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Session;
    use super::*;
    use pde_core::edit::{FieldLook, FieldValue};
    use pde_core::formats::{CalcOp, FieldCalc, FieldFormat};

    fn new_field(name: &str, kind: FieldKind, y: f64) -> FieldBox {
        FieldBox {
            page: 0,
            top_left: Point::new(72.0, y),
            width: 120.0,
            height: 18.0,
            name: name.into(),
            kind,
            tooltip: String::new(),
            required: false,
            read_only: false,
            look: FieldLook::default(),
            value: None,
            export: String::new(),
            origin: None,
            deleted: false,
        }
    }

    fn radio(name: &str, export: &str) -> FieldBox {
        let mut b = new_field(
            name,
            FieldKind::Radio {
                options: Vec::new(),
            },
            300.0,
        );
        b.export = export.into();
        b
    }

    #[test]
    fn created_fields_get_free_names_and_radio_groups() {
        let s = Session::new("conception-noms");
        let (ids, view) = s
            .viewer
            .add_form_fields(
                s.id,
                vec![
                    new_field("nom", FieldKind::text(), 100.0),
                    new_field("date", FieldKind::text(), 130.0),
                    radio("choix", "A"),
                    radio("choix", "B"),
                ],
            )
            .unwrap();
        assert_eq!(ids.len(), 4);
        assert!(view.dirty && view.can_undo);
        let fields = s.viewer.form_fields(s.id).unwrap();
        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["nom", "accord", "ville", "nom_2", "date", "choix"]);
        let created = &fields[3];
        assert!(created.created);
        // Origine en haut, comme envoyé.
        assert_eq!(created.widgets[0].bounds.y, 100.0);
        assert_eq!(fields[5].widgets.len(), 2);
        // Un autre bouton rejoint le groupe créé.
        s.viewer
            .add_form_fields(s.id, vec![radio("choix", "C")])
            .unwrap();
        let fields = s.viewer.form_fields(s.id).unwrap();
        assert_eq!(
            fields[5].kind,
            FieldKind::Radio {
                options: vec!["A".into(), "B".into(), "C".into()]
            }
        );
        // Une opération par ajout.
        s.viewer.undo(s.id).unwrap();
        let view = s.viewer.undo(s.id).unwrap();
        assert!(!view.dirty);
        assert_eq!(s.viewer.form_fields(s.id).unwrap().len(), 3);
    }

    #[test]
    fn document_fields_are_set_moved_and_deleted() {
        let s = Session::new("conception-champs");
        s.viewer
            .set_field(s.id, "nom", Some(FieldValue::Text("Jean".into())))
            .unwrap();
        let mut template = new_field("nom_complet", FieldKind::text(), 0.0);
        template.required = true;
        let view = s.viewer.set_form_field(s.id, "nom", template).unwrap();
        // La valeur suit le nouveau nom.
        assert_eq!(
            view.fields.get("nom_complet"),
            Some(&FieldValue::Text("Jean".into()))
        );
        assert!(!view.fields.contains_key("nom"));
        let fields = s.viewer.form_fields(s.id).unwrap();
        assert_eq!(fields[0].name, "nom_complet");
        assert!(fields[0].required && !fields[0].created);
        assert!(fields[0].widgets[0].element.is_some());

        s.viewer
            .place_form_widgets(
                s.id,
                vec![WidgetPlace {
                    name: "accord".into(),
                    index: 0,
                    page: 0,
                    top_left: Point::new(50.0, 60.0),
                    width: 20.0,
                    height: 20.0,
                }],
            )
            .unwrap();
        let fields = s.viewer.form_fields(s.id).unwrap();
        assert_eq!(fields[1].widgets[0].bounds.x, 50.0);
        assert_eq!(fields[1].widgets[0].bounds.y, 60.0);

        s.viewer
            .delete_form_widgets(
                s.id,
                vec![WidgetTarget {
                    name: "ville".into(),
                    index: None,
                }],
            )
            .unwrap();
        let names: Vec<String> = s
            .viewer
            .form_fields(s.id)
            .unwrap()
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(names, ["nom_complet", "accord"]);
        // Refusé : deux champs du même nom, ou un genre changé.
        let clash = new_field("accord", FieldKind::text(), 0.0);
        assert!(s.viewer.set_form_field(s.id, "nom_complet", clash).is_err());
        let checkbox = new_field("nom_complet", FieldKind::Checkbox, 0.0);
        assert!(
            s.viewer
                .set_form_field(s.id, "nom_complet", checkbox)
                .is_err()
        );
    }

    #[test]
    fn smart_values_and_totals() {
        let s = Session::new("conception-calculs");
        let number = |name: &str, y: f64| {
            new_field(
                name,
                FieldKind::Text {
                    multiline: false,
                    max_len: None,
                    password: false,
                    comb: false,
                    format: Some(FieldFormat::Number {
                        decimals: 2,
                        separator: pde_core::formats::Separator::Comma,
                        currency: " €".into(),
                        currency_before: false,
                    }),
                    calc: None,
                },
                y,
            )
        };
        let mut total = number("total", 200.0);
        if let FieldKind::Text { calc, .. } = &mut total.kind {
            *calc = Some(FieldCalc {
                op: CalcOp::Sum,
                fields: vec!["a".into(), "b".into()],
            });
        }
        let mut date = new_field("date", FieldKind::text(), 250.0);
        if let FieldKind::Text { format, .. } = &mut date.kind {
            *format = Some(FieldFormat::Date {
                pattern: "dd/mm/yyyy".into(),
            });
        }
        s.viewer
            .add_form_fields(
                s.id,
                vec![number("a", 100.0), number("b", 150.0), total, date],
            )
            .unwrap();
        s.viewer
            .set_field(s.id, "a", Some(FieldValue::Text("1 000,5".into())))
            .unwrap();
        let view = s
            .viewer
            .set_field(s.id, "b", Some(FieldValue::Text("2".into())))
            .unwrap();
        assert_eq!(view.fields["a"], FieldValue::Text("1000.50".into()));
        assert_eq!(view.fields["total"], FieldValue::Text("1002.50".into()));
        // Le total suit en une seule annulation.
        let view = s.viewer.undo(s.id).unwrap();
        assert_eq!(view.fields["total"], FieldValue::Text("1000.50".into()));
        let error = s
            .viewer
            .set_field(s.id, "total", Some(FieldValue::Text("3".into())))
            .unwrap_err();
        assert!(error.message.contains("calculé"), "{error:?}");
        let view = s
            .viewer
            .set_field(s.id, "date", Some(FieldValue::Text("2026-11-19".into())))
            .unwrap();
        assert_eq!(view.fields["date"], FieldValue::Text("19/11/2026".into()));
        let error = s
            .viewer
            .set_field(s.id, "date", Some(FieldValue::Text("31/02/2026".into())))
            .unwrap_err();
        assert!(error.message.contains("19/11/2026"), "{error:?}");
    }

    #[test]
    fn paper_forms_get_suggestions() {
        let s = Session::new("conception-detection");
        // Troisième page du faux moteur : « Date : ______ ».
        s.viewer.insert_blank_page(s.id, 2).unwrap();
        let found = s.viewer.suggest_form_fields(s.id, 2).unwrap();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, "date");
        assert!(found[0].kind.format().is_some());
        // Rien n'est ajouté tant que l'utilisateur n'accepte pas.
        assert!(s.viewer.edit_view(s.id).unwrap().elements.is_empty());
        let (ids, _) = s.viewer.add_form_fields(s.id, found).unwrap();
        assert_eq!(ids.len(), 1);
        assert!(s.viewer.suggest_form_fields(s.id, 2).unwrap().is_empty());
    }
}
