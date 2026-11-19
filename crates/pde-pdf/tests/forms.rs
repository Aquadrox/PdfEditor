//! Formulaire (E-08) et propriétés du document (E-11) sur un vrai PDF :
//! champs listés, remplis, redessinés, enregistrés, aplatis. Nécessite
//! PDFium (`scripts/fetch-pdfium.sh`).

mod common;

use std::collections::BTreeMap;

use common::{ENCRYPTED, FORM, SMART_FORM, TEST_PASSWORD, engine};
use pde_core::document::DocumentId;
use pde_core::edit::{Align, FieldKind, FieldValue, FormField, Images, Metadata, PagePlan};
use pde_core::engine::{Edition, OpenedDocument, PdfEngine};
use pde_core::formats::{CalcOp, FieldFormat, Separator};
use pde_core::render::{Bitmap, RenderRequest};

fn open_form() -> OpenedDocument {
    engine().open(FORM.to_vec(), None).expect("formulaire")
}

fn edition(fields: &[(&str, FieldValue)], metadata: Option<Metadata>, flatten: bool) -> Edition {
    Edition {
        plan: PagePlan::identity(1),
        fields: fields
            .iter()
            .map(|(n, v)| ((*n).to_string(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
        metadata,
        flatten,
        ..Edition::default()
    }
}

fn fields_of(doc: DocumentId) -> Vec<FormField> {
    engine()
        .form_fields(doc, edition(&[], None, false))
        .expect("champs")
}

fn field<'a>(fields: &'a [FormField], name: &str) -> &'a FormField {
    fields.iter().find(|f| f.name == name).expect(name)
}

fn render(doc: DocumentId) -> Bitmap {
    engine()
        .render(
            doc,
            RenderRequest {
                page: 0,
                scale: 1.0,
            },
        )
        .expect("rendu")
}

/// Pixels sombres dans [x0, x1] × [y0, y1] (page affichée, origine en bas).
fn dark(bitmap: &Bitmap, (x0, x1): (u32, u32), (y0, y1): (u32, u32)) -> usize {
    (x0..x1)
        .flat_map(|x| (y0..y1).map(move |y| (x, y)))
        .filter(|&(x, y)| {
            bitmap
                .pixel(x, bitmap.height() - y)
                .is_some_and(|p| p[0] < 110 && p[1] < 110 && p[2] < 110)
        })
        .count()
}

fn reopen(doc: DocumentId) -> OpenedDocument {
    let bytes = engine().save(doc).expect("enregistrement");
    engine().open(bytes, None).expect("réouverture")
}

#[test]
fn fields_are_listed_with_their_kind_value_and_place() {
    let doc = open_form();
    let fields = fields_of(doc.id);
    assert_eq!(fields.len(), 4, "{fields:?}");
    let name = field(&fields, "nom");
    assert!(matches!(
        name.kind,
        FieldKind::Text {
            multiline: false,
            ..
        }
    ));
    assert_eq!(name.value, Some(FieldValue::Text("Ancien".into())));
    let b = name.widgets[0].bounds;
    assert_eq!(
        (b.min().x, b.min().y, b.max().x, b.max().y),
        (120.0, 700.0, 320.0, 722.0)
    );
    assert_eq!(
        field(&fields, "accord").value,
        Some(FieldValue::Checked(false))
    );
    let civility = field(&fields, "civilite");
    assert_eq!(
        civility.kind,
        FieldKind::Radio {
            options: vec!["M".into(), "Mme".into()]
        }
    );
    assert_eq!(civility.widgets.len(), 2);
    assert_eq!(civility.widgets[1].option, Some(1));
    let city = field(&fields, "ville");
    assert_eq!(
        city.kind,
        FieldKind::Choice {
            options: vec!["Paris".into(), "Lyon".into()],
            editable: false,
            list: false,
            multi: false,
        }
    );
    assert_eq!(doc.info.metadata.title, "Formulaire de test");
    engine().close(doc.id);
}

#[test]
fn filled_values_are_drawn_saved_and_read_back() {
    let doc = open_form();
    let before = render(doc.id);
    assert_eq!(dark(&before, (124, 131), (654, 661)), 0, "case vide");
    let values = [
        ("nom", FieldValue::Text("Jean Dupont".into())),
        ("accord", FieldValue::Checked(true)),
        ("civilite", FieldValue::Choice("Mme".into())),
        ("ville", FieldValue::Choice("Lyon".into())),
    ];
    engine()
        .apply_edits(doc.id, edition(&values, None, false))
        .expect("champs remplis");
    let after = render(doc.id);
    // PDFium redessine les champs dans son style (contour, coche) : on
    // compare l'intérieur des boutons cochés et non cochés.
    assert!(
        dark(&after, (124, 131), (654, 661)) > 5,
        "case cochée dessinée"
    );
    assert!(
        dark(&after, (164, 171), (604, 611)) > 5,
        "radio Mme dessinée"
    );
    assert_eq!(dark(&after, (124, 131), (604, 611)), 0, "radio M vide");
    assert!(dark(&after, (122, 318), (702, 720)) > 30, "texte dessiné");

    let saved = reopen(doc.id);
    let fields = fields_of(saved.id);
    assert_eq!(
        field(&fields, "nom").value,
        Some(FieldValue::Text("Jean Dupont".into()))
    );
    assert_eq!(
        field(&fields, "accord").value,
        Some(FieldValue::Checked(true))
    );
    assert_eq!(
        field(&fields, "civilite").value,
        Some(FieldValue::Choice("Mme".into()))
    );
    assert_eq!(
        field(&fields, "ville").value,
        Some(FieldValue::Choice("Lyon".into()))
    );
    engine().close(saved.id);
    engine().close(doc.id);
}

#[test]
fn unknown_field_or_option_is_refused() {
    let doc = open_form();
    let wrong = [("absent", FieldValue::Text("x".into()))];
    assert!(
        engine()
            .apply_edits(doc.id, edition(&wrong, None, false))
            .is_err()
    );
    let wrong = [("civilite", FieldValue::Choice("Docteur".into()))];
    assert!(
        engine()
            .apply_edits(doc.id, edition(&wrong, None, false))
            .is_err()
    );
    engine().close(doc.id);
}

#[test]
fn properties_are_written_and_read_back() {
    let doc = open_form();
    let metadata = Metadata {
        title: "Contrat signé".into(),
        author: "Société Exemple".into(),
        subject: String::new(),
        keywords: "contrat, été".into(),
    };
    engine()
        .apply_edits(doc.id, edition(&[], Some(metadata.clone()), false))
        .expect("propriétés");
    let saved = reopen(doc.id);
    assert_eq!(saved.info.metadata, metadata);
    assert_eq!(saved.info.title.as_deref(), Some("Contrat signé"));
    engine().close(saved.id);
    engine().close(doc.id);
}

#[test]
fn a_flattened_form_keeps_its_look_without_fields() {
    let doc = open_form();
    let values = [("accord", FieldValue::Checked(true))];
    engine()
        .apply_edits(doc.id, edition(&values, None, true))
        .expect("aplati");
    assert!(
        dark(&render(doc.id), (122, 133), (652, 663)) > 20,
        "case toujours dessinée"
    );
    let saved = reopen(doc.id);
    assert!(fields_of(saved.id).is_empty(), "plus de champs");
    assert!(dark(&render(saved.id), (122, 133), (652, 663)) > 20);
    engine().close(saved.id);
    engine().close(doc.id);
}

#[test]
fn a_protected_document_refuses_values_and_properties() {
    let doc = engine()
        .open(ENCRYPTED.to_vec(), Some(TEST_PASSWORD.into()))
        .expect("ouverture");
    let plan = PagePlan::identity(doc.info.page_count());
    let with_title = Edition {
        plan,
        metadata: Some(Metadata {
            title: "x".into(),
            ..Metadata::default()
        }),
        ..Edition::new(PagePlan::default(), Vec::new(), Images::new())
    };
    let error = engine().apply_edits(doc.id, with_title).unwrap_err();
    assert!(error.to_string().contains("mot de passe"), "{error}");
    engine().close(doc.id);
}

#[test]
fn smart_fields_are_recognized_from_their_scripts_and_flags() {
    let doc = engine()
        .open(SMART_FORM.to_vec(), None)
        .expect("formulaire");
    let fields = fields_of(doc.id);
    let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
    // Le bouton poussoir n'est pas un champ à remplir.
    assert!(!names.contains(&"imprimer"), "{names:?}");
    let date = field(&fields, "date");
    assert!(date.required);
    assert_eq!(date.tooltip, "Date de naissance");
    assert_eq!(date.look.font_size, 10.0);
    assert_eq!(date.look.align, Align::Center);
    assert_eq!(date.look.border.map(|c| c.b), Some(255));
    assert_eq!(
        date.kind.format(),
        Some(&FieldFormat::Date {
            pattern: "dd/mm/yyyy".into()
        })
    );
    assert_eq!(
        field(&fields, "prix").kind.format(),
        Some(&FieldFormat::Number {
            decimals: 2,
            separator: Separator::Comma,
            currency: " €".into(),
            currency_before: false,
        })
    );
    let total = field(&fields, "total");
    assert!(total.read_only);
    let calc = total.kind.calc().expect("calcul");
    assert_eq!(
        (calc.op, calc.fields.clone()),
        (CalcOp::Sum, vec!["prix".into(), "quantite".into()])
    );
    assert_eq!(
        field(&fields, "pays").kind,
        FieldKind::Choice {
            options: vec!["France".into(), "Belgique".into()],
            editable: true,
            list: false,
            multi: false,
        }
    );
    assert_eq!(
        field(&fields, "pays").value,
        Some(FieldValue::Choice("Suisse".into()))
    );
    let languages = field(&fields, "langues");
    assert!(matches!(
        languages.kind,
        FieldKind::Choice {
            list: true,
            multi: true,
            ..
        }
    ));
    assert_eq!(
        languages.value,
        Some(FieldValue::Choices(vec!["fr".into(), "de".into()]))
    );
    assert!(matches!(
        field(&fields, "secret").kind,
        FieldKind::Text { password: true, .. }
    ));
    assert_eq!(field(&fields, "signature").kind, FieldKind::Signature);
    let city = field(&fields, "client.ville");
    assert!(matches!(
        city.kind,
        FieldKind::Text {
            max_len: Some(30),
            ..
        }
    ));
    assert_eq!(city.widgets[0].index, 0);
    engine().close(doc.id);
}

#[test]
fn several_options_are_chosen_and_read_back() {
    let doc = engine()
        .open(SMART_FORM.to_vec(), None)
        .expect("formulaire");
    let values = [
        (
            "langues",
            FieldValue::Choices(vec!["en".into(), "de".into()]),
        ),
        ("pays", FieldValue::Choice("Italie".into())),
    ];
    engine()
        .apply_edits(doc.id, edition(&values, None, false))
        .expect("listes remplies");
    let saved = reopen(doc.id);
    let fields = fields_of(saved.id);
    assert_eq!(
        field(&fields, "langues").value,
        Some(FieldValue::Choices(vec!["en".into(), "de".into()]))
    );
    assert_eq!(
        field(&fields, "pays").value,
        Some(FieldValue::Choice("Italie".into()))
    );
    engine().close(saved.id);
    engine().close(doc.id);
}

#[test]
fn flattened_amounts_show_their_format() {
    let doc = engine()
        .open(SMART_FORM.to_vec(), None)
        .expect("formulaire");
    let values = [("prix", FieldValue::Text("1234.5".into()))];
    engine()
        .apply_edits(doc.id, edition(&values, None, true))
        .expect("aplati");
    let text: String = engine()
        .page_text(doc.id, 0)
        .expect("texte")
        .chars
        .iter()
        .map(|c| c.ch)
        .collect();
    assert!(text.contains("1234,50"), "{text}");
    engine().close(doc.id);
}

#[test]
fn a_flattened_text_field_keeps_its_typed_value() {
    let doc = open_form();
    let values = [("nom", FieldValue::Text("Jean Dupont".into()))];
    engine()
        .apply_edits(doc.id, edition(&values, None, true))
        .expect("aplati");
    assert!(
        dark(&render(doc.id), (122, 318), (702, 720)) > 30,
        "texte dessiné"
    );
    let saved = reopen(doc.id);
    assert!(
        dark(&render(saved.id), (122, 318), (702, 720)) > 30,
        "texte gardé"
    );
    engine().close(saved.id);
    engine().close(doc.id);
}

/// Apparence enregistrée du premier emplacement du champ `name`.
fn saved_appearance(bytes: &[u8], name: &str) -> String {
    let doc = lopdf::Document::load_mem(bytes).expect("lecture");
    for object in doc.objects.values() {
        let Ok(dict) = object.as_dict() else { continue };
        let Ok(title) = dict.get(b"T").and_then(lopdf::Object::as_str) else {
            continue;
        };
        if title != name.as_bytes() {
            continue;
        }
        let ap = dict
            .get(b"AP")
            .and_then(lopdf::Object::as_dict)
            .expect("AP");
        let id = ap
            .get(b"N")
            .and_then(lopdf::Object::as_reference)
            .expect("N");
        let stream = doc
            .get_object(id)
            .and_then(lopdf::Object::as_stream)
            .expect("flux");
        let content = stream
            .decompressed_content()
            .unwrap_or_else(|_| stream.content.clone());
        return String::from_utf8_lossy(&content).into_owned();
    }
    panic!("champ {name} absent");
}

#[test]
fn a_filled_amount_is_kept_plain_and_shown_formatted() {
    let doc = engine()
        .open(SMART_FORM.to_vec(), None)
        .expect("formulaire");
    let values = [("prix", FieldValue::Text("1234.5".into()))];
    engine()
        .apply_edits(doc.id, edition(&values, None, false))
        .expect("rempli");
    let bytes = engine().save(doc.id).expect("enregistrement");
    let shown = saved_appearance(&bytes, "prix");
    assert!(
        shown.contains("(1234,50 \u{80}) Tj") || shown.contains("1234,50"),
        "{shown}"
    );
    let saved = engine().open(bytes, None).expect("réouverture");
    assert_eq!(
        field(&fields_of(saved.id), "prix").value,
        Some(FieldValue::Text("1234.5".into()))
    );
    engine().close(saved.id);
    engine().close(doc.id);
}
