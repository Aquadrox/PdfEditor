//! Import intégré des documents Word (E-13), quand ni Microsoft Word ni
//! LibreOffice ne sont installés : le texte, les titres, les listes et les
//! tableaux d'un `.docx` sont repris dans une mise en page simplifiée
//! (polices standard, images ignorées et signalées). Rust pur, sans
//! dépendance au système : l'archive est lue par `zip.rs`, le XML par
//! `xml.rs`, le contenu par `parse.rs`, la mise en page par `layout.rs`.

mod layout;
mod parse;
mod xml;
mod zip;

pub use layout::{Layout, layout};
pub use parse::{Block, Docx, PageSetup, Paragraph, parse};

/// Plus gros fichier .docx accepté.
pub const MAX_DOCX_BYTES: usize = 128 * 1024 * 1024;

/// Contenu d'un fichier .docx.
pub fn read(bytes: &[u8]) -> Result<Docx, String> {
    if bytes.len() > MAX_DOCX_BYTES {
        return Err("document trop volumineux".into());
    }
    if !bytes.starts_with(b"PK") {
        return Err("ce fichier n'est pas un document Word (.docx)".into());
    }
    let xml = zip::entry(bytes, "word/document.xml")?
        .ok_or("ce fichier n'est pas un document Word (.docx)")?;
    let xml =
        String::from_utf8(xml).map_err(|_| "document Word illisible (encodage)".to_string())?;
    Ok(parse(&xml))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_docx_archive_is_read_and_laid_out() {
        let xml = br#"<w:document><w:body><w:p><w:r><w:t>Nom : ______</w:t></w:r></w:p></w:body></w:document>"#;
        let archive = zip::tests::archive(&[
            ("[Content_Types].xml", b"<Types/>"),
            ("word/document.xml", xml),
        ]);
        let doc = read(&archive).unwrap();
        assert_eq!(doc.blocks.len(), 1);
        let out = layout(&doc);
        assert_eq!((out.pages, out.elements.len()), (1, 1));
        assert!(read(b"%PDF-1.7").is_err());
        let other = zip::tests::archive(&[("content.xml", b"<x/>")]);
        assert!(read(&other).unwrap_err().contains(".docx"));
    }
}
