//! Recherche dans le texte d'une page, insensible à la casse, aux accents,
//! aux ligatures (ﬁ) et à la forme des espaces (fin de ligne = espace).

use std::ops::Range;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::geometry::Rect;
use crate::text::{PageText, same_line};

/// Texte replié pour la comparaison, avec pour chaque caractère replié
/// l'indice du caractère d'origine.
struct Folded {
    chars: Vec<char>,
    origin: Vec<usize>,
}

fn fold(chars: impl Iterator<Item = char>) -> Folded {
    let mut folded = Folded {
        chars: Vec::new(),
        origin: Vec::new(),
    };
    for (index, ch) in chars.enumerate() {
        if ch.is_whitespace() {
            // Une suite d'espaces ou de fins de ligne compte pour un espace.
            if folded.chars.last().is_some_and(|c| *c != ' ') {
                folded.chars.push(' ');
                folded.origin.push(index);
            }
            continue;
        }
        for c in ch
            .nfkd()
            .filter(|c| !is_combining_mark(*c))
            .flat_map(char::to_lowercase)
        {
            folded.chars.push(c);
            folded.origin.push(index);
        }
    }
    folded
}

/// Occurrences de `query` dans la page, en indices de caractères
/// (`page.chars[range]`). Requête vide ou blanche : aucune occurrence.
pub fn find(page: &PageText, query: &str) -> Vec<Range<usize>> {
    let needle = fold(query.trim().chars()).chars;
    if needle.is_empty() {
        return Vec::new();
    }
    let haystack = fold(page.chars.iter().map(|c| c.ch));
    let mut matches = Vec::new();
    let mut start = 0;
    while start + needle.len() <= haystack.chars.len() {
        if haystack.chars[start..start + needle.len()] == needle[..] {
            let first = haystack.origin[start];
            let last = haystack.origin[start + needle.len() - 1];
            matches.push(first..last + 1);
            start += needle.len();
        } else {
            start += 1;
        }
    }
    matches
}

/// Rectangles à surligner pour une occurrence : un par ligne couverte.
pub fn match_rects(page: &PageText, range: Range<usize>) -> Vec<Rect> {
    let mut rects: Vec<Rect> = Vec::new();
    for c in page.chars.get(range).unwrap_or_default() {
        if c.bounds.is_empty() {
            continue;
        }
        match rects.last_mut() {
            Some(last) if same_line(last, &c.bounds) => *last = last.union(&c.bounds),
            _ => rects.push(c.bounds),
        }
    }
    rects
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::text::TextChar;

    /// Page dont chaque ligne de `lines` est posée 20 pt sous la précédente,
    /// avec une fin de ligne déduite entre deux lignes.
    fn page(lines: &[&str]) -> PageText {
        let mut chars = Vec::new();
        for (row, line) in lines.iter().enumerate() {
            if row > 0 {
                chars.push(TextChar {
                    ch: '\n',
                    bounds: Rect::default(),
                });
            }
            let y = 700.0 - 20.0 * row as f64;
            for (i, ch) in line.chars().enumerate() {
                let x = 5.0 * i as f64;
                chars.push(TextChar {
                    ch,
                    bounds: Rect::from_corners(Point::new(x, y), Point::new(x + 5.0, y + 10.0)),
                });
            }
        }
        PageText { chars }
    }

    fn found(page: &PageText, query: &str) -> Vec<String> {
        find(page, query)
            .into_iter()
            .map(|r| page.chars[r].iter().map(|c| c.ch).collect())
            .collect()
    }

    #[test]
    fn case_and_accents_are_ignored() {
        let p = page(&["Élève ÉLÈVE eleve"]);
        assert_eq!(found(&p, "élève"), ["Élève", "ÉLÈVE", "eleve"]);
        assert_eq!(found(&p, "ELEVE").len(), 3);
    }

    #[test]
    fn ligatures_match_their_letters() {
        let p = page(&["la ﬁn du contrat"]);
        assert_eq!(found(&p, "fin"), ["ﬁn"]);
    }

    #[test]
    fn query_can_span_a_line_break() {
        let p = page(&["prix forfaitaire", "de la prestation"]);
        assert_eq!(found(&p, "forfaitaire de"), ["forfaitaire\nde"]);
    }

    #[test]
    fn spaces_in_query_are_flexible() {
        let p = page(&["trente  jours"]);
        assert_eq!(found(&p, " trente jours "), ["trente  jours"]);
    }

    #[test]
    fn empty_query_or_no_match() {
        let p = page(&["texte"]);
        assert!(find(&p, "").is_empty());
        assert!(find(&p, "   ").is_empty());
        assert!(find(&p, "absent").is_empty());
    }

    #[test]
    fn matches_do_not_overlap() {
        assert_eq!(found(&page(&["aaaa"]), "aa").len(), 2);
    }

    #[test]
    fn one_rectangle_per_line() {
        let p = page(&["prix forfaitaire", "de la prestation"]);
        let range = find(&p, "forfaitaire de").remove(0);
        let rects = match_rects(&p, range);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].min(), Point::new(25.0, 700.0));
        assert_eq!(rects[0].max(), Point::new(80.0, 710.0));
        assert_eq!(rects[1].max(), Point::new(10.0, 690.0));
    }
}
