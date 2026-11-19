//! Mises à jour de l'application Android (A-04), reprises de VoiceToText :
//! lecture du manifeste `latest-android.json` publié par la CI à chaque tag.
//!
//! ```json
//! {
//!   "version": "0.0.30",
//!   "notes": "https://github.com/…/releases/tag/v0.0.30",
//!   "changelog": "## 0.0.30 — 2026-11-19\n\n- …",
//!   "pub_date": "2026-11-19T10:00:00Z",
//!   "platforms": {
//!     "android-aarch64": { "url": "…/v0.0.30/PdfEditor_0.0.30_android.apk",
//!                          "sha256": "…", "size": 41234567 }
//!   }
//! }
//! ```
//!
//! Le manifeste vient du réseau : tout est vérifié avant d'être proposé
//! (adresse limitée aux Releases du dépôt et au tag de la version annoncée,
//! empreinte SHA-256 et taille plausibles). Le paquet téléchargé est ensuite
//! contrôlé contre cette taille et cette empreinte, puis relu par Android
//! (même application, même clé de signature, version plus récente).
//!
//! Aucun réseau ici : le texte est lu par l'application, qui le passe à
//! [`select`] après l'avoir désérialisé.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::changelog::parse_version;

/// Entrée du manifeste pour cette application (téléphones ARM 64 bits).
pub const PLATFORM: &str = "android-aarch64";
/// Au-delà, le manifeste est refusé sans être lu.
pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
/// Au-delà, un paquet annoncé est refusé (l'APK fait quelques dizaines de Mo).
pub const MAX_APK_BYTES: u64 = 512 * 1024 * 1024;

/// Manifeste tel que publié ; seuls les champs utiles sont lus.
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub version: String,
    /// Notes de version (extrait de `CHANGELOG.md`), affichées avec la proposition.
    #[serde(default)]
    pub changelog: String,
    #[serde(default)]
    pub platforms: BTreeMap<String, Entry>,
}

/// Paquet publié pour une plateforme. Champs facultatifs à la lecture : une
/// entrée d'une autre forme ne bloque pas le manifeste, la nôtre est
/// vérifiée par [`select`].
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

/// Version proposée, vérifiée : prête à télécharger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApkRelease {
    pub version: String,
    pub url: String,
    /// Empreinte attendue, 64 chiffres hexadécimaux en minuscules.
    pub sha256: String,
    pub size: u64,
    pub changelog: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApkUpdateError {
    #[error("numéro de version illisible : {0}")]
    Version(String),
    #[error("la version {0} n'est pas publiée pour Android")]
    NotPublished(String),
    #[error("adresse de téléchargement inattendue")]
    Url,
    #[error("empreinte SHA-256 invalide")]
    Sha256,
    #[error("taille annoncée invalide")]
    Size,
}

/// Version à proposer, ou `None` si `current` est déjà à jour (ou plus
/// récente). `downloads` : préfixe autorisé des adresses, terminé par `/`
/// (`https://github.com/<dépôt>/releases/download/`) ; le paquet doit en
/// plus être rangé sous le tag de la version annoncée (`v0.0.30/`).
pub fn select(
    manifest: &Manifest,
    current: &str,
    downloads: &str,
) -> Result<Option<ApkRelease>, ApkUpdateError> {
    let announced = parse_version(&manifest.version)
        .ok_or_else(|| ApkUpdateError::Version(manifest.version.clone()))?;
    let installed =
        parse_version(current).ok_or_else(|| ApkUpdateError::Version(current.to_string()))?;
    if announced <= installed {
        return Ok(None);
    }
    let version = format!("{}.{}.{}", announced.0, announced.1, announced.2);
    let entry = manifest
        .platforms
        .get(PLATFORM)
        .ok_or_else(|| ApkUpdateError::NotPublished(version.clone()))?;
    check_url(&entry.url, downloads, &version)?;
    let sha256 = entry.sha256.trim().to_ascii_lowercase();
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ApkUpdateError::Sha256);
    }
    if entry.size == 0 || entry.size > MAX_APK_BYTES {
        return Err(ApkUpdateError::Size);
    }
    Ok(Some(ApkRelease {
        version,
        url: entry.url.clone(),
        sha256,
        size: entry.size,
        changelog: manifest.changelog.clone(),
    }))
}

/// Adresse sous `downloads` + `v<version>/`, nom de fichier simple : ni
/// remontée de dossier, ni paramètre, ni caractère inattendu.
fn check_url(url: &str, downloads: &str, version: &str) -> Result<(), ApkUpdateError> {
    if !downloads.starts_with("https://") || !downloads.ends_with('/') {
        return Err(ApkUpdateError::Url);
    }
    let file = url
        .strip_prefix(downloads)
        .and_then(|rest| rest.strip_prefix(&format!("v{version}/")))
        .ok_or(ApkUpdateError::Url)?;
    let simple = file
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if file.is_empty() || !simple || file.contains("..") || !file.ends_with(".apk") {
        return Err(ApkUpdateError::Url);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOWNLOADS: &str = "https://github.com/Aquadrox/PdfEditor/releases/download/";
    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn manifest(version: &str, url: &str, sha256: &str, size: u64) -> Manifest {
        let json = serde_json::json!({
            "version": version,
            "notes": "https://github.com/Aquadrox/PdfEditor/releases/tag/v0.0.30",
            "changelog": "## 0.0.30 — 2026-11-19\n\n- [Android] Mises à jour.",
            "pub_date": "2026-11-19T10:00:00Z",
            "platforms": { PLATFORM: { "url": url, "sha256": sha256, "size": size } },
        });
        serde_json::from_value(json).unwrap()
    }

    fn url(name: &str) -> String {
        format!("{DOWNLOADS}v0.0.30/{name}")
    }

    #[test]
    fn newer_version_is_proposed() {
        let m = manifest(
            "0.0.30",
            &url("PdfEditor_0.0.30_android.apk"),
            SHA,
            40_000_000,
        );
        let release = select(&m, "0.0.29", DOWNLOADS).unwrap().unwrap();
        assert_eq!(release.version, "0.0.30");
        assert_eq!(release.url, url("PdfEditor_0.0.30_android.apk"));
        assert_eq!(release.sha256, SHA);
        assert_eq!(release.size, 40_000_000);
        assert!(release.changelog.contains("Mises à jour"));
    }

    #[test]
    fn same_or_older_version_is_not_proposed() {
        let m = manifest("0.0.30", &url("PdfEditor_0.0.30_android.apk"), SHA, 1);
        assert_eq!(select(&m, "0.0.30", DOWNLOADS), Ok(None));
        assert_eq!(select(&m, "0.1.0", DOWNLOADS), Ok(None));
        // Même une entrée invalide n'est pas lue : rien à proposer.
        let broken = manifest("0.0.30", "http://ailleurs/x.apk", "x", 0);
        assert_eq!(select(&broken, "0.0.30", DOWNLOADS), Ok(None));
    }

    #[test]
    fn versions_compare_as_numbers() {
        let m = manifest("0.0.30", &url("a.apk"), SHA, 1);
        assert!(select(&m, "0.0.9", DOWNLOADS).unwrap().is_some());
    }

    #[test]
    fn unreadable_versions_are_refused() {
        let m = manifest("dernière", &url("a.apk"), SHA, 1);
        assert_eq!(
            select(&m, "0.0.29", DOWNLOADS),
            Err(ApkUpdateError::Version("dernière".into()))
        );
        let m = manifest("0.0.30", &url("a.apk"), SHA, 1);
        assert!(matches!(
            select(&m, "?", DOWNLOADS),
            Err(ApkUpdateError::Version(_))
        ));
    }

    #[test]
    fn missing_platform_is_explained() {
        let mut m = manifest("0.0.30", &url("a.apk"), SHA, 1);
        m.platforms.clear();
        assert_eq!(
            select(&m, "0.0.29", DOWNLOADS),
            Err(ApkUpdateError::NotPublished("0.0.30".into()))
        );
    }

    #[test]
    fn urls_outside_the_release_are_refused() {
        let bad = [
            "https://example.com/PdfEditor.apk".to_string(),
            "http://github.com/Aquadrox/PdfEditor/releases/download/v0.0.30/a.apk".to_string(),
            format!("{DOWNLOADS}v0.0.29/a.apk"),
            format!("{DOWNLOADS}v0.0.30/../v0.0.1/a.apk"),
            format!("{DOWNLOADS}v0.0.30/a.apk?x=1"),
            format!("{DOWNLOADS}v0.0.30/sous/a.apk"),
            format!("{DOWNLOADS}v0.0.30/a.exe"),
            format!("{DOWNLOADS}v0.0.30/"),
            "https://github.com/Autre/PdfEditor/releases/download/v0.0.30/a.apk".to_string(),
        ];
        for url in bad {
            let m = manifest("0.0.30", &url, SHA, 1);
            assert_eq!(
                select(&m, "0.0.29", DOWNLOADS),
                Err(ApkUpdateError::Url),
                "{url}"
            );
        }
    }

    #[test]
    fn prefix_must_be_https_and_a_folder() {
        let m = manifest("0.0.30", &url("a.apk"), SHA, 1);
        let prefix = DOWNLOADS.trim_end_matches('/');
        assert_eq!(select(&m, "0.0.29", prefix), Err(ApkUpdateError::Url));
        let insecure = DOWNLOADS.replace("https", "http");
        assert_eq!(select(&m, "0.0.29", &insecure), Err(ApkUpdateError::Url));
    }

    #[test]
    fn digest_must_be_sha256_hex() {
        for sha in ["", "abc", &"g".repeat(64), &format!("{SHA}00")] {
            let m = manifest("0.0.30", &url("a.apk"), sha, 1);
            assert_eq!(
                select(&m, "0.0.29", DOWNLOADS),
                Err(ApkUpdateError::Sha256),
                "{sha}"
            );
        }
        // Majuscules acceptées, rangées en minuscules.
        let m = manifest("0.0.30", &url("a.apk"), &SHA.to_uppercase(), 1);
        assert_eq!(
            select(&m, "0.0.29", DOWNLOADS).unwrap().unwrap().sha256,
            SHA
        );
    }

    #[test]
    fn size_must_be_plausible() {
        for size in [0, MAX_APK_BYTES + 1] {
            let m = manifest("0.0.30", &url("a.apk"), SHA, size);
            assert_eq!(select(&m, "0.0.29", DOWNLOADS), Err(ApkUpdateError::Size));
        }
    }

    #[test]
    fn unknown_fields_and_other_platforms_are_ignored() {
        let json = format!(
            r#"{{"version":"0.0.30","extra":1,"platforms":{{
                "windows-x86_64":{{"url":"x","signature":"y"}},
                "{PLATFORM}":{{"url":"{}","sha256":"{SHA}","size":5,"extra":true}}}}}}"#,
            url("a.apk")
        );
        let m: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(select(&m, "0.0.29", DOWNLOADS).unwrap().unwrap().size, 5);
        let json = format!(
            r#"{{"version":"0.0.30","platforms":{{
                "{PLATFORM}":{{"url":"{}","sha256":"{SHA}","size":5,"extra":true}}}}}}"#,
            url("a.apk")
        );
        let m: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(m.changelog, "");
        assert_eq!(select(&m, "0.0.29", DOWNLOADS).unwrap().unwrap().size, 5);
    }
}
