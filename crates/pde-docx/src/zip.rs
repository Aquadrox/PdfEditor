//! Lecture d'une entrée d'une archive ZIP (un .docx en est une) : table
//! centrale, entrée stockée ou compressée (deflate). Tailles bornées : un
//! fichier piégé ne peut pas épuiser la mémoire.

/// Taille décompressée maximale d'une entrée.
pub const MAX_ENTRY_BYTES: usize = 64 * 1024 * 1024;

fn u16_at(bytes: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?) as usize)
}

fn u32_at(bytes: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?) as usize)
}

/// Contenu de l'entrée `name`, ou `None` si elle manque.
pub fn entry(archive: &[u8], name: &str) -> Result<Option<Vec<u8>>, String> {
    let corrupt = || "archive illisible (fichier .docx abîmé ?)".to_string();
    // Fin de la table centrale : signature 0x06054b50, cherchée depuis la fin.
    let end = (0..archive.len().saturating_sub(21))
        .rev()
        .take(70_000)
        .find(|&i| archive[i..].starts_with(&[0x50, 0x4b, 0x05, 0x06]))
        .ok_or_else(corrupt)?;
    let count = u16_at(archive, end + 10).ok_or_else(corrupt)?;
    let mut at = u32_at(archive, end + 16).ok_or_else(corrupt)?;
    for _ in 0..count {
        if !archive
            .get(at..)
            .is_some_and(|b| b.starts_with(&[0x50, 0x4b, 0x01, 0x02]))
        {
            return Err(corrupt());
        }
        let method = u16_at(archive, at + 10).ok_or_else(corrupt)?;
        let compressed = u32_at(archive, at + 20).ok_or_else(corrupt)?;
        let size = u32_at(archive, at + 24).ok_or_else(corrupt)?;
        let name_len = u16_at(archive, at + 28).ok_or_else(corrupt)?;
        let extra_len = u16_at(archive, at + 30).ok_or_else(corrupt)?;
        let comment_len = u16_at(archive, at + 32).ok_or_else(corrupt)?;
        let local = u32_at(archive, at + 42).ok_or_else(corrupt)?;
        let entry_name = archive
            .get(at + 46..at + 46 + name_len)
            .ok_or_else(corrupt)?;
        at += 46 + name_len + extra_len + comment_len;
        if entry_name != name.as_bytes() {
            continue;
        }
        if size > MAX_ENTRY_BYTES {
            return Err("document trop volumineux".into());
        }
        let local_name = u16_at(archive, local + 26).ok_or_else(corrupt)?;
        let local_extra = u16_at(archive, local + 28).ok_or_else(corrupt)?;
        let start = local + 30 + local_name + local_extra;
        let data = archive.get(start..start + compressed).ok_or_else(corrupt)?;
        return match method {
            0 => Ok(Some(data.to_vec())),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(data, MAX_ENTRY_BYTES)
                .map(Some)
                .map_err(|_| corrupt()),
            _ => Err("compression de l'archive non prise en charge".into()),
        };
    }
    Ok(None)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Archive ZIP minimale (entrées compressées en deflate), pour les tests.
    pub(crate) fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, content) in files {
            let data = miniz_oxide::deflate::compress_to_vec(content, 6);
            let offset = out.len() as u32;
            let header = |sig: &[u8], central: bool| {
                let mut h = sig.to_vec();
                if central {
                    h.extend_from_slice(&[20, 0]);
                }
                h.extend_from_slice(&[20, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
                h.extend_from_slice(&(data.len() as u32).to_le_bytes());
                h.extend_from_slice(&(content.len() as u32).to_le_bytes());
                h.extend_from_slice(&(name.len() as u16).to_le_bytes());
                h.extend_from_slice(&[0, 0]);
                if central {
                    // Commentaire, disque, attributs internes et externes.
                    h.extend_from_slice(&[0; 10]);
                    h.extend_from_slice(&offset.to_le_bytes());
                }
                h.extend_from_slice(name.as_bytes());
                h
            };
            out.extend(header(&[0x50, 0x4b, 0x03, 0x04], false));
            out.extend(&data);
            central.extend(header(&[0x50, 0x4b, 0x01, 0x02], true));
        }
        let start = out.len() as u32;
        out.extend(&central);
        out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(central.len() as u32).to_le_bytes());
        out.extend_from_slice(&start.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    #[test]
    fn entries_are_found_and_inflated() {
        let zip = archive(&[
            ("a.txt", b"bonjour"),
            ("word/document.xml", b"<w:document/>"),
        ]);
        assert_eq!(
            entry(&zip, "word/document.xml").unwrap().unwrap(),
            b"<w:document/>"
        );
        assert_eq!(entry(&zip, "absent").unwrap(), None);
        assert!(entry(b"pas une archive", "a").is_err());
        let mut broken = zip.clone();
        let len = broken.len();
        broken[len - 6] = 0xff;
        assert!(entry(&broken, "a.txt").is_err());
    }
}
