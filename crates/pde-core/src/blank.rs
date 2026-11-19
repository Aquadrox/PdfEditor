//! PDF vierge (E-13) : quelques pages blanches d'une taille donnée, écrites
//! octet par octet (table de références croisées classique). Point de
//! départ d'un nouveau document et de l'import intégré d'un document Word.

use crate::document::PageSize;

/// Pages au plus d'un PDF vierge.
pub const MAX_BLANK_PAGES: u32 = 1000;

/// PDF de `count` pages blanches de taille `size`.
pub fn blank_pdf(size: PageSize, count: u32) -> Vec<u8> {
    let count = count.clamp(1, MAX_BLANK_PAGES) as usize;
    let first_page = 4;
    let kids: Vec<String> = (0..count)
        .map(|i| format!("{} 0 R", first_page + i))
        .collect();
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {count} >>",
            kids.join(" ")
        ),
        "<< /Producer (PdfEditor) >>".to_string(),
    ];
    for _ in 0..count {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.4} {:.4}] /Resources << >> >>",
            size.width, size.height
        ));
    }
    let mut out = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 3 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_pdf_has_its_pages_and_a_valid_cross_reference_table() {
        let bytes = blank_pdf(PageSize::new(100.0, 200.0), 3);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("%PDF-1.7"));
        assert!(text.contains("/Count 3"));
        assert_eq!(text.matches("/Type /Page ").count(), 3);
        // Chaque entrée de la table désigne bien son objet.
        let xref = text.rfind("xref\n").unwrap();
        for (i, line) in text[xref..].lines().skip(3).take(6).enumerate() {
            let offset: usize = line[..10].parse().unwrap();
            assert!(
                text[offset..].starts_with(&format!("{} 0 obj", i + 1)),
                "{line}"
            );
        }
        assert!(blank_pdf(PageSize::new(10.0, 10.0), 0).len() > 100);
    }
}
