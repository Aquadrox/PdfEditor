//! Conception de formulaires (E-12) sur un vrai PDF : champs créés sur un
//! document sans formulaire, champs du document déplacés, renommés, réglés
//! et supprimés ; tout relu après enregistrement. Nécessite PDFium
//! (`scripts/fetch-pdfium.sh`).

mod common;

use std::collections::BTreeMap;

use common::{FORM, engine, open_simple};
use pde_core::document::DocumentId;
use pde_core::edit::{
    Element, ElementId, FieldBox, FieldKind, FieldLook, FieldOrigin, FieldValue, FormField,
    PagePlan,
};
use pde_core::engine::{Edition, OpenedDocument, PdfEngine};
use pde_core::formats::{CalcOp, FieldCalc, FieldFormat, Separator};
use pde_core::geometry::Point;
use pde_core::render::RenderRequest;

fn field_box(name: &str, kind: FieldKind, (x, y): (f64, f64), (w, h): (f64, f64)) -> FieldBox {
    FieldBox {
        page: 0,
        top_left: Point::new(x, y),
        width: w,
        height: h,
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

fn edition(pages: usize, boxes: &[FieldBox], values: &[(&str, FieldValue)]) -> Edition {
    Edition {
        plan: PagePlan::identity(pages),
        elements: boxes
            .iter()
            .enumerate()
            .map(|(i, b)| (ElementId(i as u64 + 1), Element::Field(Box::new(b.clone()))))
            .collect(),
        fields: values
            .iter()
            .map(|(n, v)| ((*n).to_string(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
        ..Edition::default()
    }
}

fn reopen(doc: DocumentId) -> OpenedDocument {
    let bytes = engine().save(doc).expect("enregistrement");
    engine().open(bytes, None).expect("réouverture")
}

fn fields_of(doc: &OpenedDocument) -> Vec<FormField> {
    engine()
        .form_fields(doc.id, edition(doc.info.page_count(), &[], &[]))
        .expect("champs")
}

fn field<'a>(fields: &'a [FormField], name: &str) -> &'a FormField {
    fields.iter().find(|f| f.name == name).expect(name)
}

fn date_kind() -> FieldKind {
    FieldKind::Text {
        multiline: false,
        max_len: None,
        password: false,
        comb: false,
        format: Some(FieldFormat::Date {
            pattern: "dd/mm/yyyy".into(),
        }),
        calc: None,
    }
}

/// Pixels sombres dans le rectangle (page affichée, origine en bas).
fn dark_pixels(doc: DocumentId, (x0, x1): (u32, u32), (y0, y1): (u32, u32)) -> usize {
    let bitmap = engine()
        .render(
            doc,
            RenderRequest {
                page: 0,
                scale: 1.0,
            },
        )
        .expect("rendu");
    (x0..x1)
        .flat_map(|x| (y0..y1).map(move |y| (x, y)))
        .filter(|&(x, y)| {
            bitmap
                .pixel(x, bitmap.height() - y)
                .is_some_and(|p| p[0] < 110 && p[1] < 110 && p[2] < 110)
        })
        .count()
}

#[test]
fn fields_are_created_on_a_document_without_form() {
    let doc = open_simple();
    let pages = doc.info.page_count();
    let mut date = field_box("date", date_kind(), (72.0, 500.0), (150.0, 20.0));
    date.required = true;
    date.tooltip = "Date du contrat".into();
    let mut ok = field_box("accord", FieldKind::Checkbox, (72.0, 460.0), (15.0, 15.0));
    ok.export = "Oui".into();
    ok.value = Some(FieldValue::Checked(true));
    let radio = |export: &str, x: f64| {
        let mut b = field_box(
            "civilite",
            FieldKind::Radio {
                options: Vec::new(),
            },
            (x, 430.0),
            (14.0, 14.0),
        );
        b.export = export.into();
        b
    };
    let total = field_box(
        "total",
        FieldKind::Text {
            multiline: false,
            max_len: None,
            password: false,
            comb: false,
            format: Some(FieldFormat::Number {
                decimals: 2,
                separator: Separator::Comma,
                currency: " €".into(),
                currency_before: false,
            }),
            calc: Some(FieldCalc {
                op: CalcOp::Sum,
                fields: vec!["date".into()],
            }),
        },
        (300.0, 500.0),
        (100.0, 20.0),
    );
    let city = field_box(
        "ville",
        FieldKind::Choice {
            options: vec!["Paris".into(), "Lyon".into()],
            editable: true,
            list: false,
            multi: false,
        },
        (72.0, 400.0),
        (150.0, 20.0),
    );
    let signature = field_box(
        "signature",
        FieldKind::Signature,
        (300.0, 400.0),
        (180.0, 60.0),
    );
    let boxes = [
        date,
        ok,
        radio("M", 72.0),
        radio("Mme", 120.0),
        total,
        city,
        signature,
    ];
    let values = [
        ("date", FieldValue::Text("19/11/2026".into())),
        ("civilite", FieldValue::Choice("Mme".into())),
    ];
    let placed = engine()
        .apply_edits(doc.id, edition(pages, &boxes, &values))
        .expect("champs créés");
    assert_eq!(placed.len(), boxes.len());
    assert_eq!(placed[0].bounds.min(), Point::new(72.0, 480.0));
    // Case cochée dessinée par son apparence.
    assert!(
        dark_pixels(doc.id, (75, 85), (448, 458)) > 5,
        "coche dessinée"
    );

    let saved = reopen(doc.id);
    let fields = fields_of(&saved);
    assert_eq!(fields.len(), 6, "{fields:#?}");
    let date = field(&fields, "date");
    assert!(date.required);
    assert_eq!(date.tooltip, "Date du contrat");
    assert_eq!(date.kind, date_kind());
    assert_eq!(date.value, Some(FieldValue::Text("19/11/2026".into())));
    let b = date.widgets[0].bounds;
    assert_eq!(
        (b.min().x, b.min().y, b.max().x, b.max().y),
        (72.0, 480.0, 222.0, 500.0)
    );
    assert_eq!(
        field(&fields, "accord").value,
        Some(FieldValue::Checked(true))
    );
    assert_eq!(
        field(&fields, "accord").widgets[0].export.as_deref(),
        Some("Oui")
    );
    let civility = field(&fields, "civilite");
    assert_eq!(
        civility.kind,
        FieldKind::Radio {
            options: vec!["M".into(), "Mme".into()]
        }
    );
    assert_eq!(civility.value, Some(FieldValue::Choice("Mme".into())));
    let total = field(&fields, "total");
    assert_eq!(total.kind.calc().map(|c| c.op), Some(CalcOp::Sum));
    assert!(matches!(
        total.kind.format(),
        Some(FieldFormat::Number { decimals: 2, .. })
    ));
    assert!(matches!(
        field(&fields, "ville").kind,
        FieldKind::Choice { editable: true, .. }
    ));
    assert_eq!(field(&fields, "signature").kind, FieldKind::Signature);
    engine().close(saved.id);
    engine().close(doc.id);
}

#[test]
fn document_fields_are_moved_renamed_set_and_deleted() {
    let doc = engine().open(FORM.to_vec(), None).expect("formulaire");
    let mut name = field_box(
        "nom_complet",
        FieldKind::text(),
        (120.0, 760.0),
        (250.0, 24.0),
    );
    name.origin = Some(FieldOrigin {
        name: "nom".into(),
        widget: 0,
    });
    name.required = true;
    name.kind = FieldKind::Text {
        multiline: false,
        max_len: Some(40),
        password: false,
        comb: false,
        format: None,
        calc: None,
    };
    let mut ok = field_box("accord", FieldKind::Checkbox, (120.0, 665.0), (15.0, 15.0));
    ok.origin = Some(FieldOrigin {
        name: "accord".into(),
        widget: 0,
    });
    ok.deleted = true;
    let mut mme = field_box(
        "civilite",
        FieldKind::Radio {
            options: Vec::new(),
        },
        (200.0, 615.0),
        (15.0, 15.0),
    );
    mme.origin = Some(FieldOrigin {
        name: "civilite".into(),
        widget: 1,
    });
    let values = [("nom_complet", FieldValue::Text("Jean".into()))];
    engine()
        .apply_edits(doc.id, edition(1, &[name, ok, mme], &values))
        .expect("champs modifiés");
    let saved = reopen(doc.id);
    let fields = fields_of(&saved);
    let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["nom_complet", "civilite", "ville"]);
    let name = field(&fields, "nom_complet");
    assert!(name.required);
    assert_eq!(name.value, Some(FieldValue::Text("Jean".into())));
    assert!(matches!(
        name.kind,
        FieldKind::Text {
            max_len: Some(40),
            ..
        }
    ));
    assert_eq!(name.widgets[0].bounds.min(), Point::new(120.0, 736.0));
    let civility = field(&fields, "civilite");
    let moved = civility.widgets.iter().find(|w| w.index == 1).expect("Mme");
    assert_eq!(moved.bounds.min(), Point::new(200.0, 600.0));
    assert_eq!(moved.export.as_deref(), Some("Mme"));
    engine().close(saved.id);
    engine().close(doc.id);
}

#[test]
fn a_renamed_field_keeps_its_parent_name() {
    let doc = engine()
        .open(common::SMART_FORM.to_vec(), None)
        .expect("formulaire");
    let mut city = field_box("ville", FieldKind::text(), (72.0, 438.0), (160.0, 18.0));
    city.origin = Some(FieldOrigin {
        name: "client.ville".into(),
        widget: 0,
    });
    let error = engine()
        .apply_edits(doc.id, edition(1, &[city.clone()], &[]))
        .unwrap_err();
    assert!(error.to_string().contains("client."), "{error}");
    city.name = "client.commune".into();
    engine()
        .apply_edits(doc.id, edition(1, &[city], &[]))
        .expect("renommé");
    let saved = reopen(doc.id);
    assert!(fields_of(&saved).iter().any(|f| f.name == "client.commune"));
    engine().close(saved.id);
    engine().close(doc.id);
}
