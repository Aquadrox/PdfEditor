//! Thread de PDFium : documents ouverts et traitement des demandes.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use pde_core::document::DocumentId;
use pde_core::edit::{Element, FieldBox};
use pde_core::engine::{Edition, EngineError, OpenedDocument, Placed};
use pdfium_render::prelude::{PdfDocument, Pdfium};

use crate::convert::{internal, open_error};
use crate::engine::Command;
use crate::{arrange, form, reader, writer};

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
        Command::FormFields {
            doc,
            edition,
            reply,
        } => {
            let result = slot(docs, doc)
                .and_then(|s| Ok((arranged(pdfium, s, &edition)?, s)))
                .and_then(|(arranged, s)| form::form_fields(&arranged, &s.source));
            let _ = reply.send(result);
        }
        Command::PageImages {
            doc,
            edition,
            page,
            reply,
        } => {
            let result = slot(docs, doc)
                .and_then(|s| arranged(pdfium, s, &edition))
                .and_then(|arranged| writer::page_images(&arranged, page));
            let _ = reply.send(result);
        }
        Command::PageTextBlocks {
            doc,
            edition,
            page,
            reply,
        } => {
            let result = slot(docs, doc)
                .and_then(|s| arranged(pdfium, s, &edition))
                .and_then(|arranged| writer::page_text_blocks(&arranged, page));
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
    let mut fresh = arranged(pdfium, slot, edition)?;
    let placed = writer::apply(&mut fresh, &edition.elements, &edition.images)?;
    let fresh = finish(pdfium, slot, fresh, edition)?;
    Ok((fresh, placed))
}

/// L'original rechargé, pages rangées selon le plan, sans aucun élément.
fn arranged<'a>(
    pdfium: &'a Pdfium,
    slot: &Slot<'a>,
    edition: &Edition,
) -> Result<PdfDocument<'a>, EngineError> {
    let mut fresh = load(pdfium, &slot.source, slot.password.as_deref()).map_err(internal)?;
    arrange::arrange(
        pdfium,
        &mut fresh,
        &edition.plan,
        (&slot.source, slot.password.as_deref()),
        &edition.imports,
    )?;
    Ok(fresh)
}

/// Formulaire et propriétés (E-08, E-11, E-12) : champs conçus et valeurs
/// écrits par `lopdf`, document rechargé (PDFium redessine les champs),
/// puis aplati si demandé.
fn finish<'a>(
    pdfium: &'a Pdfium,
    slot: &Slot<'a>,
    doc: PdfDocument<'a>,
    edition: &Edition,
) -> Result<PdfDocument<'a>, EngineError> {
    let boxes: Vec<&FieldBox> = edition
        .elements
        .iter()
        .filter_map(|(_, e)| match e {
            Element::Field(field) => Some(&**field),
            _ => None,
        })
        .collect();
    if edition.fields.is_empty()
        && edition.metadata.is_none()
        && !edition.flatten
        && boxes.is_empty()
    {
        return Ok(doc);
    }
    let designed = boxes
        .into_iter()
        .map(|field| {
            let (_, display) = writer::page_of(&doc, field.page)?;
            let rect = field.rect();
            let (x0, y0) = display.page_point(rect.min());
            let (x1, y1) = display.page_point(rect.max());
            Ok(form::Designed {
                field,
                page: field.page,
                rect: [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)],
            })
        })
        .collect::<Result<Vec<_>, EngineError>>()?;
    let bytes = doc.save_to_bytes().map_err(internal)?;
    drop(doc);
    let bytes = form::patch(
        bytes,
        &form::FormPatch {
            values: &edition.fields,
            metadata: edition.metadata.as_ref(),
            designed: &designed,
            flatten: edition.flatten,
        },
    )?;
    let doc = load(pdfium, &bytes, slot.password.as_deref()).map_err(internal)?;
    if edition.flatten {
        for index in 0..doc.pages().len() {
            let mut page = doc.pages().get(index).map_err(internal)?;
            page.flatten().map_err(internal)?;
        }
    }
    Ok(doc)
}
