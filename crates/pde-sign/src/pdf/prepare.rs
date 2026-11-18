//! Objets de la mise à jour qui ajoute une signature : dictionnaire `/Sig`,
//! champ et widget, page, formulaire, catalogue (verrouillage).

use lopdf::{Dictionary, Object, ObjectId};

use super::appearance;
use super::fields::{self, Field};
use super::geometry::PageFrame;
use super::incremental::{Body, Update};
use super::permissions;
use super::sig_dict::{self, Details, Lock, SigDict};
use super::source::Source;
use crate::error::{Result, SignError};

/// Où poser la signature.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// Signature invisible (widget de taille nulle sur la première page).
    Invisible,
    /// Nouveau champ dans ce cadre de la page (rang à partir de 0), en
    /// points dans la page telle qu'affichée : origine en bas à gauche de la
    /// zone visible, page tournée comme à l'écran.
    NewField { page: usize, rect: [f32; 4] },
    /// Champ de signature vide existant, désigné par son nom complet.
    Field(String),
}

/// Ce qu'écrit la signature, hors CMS.
pub(crate) struct Request<'a> {
    pub target: &'a Target,
    pub details: Details<'a>,
    /// Lignes de l'apparence (vide : rien d'affiché).
    pub appearance: &'a [String],
}

pub(crate) struct Prepared {
    pub update: Update,
    pub sig_id: ObjectId,
    pub sig: SigDict,
}

struct Allocator(u32);

impl Allocator {
    fn next(&mut self) -> ObjectId {
        let id = (self.0, 0);
        self.0 += 1;
        id
    }
}

pub(crate) fn prepare(source: &Source, request: &Request<'_>, capacity: usize) -> Result<Prepared> {
    let fields = fields::all(source);
    check_permissions(source, &fields, request.details.lock)?;

    let mut ids = Allocator(source.next_id());
    let mut update = Update::default();
    let sig_id = ids.next();
    let mut catalog = source.catalog()?.clone();
    let mut catalog_changed = false;

    let new_widget = match request.target {
        Target::Field(name) => {
            sign_existing(
                source,
                &fields,
                name,
                sig_id,
                request.appearance,
                &mut ids,
                &mut update,
            )?;
            None
        }
        Target::Invisible => Some((0, [0.0; 4])),
        Target::NewField { page, rect } => {
            let frame = PageFrame::of(source, source.page_id(*page)?);
            Some((*page, frame.rect_to_user(normalized(*rect))))
        }
    };

    // Formulaire : /SigFlags 3 (signatures présentes, ajout seulement) et
    // nouveau champ dans /Fields.
    let (form_id, mut form) = match fields::acro_form(source) {
        Some((id, form)) => (id, form),
        None => (None, Dictionary::new()),
    };
    form.set("SigFlags", 3);
    if let Some((page, rect)) = new_widget {
        let page_id = source.page_id(page)?;
        let widget_id = ids.next();
        let name = unique_name(&fields);
        let mut widget = Dictionary::new();
        widget.set("Type", Object::Name(b"Annot".to_vec()));
        widget.set("Subtype", Object::Name(b"Widget".to_vec()));
        widget.set("FT", Object::Name(b"Sig".to_vec()));
        widget.set("T", lopdf::text_string(&name));
        widget.set("V", sig_id);
        // Imprimable (4) et verrouillé (128).
        widget.set("F", 132);
        widget.set(
            "Rect",
            rect.iter().map(|&v| Object::Real(v)).collect::<Vec<_>>(),
        );
        widget.set("P", page_id);
        add_appearance(&mut widget, rect, request.appearance, &mut ids, &mut update);
        update.push(widget_id, Body::Object(Object::Dictionary(widget)));
        add_to_array(source, page_id, "Annots", widget_id, &mut update)?;
        append_field(source, &mut form, widget_id, &mut update)?;
    }
    match form_id {
        Some(id) => update.push(id, Body::Object(Object::Dictionary(form))),
        None => {
            let id = ids.next();
            update.push(id, Body::Object(Object::Dictionary(form)));
            catalog.set("AcroForm", id);
            catalog_changed = true;
        }
    }

    if request.details.lock != Lock::None {
        set_doc_mdp(
            source,
            &mut catalog,
            sig_id,
            &mut update,
            &mut catalog_changed,
        )?;
    }
    if catalog_changed {
        update.push(source.root_id()?, Body::Object(Object::Dictionary(catalog)));
    }

    let sig = sig_dict::build(&request.details, capacity)?;
    update.push(sig_id, Body::Raw(sig.bytes.clone()));
    Ok(Prepared {
        update,
        sig_id,
        sig,
    })
}

/// Document verrouillé, ou certification demandée sur un document déjà signé.
fn check_permissions(source: &Source, fields: &[Field], lock: Lock) -> Result<()> {
    if let Some((_, 1)) = permissions::certification(source) {
        return Err(SignError::Locked);
    }
    let signed = fields.iter().any(|f| f.is_signature && f.value.is_some());
    if lock != Lock::None && signed {
        return Err(SignError::AlreadySigned);
    }
    Ok(())
}

fn sign_existing(
    source: &Source,
    fields: &[Field],
    name: &str,
    sig_id: ObjectId,
    appearance: &[String],
    ids: &mut Allocator,
    update: &mut Update,
) -> Result<()> {
    let field = fields
        .iter()
        .find(|f| f.is_signature && f.name == name)
        .ok_or_else(|| {
            SignError::Target(format!("le champ de signature « {name} » n'existe pas"))
        })?;
    if field.value.is_some() {
        return Err(SignError::Target(format!(
            "le champ « {name} » est déjà signé"
        )));
    }
    let field_id = field
        .id
        .ok_or_else(|| SignError::Target(format!("le champ « {name} » ne peut pas être signé")))?;
    let mut field_dict = source.document.get_dictionary(field_id)?.clone();
    field_dict.set("V", sig_id);
    let widget_id = field.widgets.first().copied();
    let (_, rect) = fields::placement(source, field);
    match widget_id {
        Some(widget_id) if widget_id == field_id => {
            if let Some(rect) = rect {
                add_appearance(&mut field_dict, normalized(rect), appearance, ids, update);
            }
        }
        Some(widget_id) => {
            if let Some(rect) = rect {
                let mut widget = source.document.get_dictionary(widget_id)?.clone();
                if add_appearance(&mut widget, normalized(rect), appearance, ids, update) {
                    update.push(widget_id, Body::Object(Object::Dictionary(widget)));
                }
            }
        }
        None => {}
    }
    update.push(field_id, Body::Object(Object::Dictionary(field_dict)));
    Ok(())
}

/// Apparence texte du widget si le cadre a une surface ; vrai si posée.
fn add_appearance(
    widget: &mut Dictionary,
    rect: [f32; 4],
    lines: &[String],
    ids: &mut Allocator,
    update: &mut Update,
) -> bool {
    let (width, height) = (rect[2] - rect[0], rect[3] - rect[1]);
    if width < 1.0 || height < 1.0 || lines.iter().all(|l| l.trim().is_empty()) {
        return false;
    }
    let font_id = ids.next();
    update.push(font_id, Body::Object(appearance::helvetica()));
    let stream_id = ids.next();
    let stream = appearance::text_appearance(lines, width, height, font_id);
    update.push(stream_id, Body::Object(Object::Stream(stream)));
    let mut ap = Dictionary::new();
    ap.set("N", stream_id);
    widget.set("AP", ap);
    true
}

/// Ajoute `item` au tableau `key` de l'objet `owner` (tableau direct,
/// indirect ou absent).
fn add_to_array(
    source: &Source,
    owner: ObjectId,
    key: &str,
    item: ObjectId,
    update: &mut Update,
) -> Result<()> {
    let mut dictionary = source.document.get_dictionary(owner)?.clone();
    match dictionary.get(key.as_bytes()).ok().cloned() {
        Some(Object::Reference(array_id)) => {
            let mut array = source.document.get_object(array_id)?.as_array()?.clone();
            array.push(item.into());
            update.push(array_id, Body::Object(Object::Array(array)));
        }
        Some(Object::Array(mut array)) => {
            array.push(item.into());
            dictionary.set(key, array);
            update.push(owner, Body::Object(Object::Dictionary(dictionary)));
        }
        _ => {
            dictionary.set(key, vec![Object::from(item)]);
            update.push(owner, Body::Object(Object::Dictionary(dictionary)));
        }
    }
    Ok(())
}

fn append_field(
    source: &Source,
    form: &mut Dictionary,
    field: ObjectId,
    update: &mut Update,
) -> Result<()> {
    match form.get(b"Fields").ok().cloned() {
        Some(Object::Reference(array_id)) => {
            let mut array = source.document.get_object(array_id)?.as_array()?.clone();
            array.push(field.into());
            update.push(array_id, Body::Object(Object::Array(array)));
        }
        Some(Object::Array(mut array)) => {
            array.push(field.into());
            form.set("Fields", array);
        }
        _ => form.set("Fields", vec![Object::from(field)]),
    }
    Ok(())
}

/// `/Perms << /DocMDP signature >>` dans le catalogue.
fn set_doc_mdp(
    source: &Source,
    catalog: &mut Dictionary,
    sig_id: ObjectId,
    update: &mut Update,
    catalog_changed: &mut bool,
) -> Result<()> {
    match catalog.get(b"Perms").ok().cloned() {
        Some(Object::Reference(perms_id)) => {
            let mut perms = source.document.get_dictionary(perms_id)?.clone();
            perms.set("DocMDP", sig_id);
            update.push(perms_id, Body::Object(Object::Dictionary(perms)));
        }
        Some(Object::Dictionary(mut perms)) => {
            perms.set("DocMDP", sig_id);
            catalog.set("Perms", perms);
            *catalog_changed = true;
        }
        _ => {
            let mut perms = Dictionary::new();
            perms.set("DocMDP", sig_id);
            catalog.set("Perms", perms);
            *catalog_changed = true;
        }
    }
    Ok(())
}

/// « Signature1 », « Signature2 »… sans reprendre un nom existant.
fn unique_name(fields: &[Field]) -> String {
    (1..)
        .map(|n| format!("Signature{n}"))
        .find(|candidate| fields.iter().all(|f| &f.name != candidate))
        .unwrap_or_else(|| "Signature".into())
}

/// Coins dans l'ordre (bas gauche, haut droit).
fn normalized(rect: [f32; 4]) -> [f32; 4] {
    [
        rect[0].min(rect[2]),
        rect[1].min(rect[3]),
        rect[0].max(rect[2]),
        rect[1].max(rect[3]),
    ]
}
