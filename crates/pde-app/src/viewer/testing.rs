//! Document ouvert sur le faux moteur, pour les tests de la visionneuse.

use std::path::PathBuf;
use std::sync::Arc;

use super::Viewer;
use crate::fake::{FakeEngine, SIMPLE_PDF};

pub(crate) struct Session {
    pub engine: Arc<FakeEngine>,
    pub viewer: Viewer,
    pub id: u64,
    pub dir: PathBuf,
}

impl Session {
    pub fn new(name: &str) -> Self {
        let engine = Arc::new(FakeEngine::default());
        let viewer = Viewer::new(engine.clone(), 1 << 20);
        let dir = std::env::temp_dir().join(format!("pde-edit-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("contrat.pdf");
        std::fs::write(&path, SIMPLE_PDF).unwrap();
        let id = viewer.open_file(&path, None).unwrap().id;
        Self {
            engine,
            viewer,
            id,
            dir,
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
