//! Conversions des types et erreurs de PDFium vers ceux de `pde-core`.

use pde_core::engine::EngineError;
use pde_core::geometry::{Point, Rect};
use pdfium_render::prelude::{PdfRect, PdfiumError, PdfiumInternalError};

/// Erreur d'ouverture d'un document, selon qu'un mot de passe a été fourni.
pub(crate) fn open_error(error: PdfiumError, password_given: bool) -> EngineError {
    match error {
        PdfiumError::PdfiumLibraryInternalError(internal) => match internal {
            PdfiumInternalError::PasswordError if password_given => EngineError::WrongPassword,
            PdfiumInternalError::PasswordError => EngineError::PasswordRequired,
            PdfiumInternalError::FormatError => {
                EngineError::InvalidFile("structure du fichier non reconnue".into())
            }
            PdfiumInternalError::FileError => EngineError::InvalidFile("contenu illisible".into()),
            PdfiumInternalError::SecurityError => {
                EngineError::InvalidFile("protection du document non prise en charge".into())
            }
            other => EngineError::Internal(format!("{other:?}")),
        },
        other => internal(other),
    }
}

/// Erreur imprévue de PDFium.
pub(crate) fn internal(error: PdfiumError) -> EngineError {
    EngineError::Internal(format!("{error:?}"))
}

/// Rectangle PDFium (points, origine en bas à gauche) vers rectangle du domaine.
pub(crate) fn rect(r: PdfRect) -> Rect {
    Rect::from_corners(
        Point::new(f64::from(r.left().value), f64::from(r.bottom().value)),
        Point::new(f64::from(r.right().value), f64::from(r.top().value)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pdfium(internal: PdfiumInternalError) -> PdfiumError {
        PdfiumError::PdfiumLibraryInternalError(internal)
    }

    #[test]
    fn password_error_depends_on_given_password() {
        assert_eq!(
            open_error(pdfium(PdfiumInternalError::PasswordError), false),
            EngineError::PasswordRequired
        );
        assert_eq!(
            open_error(pdfium(PdfiumInternalError::PasswordError), true),
            EngineError::WrongPassword
        );
    }

    #[test]
    fn format_and_file_errors_mean_invalid_file() {
        for error in [
            PdfiumInternalError::FormatError,
            PdfiumInternalError::FileError,
            PdfiumInternalError::SecurityError,
        ] {
            assert!(matches!(
                open_error(pdfium(error), false),
                EngineError::InvalidFile(_)
            ));
        }
    }

    #[test]
    fn rect_keeps_pdf_coordinates() {
        let r = rect(PdfRect::new_from_values(10.0, 20.0, 30.0, 60.0));
        assert_eq!(r.min(), Point::new(20.0, 10.0));
        assert_eq!(r.max(), Point::new(60.0, 30.0));
    }
}
