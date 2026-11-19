//! Modifications du document : éléments ajoutés (textes, images,
//! annotations) et plan des pages (ordre, pages blanches, rotations).
//!
//! Le PDF d'origine n'est jamais modifié : le plan des pages et la liste des
//! éléments sont appliqués par le moteur à une copie du document à chaque
//! changement.
//! L'historique garde chaque opération et son inverse (annuler, rétablir).

mod annotation;
mod detect;
mod element;
mod existing;
mod field_box;
mod form;
mod form_design;
mod form_ops;
mod history;
mod image;
mod import;
mod layout;
mod note;
mod original_text;
mod page_ops;
mod pages;
mod redact;
mod rotate;

pub use annotation::{
    Area, FILL_OPACITY, InkBox, MAX_INK_POINTS, MAX_MARKUP_AREAS, MarkKind, MarkupBox,
    STROKE_WIDTH_RANGE, ShapeBox, ShapeKind, ellipse_curves,
};
pub use detect::{Suggestion, detect_fields, kind_for_label, name_for_label};
pub use element::{
    Align, Color, Element, ElementId, FONT_SIZE_RANGE, MAX_TEXT_CHARS, MIN_WIDTH, StandardFont,
    TextBox, TextStyle,
};
pub use existing::{ExistingImage, MIN_EXISTING_SIDE};
pub use field_box::{
    FieldBox, FieldOrigin, MAX_EXPORT_CHARS, MAX_FIELD_NAME_CHARS, MIN_COMB_CELL, MIN_FIELD_SIDE,
};
pub use form::{
    FIELD_FONT_RANGE, FieldKind, FieldLook, FieldValue, FormField, MAX_FIELD_CHARS, MAX_OPTIONS,
    MAX_TOOLTIP_CHARS, Metadata, Widget,
};
pub use form_design::designed_fields;
pub use history::{Change, Edits, Operation};
pub use image::{
    ImageAsset, ImageBox, ImageData, ImageId, Images, MIN_IMAGE_SIDE, OPACITY_RANGE, Placement,
};
pub use import::{ImportId, ImportedPdf, Imports, PageSources};
pub use layout::{leading, wrap};
pub use note::{MAX_AUTHOR_CHARS, MAX_NOTE_CHARS, NOTE_SIZE, NoteBox};
pub use original_text::{MAX_BLOCK_CHARS, MAX_BLOCK_OBJECTS, OriginalText};
pub use pages::{BLANK_SIDE_RANGE, MAX_PAGES, PagePlan, PageSource, PlannedPage, check_blank_size};
pub use redact::{MIN_REDACT_SIDE, RedactBox};
pub use rotate::{can_rotate, rotated};
