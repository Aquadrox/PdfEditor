//! Apparence des champs texte et des listes, dessinée par PdfEditor à
//! partir de leur valeur : texte en Helvetica (WinAnsi), sur une ligne,
//! sur plusieurs lignes coupées à la largeur, en cases (peigne), ou lignes
//! d'une liste avec les options choisies surlignées. Un champ mis en forme
//! (montant, pourcentage) montre sa valeur telle qu'un lecteur l'affiche.
//! Sans elle, « Aplatir » perdait le texte saisi : PDFium n'aplatit que
//! les apparences présentes dans le fichier.

use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use pde_core::edit::{Align, FieldLook, FieldValue, wrap};
use pde_core::engine::EngineError;
use pde_core::formats::read_actions;
use pde_core::helvetica::{pdf_literal, text_width, win_ansi};

use super::appearance::frame;
use super::look::fill_operator;
use super::structure::{FieldNode, NodeKind, acroform, dict};
use super::tree::dict_mut;

/// Ce que montre le champ.
enum Shown {
    Line(String),
    Lines(String),
    Comb(String, u32),
    List(Vec<String>, Vec<usize>),
}

/// Corps d'une ligne qui tient dans `w` × `h` (corps automatique).
fn auto_size(text: &str, w: f64, h: f64) -> f64 {
    let size = ((h - 4.0) * 0.8).clamp(4.0, 12.0);
    let width = text_width(text, size);
    if width > w - 4.0 && width > 0.0 {
        (size * (w - 4.0) / width).max(4.0)
    } else {
        size
    }
}

fn x_for(align: Align, width: f64, w: f64) -> f64 {
    match align {
        Align::Left => 2.0,
        Align::Center => (w - width) / 2.0,
        Align::Right => w - 2.0 - width,
    }
}

fn show(out: &mut Vec<u8>, x: f64, y: f64, text: &str) {
    out.extend_from_slice(format!("1 0 0 1 {x:.2} {y:.2} Tm ").as_bytes());
    out.extend(pdf_literal(&win_ansi(text)));
    out.extend_from_slice(b" Tj\n");
}

fn content(look: &FieldLook, (w, h): (f64, f64), shown: &Shown) -> Vec<u8> {
    let mut out = b"/Tx BMC\n".to_vec();
    out.extend_from_slice(frame(look, w, h, false).as_bytes());
    out.extend_from_slice(format!("q 1 1 {:.2} {:.2} re W n\n", w - 2.0, h - 2.0).as_bytes());
    let fixed = (look.font_size > 0.0).then_some(look.font_size);
    let size = match shown {
        Shown::Line(text) => fixed.unwrap_or_else(|| auto_size(text, w, h)),
        _ => fixed.unwrap_or_else(|| 10.0_f64.min(h - 2.0).max(4.0)),
    };
    let leading = size * 1.15;
    let mut text = Vec::new();
    match shown {
        Shown::Line(line) => {
            let x = x_for(look.align, text_width(line, size), w);
            show(&mut text, x, (h - size) / 2.0 + size * 0.22, line);
        }
        Shown::Lines(all) => {
            let lines = wrap(all, w - 4.0, |s| text_width(s, size));
            for (i, line) in lines.iter().enumerate() {
                let y = h - 2.0 - size * 0.93 - leading * i as f64;
                if y < -size {
                    break;
                }
                show(
                    &mut text,
                    x_for(look.align, text_width(line, size), w),
                    y,
                    line,
                );
            }
        }
        Shown::Comb(value, cells) => {
            let cell = w / f64::from((*cells).max(1));
            for (i, c) in value.chars().take(*cells as usize).enumerate() {
                let s = c.to_string();
                let x = cell * i as f64 + (cell - text_width(&s, size)) / 2.0;
                show(&mut text, x, (h - size) / 2.0 + size * 0.22, &s);
            }
        }
        Shown::List(options, chosen) => {
            for (i, option) in options.iter().enumerate() {
                let top = h - 1.0 - leading * i as f64;
                if top < 0.0 {
                    break;
                }
                if chosen.contains(&i) {
                    out.extend_from_slice(
                        format!(
                            "0.600 0.757 0.855 rg 1 {:.2} {:.2} {:.2} re f\n",
                            top - leading,
                            w - 2.0,
                            leading
                        )
                        .as_bytes(),
                    );
                }
                show(&mut text, 2.0, top - size * 0.93, option);
            }
        }
    }
    out.extend_from_slice(
        format!("BT /Helv {size:.2} Tf {}\n", fill_operator(look.color)).as_bytes(),
    );
    out.extend(text);
    out.extend_from_slice(b"ET\nQ\nEMC\n");
    out
}

/// Ce que montre `node` selon sa valeur ; `None` : rien à dessiner
/// (mot de passe, autre genre).
fn shown(node: &FieldNode) -> Option<Shown> {
    let value = node.value.as_ref();
    match &node.kind {
        NodeKind::Text { password: true, .. } => None,
        NodeKind::Text {
            multiline,
            comb,
            max_len,
            ..
        } => {
            let raw = value.map(FieldValue::text).unwrap_or_default();
            let text = match read_actions(&node.actions).0 {
                Some(format) if !raw.is_empty() => format.display(raw),
                _ => raw.to_string(),
            };
            Some(match (comb, max_len) {
                (true, Some(cells)) => Shown::Comb(text, *cells),
                _ if *multiline => Shown::Lines(text),
                _ => Shown::Line(text),
            })
        }
        NodeKind::Choice { options, list, .. } => {
            let chosen: Vec<String> = match value {
                Some(FieldValue::Choices(items)) => items.clone(),
                Some(v) => vec![v.text().to_string()],
                None => Vec::new(),
            };
            Some(if *list {
                let indices = options
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| chosen.contains(o))
                    .map(|(i, _)| i)
                    .collect();
                Shown::List(options.clone(), indices)
            } else {
                Shown::Line(chosen.first().cloned().unwrap_or_default())
            })
        }
        _ => None,
    }
}

/// Police `/Helv` du formulaire.
fn helv(doc: &Document) -> Option<Object> {
    let form = acroform(doc)?;
    let (_, dr) = dict(doc, form.get(b"DR").ok()?)?;
    let (_, fonts) = dict(doc, dr.get(b"Font").ok()?)?;
    fonts.get(b"Helv").ok().cloned()
}

/// Dessine l'apparence des champs texte et listes de `nodes` qui n'en ont
/// pas (valeur saisie, champ créé ou déplacé), sur les pages droites ;
/// ailleurs, les lecteurs la dessinent (`NeedAppearances`).
pub(crate) fn draw_missing(doc: &mut Document, nodes: &[FieldNode]) -> Result<(), EngineError> {
    let Some(font) = helv(doc) else {
        return Ok(());
    };
    for node in nodes {
        let Some(shown) = shown(node) else {
            continue;
        };
        for (widget, rect) in node.widgets.iter().zip(&node.rects) {
            let Some([x0, y0, x1, y1]) = *rect else {
                continue;
            };
            let Ok(w) = doc.get_dictionary(*widget) else {
                continue;
            };
            let rotated = w
                .get(b"MK")
                .ok()
                .and_then(|mk| dict(doc, mk))
                .and_then(|(_, mk)| mk.get(b"R").ok()?.as_i64().ok())
                .is_some_and(|r| r % 360 != 0);
            if w.has(b"AP") || rotated {
                continue;
            }
            let size = (x1 - x0, y1 - y0);
            let mut xobject = Dictionary::new();
            xobject.set("Type", Object::Name(b"XObject".to_vec()));
            xobject.set("Subtype", Object::Name(b"Form".to_vec()));
            xobject.set(
                "BBox",
                Object::Array(vec![
                    0.into(),
                    0.into(),
                    Object::Real(size.0 as f32),
                    Object::Real(size.1 as f32),
                ]),
            );
            let mut fonts = Dictionary::new();
            fonts.set("Helv", font.clone());
            let mut resources = Dictionary::new();
            resources.set("Font", Object::Dictionary(fonts));
            xobject.set("Resources", Object::Dictionary(resources));
            let stream: ObjectId =
                doc.add_object(Stream::new(xobject, content(&node.look, size, &shown)));
            let mut ap = Dictionary::new();
            ap.set("N", stream);
            dict_mut(doc, *widget)?.set("AP", Object::Dictionary(ap));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    #[test]
    fn single_line_shrinks_to_fit_and_follows_alignment() {
        let look = FieldLook {
            align: Align::Right,
            ..FieldLook::default()
        };
        let out = text_of(&content(&look, (100.0, 20.0), &Shown::Line("Jean".into())));
        assert!(out.starts_with("/Tx BMC"), "{out}");
        assert!(out.contains("(Jean) Tj"));
        assert!(out.contains("/Helv 12.00 Tf"));
        assert_eq!(auto_size(&"W".repeat(50), 100.0, 20.0), 4.0);
        assert!(auto_size("abc", 100.0, 10.0) < 6.0);
    }

    #[test]
    fn several_lines_combs_and_lists() {
        let look = FieldLook::default();
        let out = text_of(&content(
            &look,
            (60.0, 60.0),
            &Shown::Lines("un deux trois quatre cinq".into()),
        ));
        assert!(out.matches(" Tj").count() >= 2, "{out}");
        let out = text_of(&content(
            &look,
            (60.0, 20.0),
            &Shown::Comb("75001".into(), 5),
        ));
        assert_eq!(out.matches(" Tj").count(), 5);
        let out = text_of(&content(
            &look,
            (60.0, 60.0),
            &Shown::List(vec!["fr".into(), "en".into()], vec![1]),
        ));
        assert_eq!(out.matches(" re f").count(), 1);
    }
}
