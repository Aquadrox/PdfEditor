//! Sauvegarde automatique (F-02) vue de l'application : quand écrire, quoi
//! effacer, et la récupération proposée au démarrage.

use std::time::Duration;

use crate::app::{App, now};
use crate::edit_view::EditView;
use crate::error::{AppError, ErrorKind};
use crate::recovery::RecoveryView;

impl App {
    /// Intervalle réglé ; `None` : désactivée (réglage à 0 ou dossier absent).
    pub fn autosave_interval(&self) -> Option<Duration> {
        self.recovery.as_ref()?;
        let seconds = self.settings.get().files.autosave_seconds;
        (seconds > 0).then(|| Duration::from_secs(u64::from(seconds)))
    }

    /// Écrit les documents modifiés depuis la dernière fois, efface ceux
    /// revenus à leur état enregistré ; renvoie les documents écrits.
    pub fn autosave(&self) -> Vec<u64> {
        let Some(recovery) = &self.recovery else {
            return Vec::new();
        };
        let work = self.viewer.autosave_work();
        for doc in work.clean {
            recovery.remove(doc);
        }
        let saved_at = now();
        let mut written = Vec::new();
        for pending in work.pending {
            match recovery.write(pending.doc, &pending.snapshot, saved_at) {
                Ok(()) => {
                    self.viewer.mark_autosaved(pending.doc, pending.revision);
                    written.push(pending.doc);
                }
                // Jamais le nom ni le contenu du document dans le journal.
                Err(e) => log::warn!("sauvegarde automatique impossible : {e}"),
            }
        }
        written
    }

    /// Documents laissés par un lancement terminé sans fermeture normale.
    pub fn recoveries(&self) -> Vec<RecoveryView> {
        self.recovery
            .as_ref()
            .map(|r| r.orphans())
            .unwrap_or_default()
    }

    /// Reprend la récupération `key` dans le document `id`, déjà ouvert ;
    /// elle passe aussitôt dans la sauvegarde de ce lancement.
    pub fn restore_recovery(&self, id: u64, key: &str) -> Result<EditView, AppError> {
        let recovery = self.recovery.as_ref().ok_or_else(|| {
            AppError::new(
                ErrorKind::InvalidRequest,
                "sauvegarde automatique indisponible",
            )
        })?;
        let recovered = recovery.load(key)?;
        let view = self.viewer.restore(id, &recovered)?;
        self.autosave();
        recovery.discard(key);
        Ok(view)
    }

    pub fn discard_recovery(&self, key: &str) {
        if let Some(recovery) = &self.recovery {
            recovery.discard(key);
        }
    }

    /// Ferme un document : ses modifications abandonnées ne sont plus à récupérer.
    pub fn close_document(&self, id: u64) {
        self.viewer.close(id);
        if let Some(recovery) = &self.recovery {
            recovery.remove(id);
        }
    }

    /// Fin du processus. Des modifications non confirmées (redémarrage
    /// pour une mise à jour…) sont écrites une dernière fois et seront
    /// proposées au prochain lancement ; sinon, rien n'est gardé.
    pub fn on_exit(&self) {
        if self.viewer.any_dirty() {
            self.autosave();
        } else {
            self.shutdown();
        }
    }

    /// Fermeture confirmée par l'utilisateur (modifications enregistrées ou
    /// abandonnées) : rien à récupérer au prochain lancement.
    pub fn shutdown(&self) {
        if let Some(recovery) = &self.recovery {
            recovery.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use pde_core::edit::{Element, TextBox, TextStyle};
    use pde_core::geometry::Point;

    use super::*;
    use crate::fake::{FakeEngine, SIMPLE_PDF};
    use crate::recent::RecentFiles;
    use crate::recovery::RecoveryStore;
    use crate::settings_store::SettingsStore;

    struct Run {
        dir: PathBuf,
        pdf: PathBuf,
    }

    impl Run {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("pde-autosave-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let pdf = dir.join("contrat.pdf");
            std::fs::write(&pdf, SIMPLE_PDF).unwrap();
            Self { dir, pdf }
        }

        /// Un lancement de l'application sur le même dossier.
        fn launch(&self, settings: &str) -> App {
            let config = self.dir.join("config.toml");
            std::fs::write(&config, settings).unwrap();
            App::new(
                Arc::new(FakeEngine::default()),
                SettingsStore::load(config),
                RecentFiles::load(self.dir.join("recent.json")),
            )
            .with_recovery(RecoveryStore::open(&self.dir.join("recovery")).unwrap())
        }
    }

    impl Drop for Run {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn text(content: &str) -> Element {
        Element::Text(TextBox {
            page: 0,
            top_left: Point::new(72.0, 100.0),
            width: 200.0,
            text: content.into(),
            style: TextStyle::default(),
        })
    }

    #[test]
    fn interval_follows_the_setting() {
        let run = Run::new("intervalle");
        assert_eq!(
            run.launch("").autosave_interval(),
            Some(Duration::from_secs(60))
        );
        let off = run.launch("[files]\nautosave_seconds = 0\n");
        assert_eq!(off.autosave_interval(), None);
    }

    #[test]
    fn crash_then_recover_into_the_reopened_file() {
        let run = Run::new("plantage");
        let first = run.launch("");
        let doc = first.open_document(&run.pdf, None).unwrap().id;
        first.viewer.add_element(doc, text("Annexe B")).unwrap();
        assert_eq!(first.autosave(), [doc]);
        assert!(first.autosave().is_empty(), "rien de nouveau");
        // Plantage : ni enregistrement, ni fermeture normale.
        drop(first);

        let second = run.launch("");
        let offered = second.recoveries();
        assert_eq!(offered.len(), 1);
        assert_eq!(offered[0].file_name, "contrat.pdf");
        assert_eq!(offered[0].elements, 1);
        let doc = second.open_document(&run.pdf, None).unwrap().id;
        let view = second.restore_recovery(doc, &offered[0].id).unwrap();
        assert!(view.dirty);
        assert_eq!(view.elements.len(), 1);
        // Reprise : désormais dans la sauvegarde de ce lancement.
        assert!(second.recoveries().is_empty());
        drop(second);
        let third = run.launch("");
        assert_eq!(third.recoveries().len(), 1);
    }

    #[test]
    fn saving_closing_or_quitting_leaves_nothing_to_recover() {
        let run = Run::new("normal");
        let app = run.launch("");
        let doc = app.open_document(&run.pdf, None).unwrap().id;
        app.viewer.add_element(doc, text("a")).unwrap();
        app.autosave();
        app.save_document(doc, None).unwrap();
        drop(app);
        assert!(run.launch("").recoveries().is_empty(), "enregistré");

        let app = run.launch("");
        let doc = app.open_document(&run.pdf, None).unwrap().id;
        app.viewer.add_element(doc, text("b")).unwrap();
        app.autosave();
        app.close_document(doc);
        drop(app);
        assert!(
            run.launch("").recoveries().is_empty(),
            "fermé sans enregistrer"
        );

        let app = run.launch("");
        let doc = app.open_document(&run.pdf, None).unwrap().id;
        app.viewer.add_element(doc, text("c")).unwrap();
        app.autosave();
        app.shutdown();
        app.on_exit();
        drop(app);
        assert!(run.launch("").recoveries().is_empty(), "quitté normalement");
    }

    #[test]
    fn unconfirmed_exit_keeps_the_latest_changes() {
        let run = Run::new("redemarrage");
        let app = run.launch("");
        let doc = app.open_document(&run.pdf, None).unwrap().id;
        app.viewer.add_element(doc, text("a")).unwrap();
        app.autosave();
        app.viewer.add_element(doc, text("b")).unwrap();
        // Redémarrage pour une mise à jour, sans passer par la question.
        app.on_exit();
        drop(app);
        let offered = run.launch("").recoveries();
        assert_eq!(offered[0].elements, 2, "dernier état écrit à la sortie");
    }

    #[test]
    fn ignored_recovery_is_gone() {
        let run = Run::new("ignoree");
        let first = run.launch("");
        let doc = first.open_document(&run.pdf, None).unwrap().id;
        first.viewer.add_element(doc, text("a")).unwrap();
        first.autosave();
        drop(first);
        let second = run.launch("");
        let offered = second.recoveries();
        second.discard_recovery(&offered[0].id);
        assert!(second.recoveries().is_empty());
        drop(second);
        assert!(run.launch("").recoveries().is_empty());
    }
}
