//! Dossiers de l'application.
//!
//! | Usage | Windows | Linux |
//! |---|---|---|
//! | réglages (`config.toml`) | `%APPDATA%\PdfEditor` | `~/.config/pdfeditor` |
//! | données (récents, journaux) | `%LOCALAPPDATA%\PdfEditor` | `~/.local/share/pdfeditor` |
//!
//! La variable `PDE_HOME` place tout dans un seul dossier (tests, version
//! portable).

use std::io;
use std::path::{Path, PathBuf};

pub const HOME_ENV: &str = "PDE_HOME";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDirs {
    pub config: PathBuf,
    pub data: PathBuf,
    pub logs: PathBuf,
}

impl AppDirs {
    /// Tous les dossiers sous `root` (variable `PDE_HOME`, tests).
    pub fn under(root: &Path) -> Self {
        Self {
            config: root.to_path_buf(),
            data: root.to_path_buf(),
            logs: root.join("logs"),
        }
    }

    /// Dossiers du système, ou ceux de `PDE_HOME` si elle est définie.
    pub fn locate() -> Result<Self, String> {
        if let Some(root) = std::env::var_os(HOME_ENV).filter(|v| !v.is_empty()) {
            return Ok(Self::under(Path::new(&root)));
        }
        let name = if cfg!(windows) {
            "PdfEditor"
        } else {
            "pdfeditor"
        };
        let config = ::dirs::config_dir().ok_or("dossier de configuration introuvable")?;
        let data = ::dirs::data_local_dir().ok_or("dossier de données introuvable")?;
        let data = data.join(name);
        Ok(Self {
            config: config.join(name),
            logs: data.join("logs"),
            data,
        })
    }

    /// Crée les dossiers manquants.
    pub fn create(&self) -> io::Result<()> {
        for dir in [&self.config, &self.data, &self.logs] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config.join("config.toml")
    }

    pub fn recent_file(&self) -> PathBuf {
        self.data.join("recent.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_under_one_root() {
        let dirs = AppDirs::under(Path::new("/tmp/pde"));
        assert_eq!(dirs.settings_file(), Path::new("/tmp/pde/config.toml"));
        assert_eq!(dirs.recent_file(), Path::new("/tmp/pde/recent.json"));
        assert_eq!(dirs.logs, Path::new("/tmp/pde/logs"));
    }

    #[test]
    fn create_makes_missing_folders() {
        let root = std::env::temp_dir().join(format!("pde-dirs-{}", std::process::id()));
        let dirs = AppDirs::under(&root);
        dirs.create().unwrap();
        assert!(dirs.logs.is_dir());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn system_folders_are_found() {
        // Sans PDE_HOME (non définie en CI) : dossiers du système.
        if std::env::var_os(HOME_ENV).is_none() {
            let dirs = AppDirs::locate().unwrap();
            assert!(dirs.config.ends_with("PdfEditor") || dirs.config.ends_with("pdfeditor"));
            assert!(dirs.logs.starts_with(&dirs.data));
        }
    }
}
