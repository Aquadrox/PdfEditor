//! Thread dédié à PDFium et poignée pour lui adresser des demandes.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};

use pde_core::document::{DocumentId, PageIndex};
use pde_core::engine::{EngineError, OpenedDocument, PdfEngine};
use pde_core::render::{Bitmap, RenderRequest};
use pde_core::text::PageText;
use pdfium_render::prelude::{PdfDocument, Pdfium};

use crate::convert::open_error;
use crate::library::current_candidates;
use crate::reader;

type Reply<T> = SyncSender<Result<T, EngineError>>;

/// Demandes traitées par le thread de PDFium.
enum Command {
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

/// Boucle du thread : traite les demandes jusqu'à ce que toutes les
/// poignées soient lâchées. Les documents sont fermés avant PDFium.
fn serve(pdfium: &Pdfium, commands: Receiver<Command>) {
    let mut docs: HashMap<DocumentId, PdfDocument<'_>> = HashMap::new();
    let mut next_id = 1;
    for command in commands {
        // Une panique sur un fichier piégé ne doit pas arrêter le moteur :
        // la demande échoue (réponse perdue → erreur chez l'appelant), les
        // autres documents restent ouverts.
        let _ = catch_unwind(AssertUnwindSafe(|| {
            handle(pdfium, &mut docs, &mut next_id, command);
        }));
    }
}

fn handle<'a>(
    pdfium: &'a Pdfium,
    docs: &mut HashMap<DocumentId, PdfDocument<'a>>,
    next_id: &mut u64,
    command: Command,
) {
    match command {
        Command::Open {
            bytes,
            password,
            reply,
        } => {
            let result = pdfium
                .load_pdf_from_byte_vec(bytes, password.as_deref())
                .map_err(|e| open_error(e, password.is_some()))
                .and_then(|doc| {
                    let info = reader::document_info(&doc)?;
                    let id = DocumentId(*next_id);
                    *next_id += 1;
                    docs.insert(id, doc);
                    Ok(OpenedDocument { id, info })
                });
            let _ = reply.send(result);
        }
        Command::Render {
            doc,
            request,
            reply,
        } => {
            let result = docs
                .get(&doc)
                .ok_or(EngineError::UnknownDocument)
                .and_then(|d| reader::render(d, request));
            let _ = reply.send(result);
        }
        Command::Text { doc, page, reply } => {
            let result = docs
                .get(&doc)
                .ok_or(EngineError::UnknownDocument)
                .and_then(|d| reader::page_text(d, page));
            let _ = reply.send(result);
        }
        Command::Close { doc } => {
            docs.remove(&doc);
        }
    }
}
