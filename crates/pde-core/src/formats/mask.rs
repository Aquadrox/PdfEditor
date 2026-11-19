//! Masque de saisie (`AFSpecial_KeystrokeEx` d'Acrobat) : `9` un chiffre,
//! `A` une lettre, `O` une lettre ou un chiffre, `X` tout caractère ; les
//! autres caractères du masque s'écrivent tels quels. La saisie peut les
//! omettre ou en mettre d'autres : « 0612345678 » et « 06.12.34.56.78 »
//! donnent « 06 12 34 56 78 » avec le masque « 99 99 99 99 99 ».

pub(super) fn is_placeholder(c: char) -> bool {
    matches!(c, '9' | 'A' | 'O' | 'X')
}

fn accepts(placeholder: char, c: char) -> bool {
    match placeholder {
        '9' => c.is_ascii_digit(),
        'A' => c.is_alphabetic(),
        'O' => c.is_alphanumeric(),
        _ => !c.is_control(),
    }
}

pub(super) fn apply(mask: &str, input: &str) -> Result<String, String> {
    let wrong = || format!("saisie attendue : {}", sample(mask));
    // Seuls lettres et chiffres comptent (sauf pour « X », qui prend tout).
    let any = mask.contains('X');
    let mut chars = input
        .chars()
        .filter(|c| {
            if any {
                !c.is_whitespace()
            } else {
                c.is_alphanumeric()
            }
        })
        .peekable();
    let mut out = String::new();
    for m in mask.chars() {
        if is_placeholder(m) {
            let c = chars.next().ok_or_else(wrong)?;
            if !accepts(m, c) {
                return Err(wrong());
            }
            out.push(c);
        } else {
            out.push(m);
            // La saisie a pu reprendre le caractère du masque.
            if any && chars.peek() == Some(&m) {
                chars.next();
            }
        }
    }
    if chars.next().is_some() {
        return Err(wrong());
    }
    Ok(out)
}

/// Exemple lisible : « 99 99 99 99 99 » → « 01 23 45 67 89 ».
pub(super) fn sample(mask: &str) -> String {
    let mut digit = 0u32;
    mask.chars()
        .map(|m| match m {
            '9' => {
                let c = char::from_digit(digit % 10, 10).unwrap_or('0');
                digit += 1;
                c
            }
            'A' => 'A',
            'O' | 'X' => 'x',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_is_fitted_to_the_mask() {
        let phone = "99 99 99 99 99";
        assert_eq!(apply(phone, "0612345678").unwrap(), "06 12 34 56 78");
        assert_eq!(apply(phone, "06.12.34.56.78").unwrap(), "06 12 34 56 78");
        assert!(
            apply(phone, "061234567")
                .unwrap_err()
                .contains("01 23 45 67 89")
        );
        assert!(apply(phone, "06123456789").is_err());
        assert!(apply(phone, "06 12 34 56 7a").is_err());
        assert_eq!(apply("99999", "75001").unwrap(), "75001");
        assert_eq!(apply("AA99", "fr76").unwrap(), "fr76");
        assert_eq!(apply("X-X", "a-b").unwrap(), "a-b");
        assert_eq!(sample("AA99 OX"), "AA01 xx");
    }
}
