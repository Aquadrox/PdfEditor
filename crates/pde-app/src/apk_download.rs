//! Mise à jour Android (A-04) : téléchargement vérifié de l'APK annoncé par
//! le manifeste ([`pde_core::apk_update`]), repris de VoiceToText.
//!
//! Le contenu arrive par un simple [`Read`] (la requête HTTPS est faite par
//! l'application) : taille bornée par celle annoncée, empreinte SHA-256
//! calculée au fil de l'eau, fichier écrit à côté (`.part`) puis renommé
//! seulement s'il est conforme. En cas d'échec, rien ne reste sur le disque.

use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use pde_core::apk_update::{ApkRelease, MAX_MANIFEST_BYTES, Manifest};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("fichier plus gros qu'annoncé : refusé")]
    TooBig,
    #[error("téléchargement incomplet : réessayez")]
    Incomplete,
    #[error("empreinte SHA-256 inattendue : fichier refusé")]
    Digest,
    #[error("téléchargement interrompu : {0}")]
    Read(io::Error),
    #[error("écriture impossible dans le dossier de l'application : {0}")]
    Write(io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("manifeste trop gros : ignoré")]
    TooBig,
    #[error("manifeste illisible : {0}")]
    Invalid(String),
}

/// Lit le manifeste (au plus [`MAX_MANIFEST_BYTES`] octets).
pub fn read_manifest(reader: impl Read) -> Result<Manifest, ManifestError> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| ManifestError::Invalid(e.to_string()))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ManifestError::TooBig);
    }
    serde_json::from_slice(&bytes).map_err(|e| ManifestError::Invalid(e.to_string()))
}

/// Copie `reader` dans `dest` en vérifiant taille et empreinte ; `progress`
/// reçoit l'avancement en % à chaque changement.
pub fn download(
    reader: impl Read,
    release: &ApkRelease,
    dest: &Path,
    mut progress: impl FnMut(u8),
) -> Result<(), DownloadError> {
    let part = part_path(dest);
    let result = write_checked(reader, release, &part, &mut progress)
        .and_then(|()| fs::rename(&part, dest).map_err(DownloadError::Write));
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    dest.with_file_name(name)
}

fn write_checked(
    mut reader: impl Read,
    release: &ApkRelease,
    path: &Path,
    progress: &mut impl FnMut(u8),
) -> Result<(), DownloadError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(DownloadError::Write)?;
    }
    let mut output = BufWriter::new(File::create(path).map_err(DownloadError::Write)?);
    let mut digest = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    let mut last = None;
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(DownloadError::Read(e)),
        };
        total += read as u64;
        if total > release.size {
            return Err(DownloadError::TooBig);
        }
        let chunk = &buffer[..read];
        digest.update(chunk);
        output.write_all(chunk).map_err(DownloadError::Write)?;
        let percent = (total * 100 / release.size) as u8;
        if last != Some(percent) {
            last = Some(percent);
            progress(percent);
        }
    }
    if total != release.size {
        return Err(DownloadError::Incomplete);
    }
    let hex: String = digest
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if hex != release.sha256 {
        return Err(DownloadError::Digest);
    }
    let file = output
        .into_inner()
        .map_err(|e| DownloadError::Write(e.into_error()))?;
    file.sync_all().map_err(DownloadError::Write)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(content: &[u8]) -> ApkRelease {
        let sha256 = Sha256::digest(content)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        ApkRelease {
            version: "0.0.30".into(),
            url: "https://github.com/Aquadrox/PdfEditor/releases/download/v0.0.30/a.apk".into(),
            sha256,
            size: content.len() as u64,
            changelog: String::new(),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pde-apk-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn content() -> Vec<u8> {
        (0..200_000u32).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn conforming_file_is_kept_with_progress() {
        let dir = temp_dir("ok");
        let dest = dir.join("update").join("PdfEditor.apk");
        let data = content();
        let mut seen = Vec::new();
        download(data.as_slice(), &release(&data), &dest, |p| seen.push(p)).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), data);
        assert!(!part_path(&dest).exists());
        assert_eq!(seen.last(), Some(&100));
        assert!(seen.windows(2).all(|w| w[0] < w[1]), "{seen:?}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn wrong_digest_leaves_nothing() {
        let dir = temp_dir("digest");
        let dest = dir.join("PdfEditor.apk");
        let data = content();
        let mut other = data.clone();
        other[10] ^= 1;
        let error = download(other.as_slice(), &release(&data), &dest, |_| {}).unwrap_err();
        assert!(matches!(error, DownloadError::Digest), "{error}");
        assert!(!dest.exists() && !part_path(&dest).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn larger_than_announced_is_refused_early() {
        let dir = temp_dir("big");
        let dest = dir.join("PdfEditor.apk");
        let data = content();
        let mut announced = release(&data[..1000]);
        announced.size = 1000;
        // Flux sans fin : la lecture doit s'arrêter dès la taille dépassée.
        let endless = io::repeat(7);
        let error = download(endless, &announced, &dest, |_| {}).unwrap_err();
        assert!(matches!(error, DownloadError::TooBig), "{error}");
        assert!(!dest.exists() && !part_path(&dest).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn truncated_download_is_refused() {
        let dir = temp_dir("short");
        let dest = dir.join("PdfEditor.apk");
        let data = content();
        let error = download(&data[..5000], &release(&data), &dest, |_| {}).unwrap_err();
        assert!(matches!(error, DownloadError::Incomplete), "{error}");
        assert!(!dest.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn read_error_is_reported() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::ConnectionReset, "coupé"))
            }
        }
        let dir = temp_dir("broken");
        let dest = dir.join("PdfEditor.apk");
        let error = download(Broken, &release(b"abc"), &dest, |_| {}).unwrap_err();
        assert!(error.to_string().contains("coupé"), "{error}");
        assert!(!part_path(&dest).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn replaces_a_previous_file() {
        let dir = temp_dir("replace");
        let dest = dir.join("PdfEditor.apk");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&dest, b"ancien").unwrap();
        download(&b"nouveau"[..], &release(b"nouveau"), &dest, |_| {}).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"nouveau");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn manifest_is_read_and_bounded() {
        let json = br#"{"version":"0.0.30","platforms":{}}"#;
        assert_eq!(read_manifest(&json[..]).unwrap().version, "0.0.30");
        let huge = vec![b' '; MAX_MANIFEST_BYTES as usize + 1];
        assert!(matches!(
            read_manifest(huge.as_slice()),
            Err(ManifestError::TooBig)
        ));
        assert!(matches!(
            read_manifest(&b"<html>"[..]),
            Err(ManifestError::Invalid(_))
        ));
    }
}
