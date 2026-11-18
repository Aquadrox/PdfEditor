//! Dictionnaire de signature écrit à la main, avec deux trous de taille
//! fixe : `/ByteRange` (positions, connues après écriture du fichier) et
//! `/Contents` (le CMS, calculé sur tout le reste du fichier).

use lopdf::{Object, Writer};

use crate::error::Result;

/// Largeur réservée à chaque nombre de `/ByteRange` (fichiers < 10 Go).
const NUMBER_WIDTH: usize = 10;

/// Niveau de verrouillage d'une signature de certification (DocMDP).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lock {
    /// Signature d'approbation : le document reste modifiable.
    None,
    /// Certification `/P 2` : seuls le remplissage du formulaire et
    /// d'autres signatures restent permis.
    FormsAndSignatures,
    /// Certification `/P 1` : plus aucune modification permise.
    Everything,
}

impl Lock {
    /// Valeur `/P` de DocMDP.
    pub fn permission(self) -> Option<i64> {
        match self {
            Lock::None => None,
            Lock::FormsAndSignatures => Some(2),
            Lock::Everything => Some(1),
        }
    }

    pub(crate) fn from_permission(p: i64) -> Lock {
        match p {
            1 => Lock::Everything,
            _ => Lock::FormsAndSignatures,
        }
    }
}

/// Ce que le dictionnaire décrit, en plus du CMS.
pub(crate) struct Details<'a> {
    pub name: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub location: Option<&'a str>,
    pub contact: Option<&'a str>,
    pub date: &'a str,
    pub lock: Lock,
}

/// Corps du dictionnaire et place des trous, relative au début du corps.
pub(crate) struct SigDict {
    pub bytes: Vec<u8>,
    /// Début de la liste `[0 … … …]`.
    pub byte_range_at: usize,
    pub byte_range_len: usize,
    /// Position du `<` de `/Contents`.
    pub contents_at: usize,
    /// Octets de CMS que peut contenir `/Contents`.
    pub capacity: usize,
}

/// `encrypt` chiffre les chaînes d'un document protégé (pas `/Contents`).
pub(crate) fn build(
    details: &Details<'_>,
    capacity: usize,
    encrypt: &dyn Fn(Object) -> Result<Object>,
) -> Result<SigDict> {
    let mut out = Vec::new();
    out.extend_from_slice(
        b"<<\n/Type /Sig\n/Filter /Adobe.PPKLite\n/SubFilter /ETSI.CAdES.detached\n/ByteRange ",
    );
    let byte_range_at = out.len();
    let placeholder = byte_range_text(&[0, 0, 0, 0]);
    out.extend_from_slice(placeholder.as_bytes());
    out.extend_from_slice(b"\n/Contents ");
    let contents_at = out.len();
    out.push(b'<');
    out.resize(out.len() + capacity * 2, b'0');
    out.push(b'>');
    out.extend_from_slice(b"\n/M ");
    Writer::write_object(&mut out, &encrypt(Object::string_literal(details.date))?)?;
    for (key, value) in [
        ("Name", details.name),
        ("Reason", details.reason),
        ("Location", details.location),
        ("ContactInfo", details.contact),
    ] {
        if let Some(value) = value.map(str::trim).filter(|v| !v.is_empty()) {
            out.extend_from_slice(format!("\n/{key} ").as_bytes());
            Writer::write_object(&mut out, &encrypt(lopdf::text_string(value))?)?;
        }
    }
    if let Some(p) = details.lock.permission() {
        out.extend_from_slice(
            format!(
                "\n/Reference [<< /Type /SigRef /TransformMethod /DocMDP \
                 /TransformParams << /Type /TransformParams /P {p} /V /1.2 >> >>]"
            )
            .as_bytes(),
        );
    }
    out.extend_from_slice(b"\n/Prop_Build << /App << /Name /PdfEditor >> >>\n>>");
    Ok(SigDict {
        bytes: out,
        byte_range_at,
        byte_range_len: placeholder.len(),
        contents_at,
        capacity,
    })
}

/// `[0 a b c]`, chaque nombre complété par des espaces à largeur fixe.
pub(crate) fn byte_range_text(values: &[usize; 4]) -> String {
    let numbers: Vec<String> = values
        .iter()
        .map(|v| format!("{v:<NUMBER_WIDTH$}"))
        .collect();
    format!("[{}]", numbers.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_range_keeps_its_width() {
        let empty = byte_range_text(&[0, 0, 0, 0]);
        let filled = byte_range_text(&[0, 1234, 9_876_543, 12]);
        assert_eq!(empty.len(), filled.len());
        assert!(filled.starts_with("[0          1234       9876543    12"));
    }

    #[test]
    fn holes_are_where_announced() {
        let details = Details {
            name: Some("Hélène"),
            reason: Some("  "),
            location: None,
            contact: None,
            date: "D:20261118120000Z",
            lock: Lock::Everything,
        };
        let dict = build(&details, 100, &Ok).unwrap();
        let bytes = &dict.bytes;
        assert_eq!(bytes[dict.byte_range_at], b'[');
        assert_eq!(bytes[dict.byte_range_at + dict.byte_range_len - 1], b']');
        assert_eq!(bytes[dict.contents_at], b'<');
        assert_eq!(bytes[dict.contents_at + 201], b'>');
        let text = String::from_utf8_lossy(bytes);
        assert!(text.contains("/P 1 /V /1.2"));
        // Raison vide : pas d'entrée ; nom accentué : chaîne UTF-16 en hexadécimal.
        assert!(!text.contains("/Reason"));
        assert!(text.contains("/Name <FEFF"));
    }
}
