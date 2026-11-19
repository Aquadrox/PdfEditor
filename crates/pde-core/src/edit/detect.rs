//! Détection des champs d'un formulaire « papier » (E-12) : là où un
//! document imprimé attend une écriture — traits `_____`, pointillés
//! `.....`, cases `☐`, libellé terminé par « : » en fin de ligne — un champ
//! est proposé, de genre deviné d'après son libellé (date, e-mail,
//! téléphone, montant, signature…). Rien n'est ajouté sans accord.

use crate::formats::{FieldFormat, Separator};
use crate::geometry::{Point, Rect};
use crate::text::{PageText, TextChar, same_line};

use super::form::FieldKind;

/// Champ proposé sur une page (page affichée, origine en bas).
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub rect: Rect,
    pub kind: FieldKind,
    /// Libellé lu à côté (« Date de naissance »), vide s'il n'y en a pas.
    pub label: String,
}

const BOXES: &[char] = &['☐', '□', '❑', '❒', '◻', '▢', '⬜', '\u{f06f}', '\u{f0a8}'];
/// Hauteur d'un champ proposé (points), bornée.
const MIN_HEIGHT: f64 = 14.0;
const MAX_HEIGHT: f64 = 22.0;

fn fold(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' => 'e',
            'à' | 'â' | 'À' => 'a',
            'î' | 'ï' => 'i',
            'ô' => 'o',
            'û' | 'ù' => 'u',
            'ç' => 'c',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

fn has_word(label: &str, words: &[&str]) -> bool {
    let folded = fold(label);
    let tokens: Vec<&str> = folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    words.iter().any(|w| {
        if w.contains(' ') {
            folded.contains(w)
        } else {
            tokens.contains(w)
        }
    })
}

fn text_kind(format: Option<FieldFormat>, multiline: bool) -> FieldKind {
    FieldKind::Text {
        multiline,
        max_len: None,
        password: false,
        comb: false,
        format,
        calc: None,
    }
}

/// Genre deviné d'après le libellé.
pub fn kind_for_label(label: &str, multiline: bool) -> FieldKind {
    let number = |decimals, currency: &str| FieldFormat::Number {
        decimals,
        separator: Separator::Comma,
        currency: currency.into(),
        currency_before: false,
    };
    let mask = |mask: &str| FieldFormat::Mask { mask: mask.into() };
    let format = if has_word(label, &["signature", "signe", "signataire", "visa"]) {
        return FieldKind::Signature;
    } else if has_word(
        label,
        &[
            "date",
            "ne le",
            "nee le",
            "fait le",
            "le",
            "naissance",
            "echeance",
        ],
    ) {
        Some(FieldFormat::Date {
            pattern: "dd/mm/yyyy".into(),
        })
    } else if has_word(label, &["heure", "horaire"]) {
        Some(FieldFormat::Time {
            pattern: "HH:MM".into(),
        })
    } else if has_word(label, &["email", "e mail", "mail", "courriel"]) {
        Some(FieldFormat::Email)
    } else if has_word(
        label,
        &["tel", "telephone", "portable", "mobile", "fax", "phone"],
    ) {
        Some(mask("99 99 99 99 99"))
    } else if has_word(label, &["code postal", "cp"]) {
        Some(mask("99999"))
    } else if has_word(label, &["siret"]) {
        Some(mask("999 999 999 99999"))
    } else if has_word(label, &["siren"]) {
        Some(mask("999 999 999"))
    } else if has_word(
        label,
        &[
            "montant", "prix", "total", "somme", "tarif", "cout", "ht", "ttc", "eur", "euros",
        ],
    ) || label.contains('€')
    {
        Some(number(2, " €"))
    } else if has_word(label, &["taux", "pourcentage"]) || label.contains('%') {
        Some(FieldFormat::Percent {
            decimals: 0,
            separator: Separator::Comma,
        })
    } else if has_word(label, &["nombre", "quantite", "qte", "nb", "age"]) {
        Some(number(0, ""))
    } else {
        None
    };
    text_kind(format, multiline)
}

/// Nom de champ tiré d'un libellé (« Date de naissance » → `date_de_naissance`).
pub fn name_for_label(label: &str) -> String {
    let mut name = String::new();
    for c in fold(label).chars() {
        if c.is_ascii_alphanumeric() {
            name.push(c);
        } else if !name.ends_with('_') && !name.is_empty() {
            name.push('_');
        }
        if name.len() >= 30 {
            break;
        }
    }
    let name = name.trim_matches('_').to_string();
    if name.is_empty() {
        "champ".into()
    } else {
        name
    }
}

/// Lignes de la page : caractères qui partagent une même ligne.
fn lines(text: &PageText) -> Vec<Vec<&TextChar>> {
    let mut lines: Vec<Vec<&TextChar>> = Vec::new();
    for c in &text.chars {
        if c.ch == '\n' || c.ch == '\r' {
            lines.push(Vec::new());
            continue;
        }
        let Some(line) = lines.last_mut() else {
            lines.push(vec![c]);
            continue;
        };
        let last = line.iter().rev().find(|c| !c.bounds.is_empty());
        match last {
            Some(last) if !c.bounds.is_empty() && !same_line(&last.bounds, &c.bounds) => {
                lines.push(vec![c]);
            }
            _ => line.push(c),
        }
    }
    lines.retain(|l| l.iter().any(|c| !c.bounds.is_empty()));
    lines
}

fn label_text(chars: &[&TextChar]) -> String {
    let text: String = chars.iter().map(|c| c.ch).collect();
    let cleaned: String = text
        .chars()
        .filter(|c| !matches!(c, '_' | '…') && !BOXES.contains(c))
        .collect();
    let trimmed = cleaned.trim().trim_end_matches(['.', ':', ' ']).trim();
    // Dernier libellé avant le blanc : après la dernière ponctuation forte.
    let last = trimmed
        .rsplit(['\t', ';', '|'])
        .next()
        .unwrap_or(trimmed)
        .trim();
    last.chars().take(60).collect()
}

fn line_height(chars: &[&TextChar]) -> f64 {
    chars
        .iter()
        .filter(|c| !c.bounds.is_empty() && c.ch.is_alphanumeric())
        .map(|c| c.bounds.height())
        .fold(0.0, f64::max)
}

/// Rectangle d'un champ posé sur une ligne d'écriture de `x0` à `x1`,
/// dont le bas est `bottom`.
fn field_rect(x0: f64, x1: f64, bottom: f64, height: f64) -> Rect {
    Rect::from_corners(Point::new(x0, bottom), Point::new(x1, bottom + height))
}

/// Blancs d'une ligne : suites de `_`, de points ou de `…`.
fn blanks(line: &[&TextChar]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < line.len() {
        let is_blank = |c: char| matches!(c, '_' | '.' | '…');
        if !is_blank(line[i].ch) {
            i += 1;
            continue;
        }
        let start = i;
        while i < line.len()
            && (is_blank(line[i].ch)
                || line[i].ch == ' ' && i + 1 < line.len() && is_blank(line[i + 1].ch))
        {
            i += 1;
        }
        let run = &line[start..i];
        let underscores = run.iter().filter(|c| c.ch == '_').count();
        let dots = run.iter().map(|c| match c.ch {
            '.' => 1,
            '…' => 3,
            _ => 0,
        });
        if underscores >= 3 || dots.sum::<usize>() >= 5 {
            out.push((start, i));
        }
    }
    out
}

/// Champs proposés sur une page de largeur affichée `page_width`.
pub fn detect_fields(text: &PageText, page_width: f64) -> Vec<Suggestion> {
    let mut found: Vec<Suggestion> = Vec::new();
    for line in lines(text) {
        let height = (line_height(&line) * 1.4).clamp(MIN_HEIGHT, MAX_HEIGHT);
        let mut consumed = 0;
        for (start, end) in blanks(&line) {
            let run: Vec<&&TextChar> = line[start..end]
                .iter()
                .filter(|c| !c.bounds.is_empty())
                .collect();
            let (Some(first), Some(last)) = (run.first(), run.last()) else {
                continue;
            };
            let mut label = label_text(&line[consumed..start]);
            if label.is_empty() {
                label = label_text(&line[end..]);
            }
            let bottom = run
                .iter()
                .map(|c| c.bounds.min().y)
                .fold(f64::INFINITY, f64::min);
            let kind = kind_for_label(&label, false);
            let height = if kind == FieldKind::Signature {
                height.max(36.0)
            } else {
                height
            };
            found.push(Suggestion {
                rect: field_rect(first.bounds.min().x, last.bounds.max().x, bottom, height),
                kind,
                label,
            });
            consumed = end;
        }
        for (i, c) in line.iter().enumerate() {
            if !BOXES.contains(&c.ch) || c.bounds.is_empty() {
                continue;
            }
            let side = c.bounds.width().max(c.bounds.height()).max(8.0);
            let next = line[i + 1..]
                .iter()
                .position(|c| BOXES.contains(&c.ch))
                .map_or(line.len(), |p| i + 1 + p);
            let center = Point::new(
                (c.bounds.min().x + c.bounds.max().x) / 2.0,
                (c.bounds.min().y + c.bounds.max().y) / 2.0,
            );
            found.push(Suggestion {
                rect: Rect::from_corners(
                    Point::new(center.x - side / 2.0, center.y - side / 2.0),
                    Point::new(center.x + side / 2.0, center.y + side / 2.0),
                ),
                kind: FieldKind::Checkbox,
                label: label_text(&line[i + 1..next]),
            });
        }
        // « Libellé : » seul en fin de ligne, avec la place d'écrire après.
        let visible: Vec<&&TextChar> = line.iter().filter(|c| !c.bounds.is_empty()).collect();
        if let Some(last) = visible.last()
            && last.ch == ':'
            && consumed == 0
            && !line.iter().any(|c| BOXES.contains(&c.ch))
            && last.bounds.max().x < page_width * 0.6
        {
            let label = label_text(&line);
            if !label.is_empty() && label.chars().count() <= 40 {
                let x0 = last.bounds.max().x + 4.0;
                let x1 = (x0 + 220.0).min(page_width - 36.0);
                let kind = kind_for_label(&label, false);
                let height = if kind == FieldKind::Signature {
                    height.max(36.0)
                } else {
                    height
                };
                if x1 - x0 >= 40.0 {
                    found.push(Suggestion {
                        rect: field_rect(x0, x1, last.bounds.min().y - 3.0, height),
                        kind,
                        label,
                    });
                }
            }
        }
    }
    merge_paragraphs(found)
}

/// Lignes d'écriture empilées sans libellé sous un champ texte de même
/// largeur : une seule zone de plusieurs lignes.
fn merge_paragraphs(found: Vec<Suggestion>) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = Vec::new();
    for s in found {
        if let Some(prev) = out.last_mut()
            && s.label.is_empty()
            && matches!(prev.kind, FieldKind::Text { format: None, .. })
            && matches!(s.kind, FieldKind::Text { format: None, .. })
            && (prev.rect.min().x - s.rect.min().x).abs() < 30.0
            && (prev.rect.max().x - s.rect.max().x).abs() < 30.0
            && prev.rect.min().y - s.rect.max().y < 2.0 * MAX_HEIGHT
            && prev.rect.min().y > s.rect.min().y
        {
            prev.rect = prev.rect.union(&s.rect);
            prev.kind = text_kind(None, true);
            continue;
        }
        out.push(s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une ligne de texte à la hauteur `y`, caractères de 6 pt de large.
    fn line(text: &str, x: f64, y: f64, out: &mut Vec<TextChar>) {
        for (i, ch) in text.chars().enumerate() {
            let x0 = x + 6.0 * i as f64;
            let bounds = if ch == ' ' {
                Rect::from_corners(Point::new(x0, y), Point::new(x0 + 3.0, y + 10.0))
            } else {
                Rect::from_corners(Point::new(x0, y), Point::new(x0 + 6.0, y + 10.0))
            };
            out.push(TextChar { ch, bounds });
        }
        out.push(TextChar {
            ch: '\n',
            bounds: Rect::default(),
        });
    }

    fn page(lines: &[(&str, f64)]) -> PageText {
        let mut chars = Vec::new();
        for (text, y) in lines {
            line(text, 72.0, *y, &mut chars);
        }
        PageText { chars }
    }

    #[test]
    fn blanks_boxes_and_labels_become_fields() {
        let text = page(&[
            ("Nom : ____________", 700.0),
            ("Date de naissance : ..........", 680.0),
            ("E-mail :", 660.0),
            ("☐ J'accepte ☐ Je refuse", 640.0),
            ("Signature : ______________", 600.0),
            ("Montant TTC : ________ et téléphone ________", 580.0),
        ]);
        let found = detect_fields(&text, 595.0);
        let summary: Vec<(String, &str)> = found
            .iter()
            .map(|s| {
                let kind = match &s.kind {
                    FieldKind::Text { format: None, .. } => "texte",
                    FieldKind::Text {
                        format: Some(FieldFormat::Date { .. }),
                        ..
                    } => "date",
                    FieldKind::Text {
                        format: Some(FieldFormat::Email),
                        ..
                    } => "email",
                    FieldKind::Text {
                        format: Some(FieldFormat::Number { .. }),
                        ..
                    } => "montant",
                    FieldKind::Text {
                        format: Some(FieldFormat::Mask { .. }),
                        ..
                    } => "masque",
                    FieldKind::Checkbox => "case",
                    FieldKind::Signature => "signature",
                    _ => "autre",
                };
                (s.label.clone(), kind)
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                ("Nom".into(), "texte"),
                ("Date de naissance".into(), "date"),
                ("E-mail".into(), "email"),
                ("J'accepte".into(), "case"),
                ("Je refuse".into(), "case"),
                ("Signature".into(), "signature"),
                ("Montant TTC".into(), "montant"),
                ("et téléphone".into(), "masque"),
            ]
        );
        // Le champ couvre le trait, posé dessus.
        let name = found[0].rect;
        assert_eq!(name.min(), Point::new(72.0 + 6.0 * 6.0, 700.0));
        assert_eq!(name.max().x, 72.0 + 6.0 * 18.0);
        assert!(found[5].rect.height() >= 36.0);
    }

    #[test]
    fn stacked_blank_lines_make_one_paragraph() {
        let text = page(&[
            ("Observations : ____________", 700.0),
            ("_________________________", 684.0),
            ("_________________________", 668.0),
        ]);
        let found = detect_fields(&text, 595.0);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(matches!(
            found[1].kind,
            FieldKind::Text {
                multiline: true,
                ..
            }
        ));
    }

    #[test]
    fn names_come_from_labels() {
        assert_eq!(name_for_label("Date de naissance"), "date_de_naissance");
        assert_eq!(name_for_label("  N° de téléphone : "), "n_de_telephone");
        assert_eq!(name_for_label("…"), "champ");
        assert_eq!(
            kind_for_label("Code postal", false),
            text_kind(
                Some(FieldFormat::Mask {
                    mask: "99999".into()
                }),
                false
            )
        );
        assert_eq!(kind_for_label("Lieu", true), text_kind(None, true));
    }
}
