//! Apparence d'un champ : apparence par défaut (`/DA` : police, corps,
//! couleur du texte), alignement (`/Q`) et couleurs du contour et du fond
//! (`/MK` : `/BC`, `/BG`), lues et écrites.

use lopdf::{Dictionary, Document, Object};
use pde_core::edit::{Align, Color, FieldLook};

/// Composantes d'une couleur PDF (gris, RVB, CMJN) → couleur RVB.
fn color_of(values: &[f64]) -> Option<Color> {
    let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    match values {
        [g] => Some(Color {
            r: byte(*g),
            g: byte(*g),
            b: byte(*g),
        }),
        [r, g, b] => Some(Color {
            r: byte(*r),
            g: byte(*g),
            b: byte(*b),
        }),
        [c, m, y, k] => Some(Color {
            r: byte((1.0 - c) * (1.0 - k)),
            g: byte((1.0 - m) * (1.0 - k)),
            b: byte((1.0 - y) * (1.0 - k)),
        }),
        _ => None,
    }
}

fn numbers(doc: &Document, object: &Object) -> Option<Vec<f64>> {
    let (_, object) = doc.dereference(object).ok()?;
    Some(
        object
            .as_array()
            .ok()?
            .iter()
            .filter_map(|v| v.as_float().ok().map(f64::from))
            .collect(),
    )
}

/// Corps et couleur lus dans une apparence par défaut (« /Helv 12 Tf 0 g »).
fn parse_da(da: &str) -> (f64, Color) {
    let mut size = 0.0;
    let mut color = Color { r: 0, g: 0, b: 0 };
    let mut operands: Vec<f64> = Vec::new();
    for word in da.split_whitespace() {
        match word {
            "Tf" => {
                size = operands.last().copied().unwrap_or(0.0);
                operands.clear();
            }
            "g" | "rg" | "k" => {
                let count = match word {
                    "g" => 1,
                    "rg" => 3,
                    _ => 4,
                };
                if operands.len() >= count
                    && let Some(c) = color_of(&operands[operands.len() - count..])
                {
                    color = c;
                }
                operands.clear();
            }
            other => match other.parse::<f64>() {
                Ok(n) => operands.push(n),
                Err(_) => {
                    if !other.starts_with('/') {
                        operands.clear();
                    }
                }
            },
        }
    }
    let size = if size.is_finite() && size >= 0.0 {
        size
    } else {
        0.0
    };
    (size, color)
}

/// Apparence d'un champ ; `widget` : son premier emplacement.
pub(crate) fn read(
    doc: &Document,
    da: Option<&str>,
    quadding: Option<i64>,
    widget: Option<&Dictionary>,
) -> FieldLook {
    let (font_size, color) = da
        .map(parse_da)
        .unwrap_or((0.0, Color { r: 0, g: 0, b: 0 }));
    let align = match quadding {
        Some(1) => Align::Center,
        Some(2) => Align::Right,
        _ => Align::Left,
    };
    let mk = widget
        .and_then(|w| w.get(b"MK").ok())
        .and_then(|mk| super::structure::dict(doc, mk))
        .map(|(_, d)| d);
    let mk_color = |key: &[u8]| {
        mk.and_then(|d| d.get(key).ok())
            .and_then(|c| numbers(doc, c))
            .and_then(|c| color_of(&c))
    };
    FieldLook {
        font_size: if pde_core::edit::FIELD_FONT_RANGE.contains(&font_size) {
            font_size
        } else {
            0.0
        },
        align,
        color,
        border: mk_color(b"BC"),
        background: mk_color(b"BG"),
    }
}

fn components(color: Color) -> Vec<Object> {
    [color.r, color.g, color.b]
        .iter()
        .map(|c| Object::Real(f32::from(*c) / 255.0))
        .collect()
}

/// Opérateur de couleur de remplissage (« 0.1 0.2 0.3 rg »).
pub(crate) fn fill_operator(color: Color) -> String {
    let c = |v: u8| format!("{:.3}", f64::from(v) / 255.0);
    format!("{} {} {} rg", c(color.r), c(color.g), c(color.b))
}

/// Opérateur de couleur de trait (« 0.1 0.2 0.3 RG »).
pub(crate) fn stroke_operator(color: Color) -> String {
    fill_operator(color).replace("rg", "RG")
}

/// Apparence par défaut écrite pour `look` (police Helvetica du formulaire).
pub(crate) fn da(look: &FieldLook) -> String {
    let size = if look.font_size > 0.0 {
        format!("{}", look.font_size)
    } else {
        "0".into()
    };
    format!("/Helv {size} Tf {}", fill_operator(look.color))
}

pub(crate) fn quadding(look: &FieldLook) -> i64 {
    match look.align {
        Align::Left => 0,
        Align::Center => 1,
        Align::Right => 2,
    }
}

/// Dictionnaire `/MK` : couleurs, rotation, légende d'un bouton.
pub(crate) fn mk(look: &FieldLook, rotation: i64, caption: Option<&str>) -> Dictionary {
    let mut mk = Dictionary::new();
    if let Some(border) = look.border {
        mk.set("BC", Object::Array(components(border)));
    }
    if let Some(background) = look.background {
        mk.set("BG", Object::Array(components(background)));
    }
    if rotation != 0 {
        mk.set("R", rotation);
    }
    if let Some(caption) = caption {
        mk.set("CA", Object::string_literal(caption));
    }
    mk
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_appearance_is_read_and_written() {
        assert_eq!(
            parse_da("/Helv 12 Tf 0 g"),
            (12.0, Color { r: 0, g: 0, b: 0 })
        );
        assert_eq!(
            parse_da("1 0 0 rg /TiRo 0 Tf"),
            (0.0, Color { r: 255, g: 0, b: 0 })
        );
        assert_eq!(parse_da("0 0 0 1 k /Helv 9.5 Tf").0, 9.5);
        assert_eq!(parse_da("n'importe quoi").0, 0.0);
        let look = FieldLook {
            font_size: 10.0,
            ..FieldLook::default()
        };
        assert_eq!(da(&look), "/Helv 10 Tf 0.000 0.000 0.000 rg");
        let (size, _) = parse_da(&da(&look));
        assert_eq!(size, 10.0);
    }
}
