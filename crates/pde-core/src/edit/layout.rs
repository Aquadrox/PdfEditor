//! Mise en lignes d'une zone de texte. La mesure du texte vient du moteur
//! (il connaît les polices) : ici, seulement la règle de coupure, testable.

use super::element::TextStyle;

/// Distance entre deux lignes de base (points).
pub fn leading(style: &TextStyle) -> f64 {
    style.size * style.line_height
}

/// Coupe `text` en lignes de largeur au plus `max_width` selon `measure`
/// (largeur d'une chaîne, en points). Les retours à la ligne du texte sont
/// gardés ; on coupe entre les mots, et à l'intérieur d'un mot seulement
/// s'il est plus long que la ligne entière.
pub fn wrap(text: &str, max_width: f64, measure: impl Fn(&str) -> f64) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let paragraph = paragraph.trim_end_matches('\r');
        let mut line = String::new();
        for word in paragraph.split(' ') {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if measure(&candidate) <= max_width {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // Mot plus long que la ligne : coupé lettre à lettre.
            for ch in word.chars() {
                let mut longer = line.clone();
                longer.push(ch);
                if !line.is_empty() && measure(&longer) > max_width {
                    lines.push(std::mem::take(&mut line));
                    line.push(ch);
                } else {
                    line = longer;
                }
            }
        }
        lines.push(line.trim_end().to_owned());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chaque caractère mesure 10 pt.
    fn measure(s: &str) -> f64 {
        10.0 * s.chars().count() as f64
    }

    #[test]
    fn short_text_stays_on_one_line() {
        assert_eq!(wrap("Bonjour", 100.0, measure), ["Bonjour"]);
    }

    #[test]
    fn words_wrap_at_the_width() {
        // 100 pt = 10 caractères par ligne.
        assert_eq!(
            wrap("le prix est fixe a vingt euros", 100.0, measure),
            ["le prix", "est fixe a", "vingt", "euros"]
        );
    }

    #[test]
    fn line_breaks_are_kept_and_empty_lines_too() {
        assert_eq!(
            wrap("un\n\ndeux\r\ntrois", 100.0, measure),
            ["un", "", "deux", "trois"]
        );
    }

    #[test]
    fn long_word_is_cut() {
        assert_eq!(
            wrap("anticonstitutionnellement", 100.0, measure),
            ["anticonsti", "tutionnell", "ement"]
        );
    }

    #[test]
    fn empty_text_is_one_empty_line() {
        assert_eq!(wrap("", 100.0, measure), [""]);
    }

    #[test]
    fn leading_from_style() {
        let style = TextStyle {
            size: 10.0,
            line_height: 1.5,
            ..TextStyle::default()
        };
        assert_eq!(leading(&style), 15.0);
    }
}
