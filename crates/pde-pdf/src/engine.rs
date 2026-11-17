//! Thread dédié à PDFium et poignée pour lui adresser des demandes.

use std::sync::OnceLock;
use std::sync::mpsc::{Sender, SyncSender, channel, sync_channel};

use pde_core::document::{DocumentId, PageIndex};
use pde_core::edit::FormField;
use pde_core::engine::{Edition, EngineError, OpenedDocument, PageImage, PdfEngine, Placed};
use pde_core::render::{Bitmap, RenderRequest};
use pde_core::text::PageText;
use pdfium_render::prelude::Pdfium;

use crate::library::current_candidates;
use crate::worker::serve;

pub(crate) type Reply<T> = SyncSender<Result<T, EngineError>>;

/// Demandes traitées par le thread de PDFium.
pub(crate) enum Command {
    Open {
        bytes: Vec<u8>,
        password: Option<String>,
        reply: Reply<OpenedDocument>,
    },
    Render {
        doc: DocumentId,
        request: RenderRequest,
        reply: Reply<Bitmap>,
    },
    Text {
        doc: DocumentId,
        page: PageIndex,
        reply: Reply<PageText>,
    },
    ApplyEdits {
        doc: DocumentId,
        edition: Edition,
        reply: Reply<Vec<Placed>>,
    },
    Export {
        doc: DocumentId,
        edition: Edition,
        reply: Reply<Vec<u8>>,
    },
    FormFields {
        doc: DocumentId,
        edition: Edition,
        reply: Reply<Vec<FormField>>,
    },
    PageImages {
        doc: DocumentId,
        edition: Edition,
        page: PageIndex,
        reply: Reply<Vec<PageImage>>,
    },
    Save {
        doc: DocumentId,
        reply: Reply<Vec<u8>>,
    },
    Close {
        doc: DocumentId,
    },
}

/// Poignée vers le moteur PDFium : utilisable depuis tous les threads.
///
/// PDFium se charge une seule fois par processus : on obtient le moteur par
/// [`PdfiumEngine::global`], jamais par un constructeur.
pub struct PdfiumEngine {
    commands: Sender<Command>,
}

static GLOBAL: OnceLock<Result<PdfiumEngine, EngineError>> = OnceLock::new();

impl PdfiumEngine {
    /// Moteur du processus, démarré au premier appel. L'erreur (bibliothèque
    /// introuvable…) est mémorisée : elle sera la même à chaque appel.
    pub fn global() -> Result<&'static PdfiumEngine, EngineError> {
        GLOBAL
            .get_or_init(Self::start)
            .as_ref()
            .map_err(Clone::clone)
    }

    fn start() -> Result<Self, EngineError> {
        let (commands, receiver) = channel();
        let (ready, started) = sync_channel(1);
        std::thread::Builder::new()
            .name("pdfium".into())
            .spawn(move || match bind() {
                Ok(pdfium) => {
                    // Le récepteur attend toujours cette réponse.
                    let _ = ready.send(Ok(()));
                    serve(&pdfium, receiver);
                }
                Err(error) => {
                    let _ = ready.send(Err(error));
                }
            })
            .map_err(|e| EngineError::Unavailable(format!("thread du moteur : {e}")))?;
        started.recv().map_err(|_| {
            EngineError::Unavailable("le moteur s'est arrêté au démarrage".into())
        })??;
        Ok(Self { commands })
    }

    /// Envoie une demande et attend sa réponse.
    fn ask<T>(&self, command: impl FnOnce(Reply<T>) -> Command) -> Result<T, EngineError> {
        let (reply, response) = sync_channel(1);
        self.commands
            .send(command(reply))
            .map_err(|_| EngineError::Unavailable("le moteur PDF est arrêté".into()))?;
        response
            .recv()
            .map_err(|_| EngineError::Internal("le moteur a échoué sur cette demande".into()))?
    }
}

impl PdfEngine for PdfiumEngine {
    fn open(
        &self,
        bytes: Vec<u8>,
        password: Option<String>,
    ) -> Result<OpenedDocument, EngineError> {
        self.ask(|reply| Command::Open {
            bytes,
            password,
            reply,
        })
    }

    fn render(&self, doc: DocumentId, request: RenderRequest) -> Result<Bitmap, EngineError> {
        self.ask(|reply| Command::Render {
            doc,
            request,
            reply,
        })
    }

    fn page_text(&self, doc: DocumentId, page: PageIndex) -> Result<PageText, EngineError> {
        self.ask(|reply| Command::Text { doc, page, reply })
    }

    fn apply_edits(&self, doc: DocumentId, edition: Edition) -> Result<Vec<Placed>, EngineError> {
        self.ask(|reply| Command::ApplyEdits {
            doc,
            edition,
            reply,
        })
    }

    fn export(&self, doc: DocumentId, edition: Edition) -> Result<Vec<u8>, EngineError> {
        self.ask(|reply| Command::Export {
            doc,
            edition,
            reply,
        })
    }

    fn form_fields(
        &self,
        doc: DocumentId,
        edition: Edition,
    ) -> Result<Vec<FormField>, EngineError> {
        self.ask(|reply| Command::FormFields {
            doc,
            edition,
            reply,
        })
    }

    fn page_images(
        &self,
        doc: DocumentId,
        edition: Edition,
        page: PageIndex,
    ) -> Result<Vec<PageImage>, EngineError> {
        self.ask(|reply| Command::PageImages {
            doc,
            edition,
            page,
            reply,
        })
    }

    fn save(&self, doc: DocumentId) -> Result<Vec<u8>, EngineError> {
        self.ask(|reply| Command::Save { doc, reply })
    }

    fn close(&self, doc: DocumentId) {
        // Moteur arrêté : il n'y a plus rien à fermer.
        let _ = self.commands.send(Command::Close { doc });
    }
}

/// Charge la bibliothèque PDFium depuis le premier dossier qui la contient.
fn bind() -> Result<Pdfium, EngineError> {
    let mut tried = Vec::new();
    for dir in current_candidates() {
        let path = Pdfium::pdfium_platform_library_name_at_path(&dir);
        if path.is_file() {
            return Pdfium::bind_to_library(&path)
                .map(Pdfium::new)
                .map_err(|e| EngineError::Unavailable(format!("{}: {e:?}", path.display())));
        }
        tried.push(path.display().to_string());
    }
    Err(EngineError::Unavailable(format!(
        "bibliothèque PDFium introuvable (cherchée dans : {})",
        tried.join(", ")
    )))
}
