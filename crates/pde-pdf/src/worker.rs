//! Thread de PDFium : documents ouverts et traitement des demandes.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use pde_core::document::DocumentId;
use pde_core::engine::{Edition, EngineError, OpenedDocument, Placed};
use pdfium_render::prelude::{PdfDocument, Pdfium};

use crate::convert::{internal, open_error};
use crate::engine::Command;
use crate::{arrange, reader, writer};

/// Document ouvert : sa version affichée (d'origine, ou avec les éléments
/// posés) et de quoi repartir de l'original à chaque modification.
struct Slot<'a> {
    current: PdfDocument<'a>,
    source: Arc<Vec<u8>>,
    password: Option<String>,
}

/// Boucle du thread : traite les demandes jusqu'à ce que toutes les
/// poignées soient lâchées. Les documents sont fermés avant PDFium.
pub(crate) fn serve(pdfium: &Pdfium, commands: Receiver<Command>) {
    let mut docs: HashMap<DocumentId, Slot<'_>> = HashMap::new();
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
    docs: &mut HashMap<DocumentId, Slot<'a>>,
    next_id: &mut u64,
    command: Command,
) {
    match command {
        Command::Open {
            bytes,
            password,
            reply,
        } => {
            let source = Arc::new(bytes);
            let result = load(pdfium, &source, password.as_deref())
                .map_err(|e| open_error(e, password.is_some()))
                .and_then(|current| {
                    let info = reader::document_info(&current)?;
                    let id = DocumentId(*next_id);
                    *next_id += 1;
                    docs.insert(
                        id,
                        Slot {
                            current,
                            source,
                            password,
                        },
                    );
                    Ok(OpenedDocument { id, info })
                });
            let _ = reply.send(result);
        }
        Command::Render {
            doc,
            request,
            reply,
        } => {
            let result = slot(docs, doc).and_then(|s| reader::render(&s.current, request));
            let _ = reply.send(result);
        }
        Command::Text { doc, page, reply } => {
            let result = slot(docs, doc).and_then(|s| reader::page_text(&s.current, page));
            let _ = reply.send(result);
        }
        Command::ApplyEdits {
            doc,
            edition,
            reply,
        } => {
            let result = docs
                .get_mut(&doc)
                .ok_or(EngineError::UnknownDocument)
                .and_then(|s| {
                    let (fresh, placed) = build(pdfium, s, &edition)?;
                    s.current = fresh;
                    Ok(placed)
                });
            let _ = reply.send(result);
        }
        Command::Export {
            doc,
            edition,
            reply,
        } => {
            let result = slot(docs, doc)
                .and_then(|s| build(pdfium, s, &edition))
                .and_then(|(built, _)| built.save_to_bytes().map_err(internal));
            let _ = reply.send(result);
        }
        Command::Save { doc, reply } => {
            let result = slot(docs, doc).and_then(|s| s.current.save_to_bytes().map_err(internal));
            let _ = reply.send(result);
        }
        Command::Close { doc } => {
            docs.remove(&doc);
        }
    }
}

fn slot<'s, 'a>(
    docs: &'s HashMap<DocumentId, Slot<'a>>,
    doc: DocumentId,
) -> Result<&'s Slot<'a>, EngineError> {
    docs.get(&doc).ok_or(EngineError::UnknownDocument)
}

/// Copie du document d'origine (PDFium garde ses propres octets).
fn load<'a>(
    pdfium: &'a Pdfium,
    source: &[u8],
    password: Option<&str>,
) -> Result<PdfDocument<'a>, pdfium_render::prelude::PdfiumError> {
    pdfium.load_pdf_from_byte_vec(source.to_vec(), password)
}

/// Recharge l'original, range ses pages et pose les éléments, dans un
/// nouveau document : celui affiché n'est remplacé qu'en cas de succès (un
/// élément impossible à écrire laisse l'affichage précédent intact), et une
/// extraction ne le touche pas.
fn build<'a>(
    pdfium: &'a Pdfium,
    slot: &Slot<'a>,
    edition: &Edition,
) -> Result<(PdfDocument<'a>, Vec<Placed>), EngineError> {
    let mut fresh = load(pdfium, &slot.source, slot.password.as_deref()).map_err(internal)?;
    arrange::arrange(
        pdfium,
        &mut fresh,
        &edition.plan,
        (&slot.source, slot.password.as_deref()),
        &edition.imports,
    )?;
    let placed = writer::apply(&mut fresh, &edition.elements, &edition.images)?;
    Ok((fresh, placed))
}
