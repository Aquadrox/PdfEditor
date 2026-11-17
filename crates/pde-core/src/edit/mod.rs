//! Modifications du document (v0.2 : éléments ajoutés, textes, images et
//! annotations).
//!
//! Le PDF d'origine n'est jamais modifié : les éléments ajoutés forment une
//! liste, posée par le moteur sur une copie du document à chaque changement.
//! L'historique garde chaque opération et son inverse (annuler, rétablir).

mod annotation;
mod element;
mod history;
mod image;
mod layout;

pub use annotation::{
    Area, FILL_OPACITY, InkBox, MAX_INK_POINTS, MAX_MARKUP_AREAS, MarkKind, MarkupBox,
    STROKE_WIDTH_RANGE, ShapeBox, ShapeKind, ellipse_curves,
};
pub use element::{Align, Color, Element, ElementId, StandardFont, TextBox, TextStyle};
pub use history::{Change, Edits, Operation};
pub use image::{
    ImageAsset, ImageBox, ImageData, ImageId, Images, MIN_IMAGE_SIDE, OPACITY_RANGE, Placement,
};
pub use layout::{leading, wrap};
