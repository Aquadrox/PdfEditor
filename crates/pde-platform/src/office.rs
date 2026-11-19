//! Conversion des documents de bureautique en PDF (E-13) par les logiciels
//! installés : Microsoft Word sous Windows (automatisation COM par
//! PowerShell), LibreOffice partout (`soffice --headless --convert-to pdf`).
//! Les chemins passent par des variables d'environnement ou des arguments,
//! jamais dans le texte d'une commande ; chaque conversion a un délai.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use pde_core::convert::DocumentConverter;

/// Durée maximale d'une conversion (gros documents, premier lancement).
const TIMEOUT: Duration = Duration::from_secs(180);

/// Lance `command` et attend sa fin, au plus `TIMEOUT` ; `Ok(())` s'il a réussi.
fn run(mut command: Command, what: &str) -> Result<(), String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{what} n'a pas pu être lancé : {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("{what} a échoué ({status})")),
            Ok(None) if started.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{what} ne répond pas : conversion abandonnée"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(e) => return Err(format!("{what} : {e}")),
        }
    }
}

/// PDF attendu dans `out_dir` pour `input` (même nom, extension `.pdf`).
fn expected_pdf(input: &Path, out_dir: &Path) -> Result<PathBuf, String> {
    let stem = input.file_stem().ok_or("nom de fichier invalide")?;
    Ok(out_dir.join(stem).with_extension("pdf"))
}

/// LibreOffice (ou OpenOffice) en mode sans fenêtre.
pub struct LibreOffice {
    program: PathBuf,
}

impl LibreOffice {
    /// Le programme `soffice` trouvé aux emplacements habituels ou dans le PATH.
    pub fn find() -> Option<Self> {
        candidates()
            .into_iter()
            .find(|p| p.is_file())
            .map(|program| Self { program })
    }
}

fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    #[cfg(windows)]
    for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        if let Some(dir) = std::env::var_os(var) {
            out.push(PathBuf::from(dir).join("LibreOffice/program/soffice.exe"));
        }
    }
    #[cfg(target_os = "macos")]
    out.push(PathBuf::from(
        "/Applications/LibreOffice.app/Contents/MacOS/soffice",
    ));
    let names: &[&str] = if cfg!(windows) {
        &["soffice.exe"]
    } else {
        &["soffice", "libreoffice"]
    };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            out.extend(names.iter().map(|n| dir.join(n)));
        }
    }
    out
}

impl DocumentConverter for LibreOffice {
    fn name(&self) -> &str {
        "LibreOffice"
    }

    fn convert(&self, input: &Path, out_dir: &Path) -> Result<PathBuf, String> {
        // Profil à part : une fenêtre LibreOffice déjà ouverte ne bloque pas
        // la conversion.
        let profile = out_dir.join("profil-libreoffice");
        let profile_url = format!(
            "file:///{}",
            profile
                .display()
                .to_string()
                .replace('\\', "/")
                .trim_start_matches('/')
        );
        let mut command = Command::new(&self.program);
        command
            .arg(format!("-env:UserInstallation={profile_url}"))
            .args([
                "--headless",
                "--norestore",
                "--nolockcheck",
                "--convert-to",
                "pdf",
                "--outdir",
            ])
            .arg(out_dir)
            .arg(input);
        run(command, "LibreOffice")?;
        let _ = std::fs::remove_dir_all(&profile);
        let pdf = expected_pdf(input, out_dir)?;
        if pdf.is_file() {
            Ok(pdf)
        } else {
            Err("LibreOffice n'a pas produit de PDF".into())
        }
    }
}

/// Microsoft Word, piloté par PowerShell (Windows seulement).
pub struct Word;

impl Word {
    /// Word est installé (classe COM `Word.Application` enregistrée).
    pub fn find() -> Option<Self> {
        if !cfg!(windows) {
            return None;
        }
        let mut command = Command::new("reg");
        command.args(["query", r"HKCR\Word.Application\CLSID"]);
        run(command, "reg").ok().map(|()| Word)
    }
}

/// Script PowerShell : chemins lus dans l'environnement (PDE_IN, PDE_OUT),
/// document ouvert en lecture seule, exporté en PDF (17), Word refermé.
const WORD_SCRIPT: &str = "$ErrorActionPreference = 'Stop'; \
$w = New-Object -ComObject Word.Application; $w.Visible = $false; $w.DisplayAlerts = 0; \
try { $d = $w.Documents.Open($env:PDE_IN, $false, $true, $false); \
$d.ExportAsFixedFormat($env:PDE_OUT, 17); $d.Close(0) } finally { $w.Quit() }";

impl DocumentConverter for Word {
    fn name(&self) -> &str {
        "Microsoft Word"
    }

    fn convert(&self, input: &Path, out_dir: &Path) -> Result<PathBuf, String> {
        let pdf = expected_pdf(input, out_dir)?;
        let mut command = Command::new("powershell");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                WORD_SCRIPT,
            ])
            .env("PDE_IN", input)
            .env("PDE_OUT", &pdf);
        run(command, "Microsoft Word")?;
        if pdf.is_file() {
            Ok(pdf)
        } else {
            Err("Microsoft Word n'a pas produit de PDF".into())
        }
    }
}

/// Convertisseurs installés, du plus fidèle au moins fidèle pour un
/// document Word : Word, puis LibreOffice.
pub fn installed() -> Vec<std::sync::Arc<dyn DocumentConverter>> {
    let mut out: Vec<std::sync::Arc<dyn DocumentConverter>> = Vec::new();
    if let Some(word) = Word::find() {
        out.push(std::sync::Arc::new(word));
    }
    if let Some(office) = LibreOffice::find() {
        out.push(std::sync::Arc::new(office));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failing_or_missing_program_is_reported() {
        let missing = Command::new("programme-qui-n-existe-pas-pde");
        assert!(run(missing, "Test").unwrap_err().contains("lancé"));
        let pdf = expected_pdf(Path::new("/docs/bail.docx"), Path::new("/tmp/x")).unwrap();
        assert_eq!(pdf, PathBuf::from("/tmp/x/bail.pdf"));
    }

    /// Vraie conversion si LibreOffice est installé (sinon rien à vérifier).
    #[test]
    fn libreoffice_converts_a_document_when_installed() {
        let Some(office) = LibreOffice::find() else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("pde-office-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("lettre.rtf");
        std::fs::write(&input, r"{\rtf1\ansi Nom : ________\par}").unwrap();
        let out = dir.join("pdf");
        std::fs::create_dir_all(&out).unwrap();
        let pdf = office.convert(&input, &out).unwrap();
        assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
