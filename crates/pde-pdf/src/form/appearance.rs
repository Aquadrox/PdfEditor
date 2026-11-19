//! Apparences des champs créés que les lecteurs ne dessinent pas d'eux-
//! mêmes : cases à cocher et boutons radio (états « coché » et `Off`),
//! emplacement de signature. Les champs texte et les listes n'en ont pas :
//! le formulaire demande aux lecteurs de les dessiner (`NeedAppearances`).

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use pde_core::edit::FieldLook;

use super::look::{fill_operator, stroke_operator};

/// Courbes de Bézier d'un cercle de centre (cx, cy) et de rayon r.
fn circle(cx: f64, cy: f64, r: f64) -> String {
    let k = 0.552_284_75 * r;
    format!(
        "{x0:.2} {cy:.2} m {x0:.2} {a:.2} {b:.2} {y1:.2} {cx:.2} {y1:.2} c \
         {c:.2} {y1:.2} {x1:.2} {a:.2} {x1:.2} {cy:.2} c \
         {x1:.2} {d:.2} {c:.2} {y0:.2} {cx:.2} {y0:.2} c \
         {b:.2} {y0:.2} {x0:.2} {d:.2} {x0:.2} {cy:.2} c h",
        x0 = cx - r,
        x1 = cx + r,
        y0 = cy - r,
        y1 = cy + r,
        a = cy + k,
        b = cx - k,
        c = cx + k,
        d = cy - k,
    )
}

/// Fond et contour d'un champ de `w` × `h` (rond pour un bouton radio).
pub(super) fn frame(look: &FieldLook, w: f64, h: f64, round: bool) -> String {
    let mut out = String::new();
    let shape = |inset: f64| {
        if round {
            circle(w / 2.0, h / 2.0, (w.min(h) / 2.0 - inset).max(0.5))
        } else {
            format!(
                "{inset:.2} {inset:.2} {:.2} {:.2} re",
                (w - 2.0 * inset).max(0.5),
                (h - 2.0 * inset).max(0.5)
            )
        }
    };
    if let Some(background) = look.background {
        out += &format!("{} {} f\n", fill_operator(background), shape(0.0));
    }
    if let Some(border) = look.border {
        out += &format!("{} 1 w {} S\n", stroke_operator(border), shape(0.5));
    }
    out
}

pub(super) fn form_xobject(
    doc: &mut Document,
    w: f64,
    h: f64,
    content: String,
    zadb: bool,
) -> ObjectId {
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XObject".to_vec()));
    dict.set("Subtype", Object::Name(b"Form".to_vec()));
    dict.set(
        "BBox",
        Object::Array(vec![
            0.into(),
            0.into(),
            Object::Real(w as f32),
            Object::Real(h as f32),
        ]),
    );
    if zadb {
        let mut font = Dictionary::new();
        font.set("Type", Object::Name(b"Font".to_vec()));
        font.set("Subtype", Object::Name(b"Type1".to_vec()));
        font.set("BaseFont", Object::Name(b"ZapfDingbats".to_vec()));
        let mut fonts = Dictionary::new();
        fonts.set("ZaDb", Object::Dictionary(font));
        let mut resources = Dictionary::new();
        resources.set("Font", Object::Dictionary(fonts));
        dict.set("Resources", Object::Dictionary(resources));
    }
    doc.add_object(Stream::new(dict, content.into_bytes()))
}

/// Apparence `/AP` d'une case à cocher (coche) ou d'un bouton radio
/// (point), états `export` et `Off`.
pub(crate) fn button(
    doc: &mut Document,
    look: &FieldLook,
    (w, h): (f64, f64),
    export: &str,
    radio: bool,
) -> Dictionary {
    let base = frame(look, w, h, radio);
    let ink = fill_operator(look.color);
    let mark = if radio {
        format!("{ink} {} f\n", circle(w / 2.0, h / 2.0, w.min(h) * 0.22))
    } else {
        // Coche « 4 » de ZapfDingbats (largeur 0,846 em), centrée.
        let size = w.min(h) * 0.75;
        format!(
            "q BT {ink} /ZaDb {size:.2} Tf {:.2} {:.2} Td (4) Tj ET Q\n",
            (w - size * 0.846) / 2.0,
            (h - size * 0.7) / 2.0
        )
    };
    let on = form_xobject(doc, w, h, format!("{base}{mark}"), !radio);
    let off = form_xobject(doc, w, h, base, false);
    let mut normal = Dictionary::new();
    normal.set(export.as_bytes().to_vec(), on);
    normal.set("Off", off);
    let mut ap = Dictionary::new();
    ap.set("N", Object::Dictionary(normal));
    ap
}

/// Apparence d'un emplacement de signature vide : son cadre.
pub(crate) fn signature(doc: &mut Document, look: &FieldLook, (w, h): (f64, f64)) -> Dictionary {
    let content = frame(look, w, h, false);
    let normal = form_xobject(doc, w, h, content, false);
    let mut ap = Dictionary::new();
    ap.set("N", normal);
    ap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_have_a_checked_and_an_off_state() {
        let mut doc = Document::with_version("1.7");
        let ap = button(&mut doc, &FieldLook::default(), (15.0, 15.0), "Oui", false);
        let normal = ap.get(b"N").unwrap().as_dict().unwrap();
        assert!(normal.has(b"Oui") && normal.has(b"Off"));
        let on = normal.get(b"Oui").unwrap().as_reference().unwrap();
        let stream = doc.get_object(on).unwrap().as_stream().unwrap();
        assert!(String::from_utf8_lossy(&stream.content).contains("(4) Tj"));
        let ap = button(&mut doc, &FieldLook::default(), (15.0, 15.0), "Mme", true);
        assert!(ap.get(b"N").unwrap().as_dict().unwrap().has(b"Mme"));
        assert!(circle(5.0, 5.0, 5.0).starts_with("0.00 5.00 m"));
    }
}
