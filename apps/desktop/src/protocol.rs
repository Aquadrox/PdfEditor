//! Protocole `pde://` : images des pages pour l'interface.
//!
//! Adresse : `pde://localhost/page/{document}/{page}?scale={pixels par point}`
//! (sous Windows, le WebView l'écrit `http://pde.localhost/page/…`).

use pde_app::viewer::Viewer;
use tauri::http::{Request, Response, StatusCode, header};

/// Demande d'image de page, extraite de l'adresse.
#[derive(Debug, PartialEq)]
pub struct PageRequest {
    pub doc: u64,
    pub page: usize,
    pub scale: f64,
}

/// Analyse le chemin (`/page/3/0`) et les paramètres (`scale=1.5`).
pub fn parse(path: &str, query: Option<&str>) -> Result<PageRequest, String> {
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    let ["page", doc, page] = parts.as_slice() else {
        return Err(format!("adresse inconnue : {path}"));
    };
    let doc = doc
        .parse()
        .map_err(|_| format!("document invalide : {doc}"))?;
    let page = page
        .parse()
        .map_err(|_| format!("page invalide : {page}"))?;
    let scale = query
        .unwrap_or_default()
        .split('&')
        .find_map(|pair| pair.strip_prefix("scale="))
        .ok_or("paramètre scale manquant")?;
    let scale = scale
        .parse()
        .map_err(|_| format!("zoom invalide : {scale}"))?;
    Ok(PageRequest { doc, page, scale })
}

pub fn respond(viewer: &Viewer, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let uri = request.uri();
    let result = parse(uri.path(), uri.query())
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
        .and_then(|r| {
            viewer
                .page_png(r.doc, r.page, r.scale)
                .map_err(|e| (StatusCode::NOT_FOUND, e.message))
        });
    let builder = Response::builder()
        // Les numéros de documents repartent de 1 à chaque lancement : le
        // WebView ne doit rien garder d'une session à l'autre.
        .header(header::CACHE_CONTROL, "no-store");
    let response = match result {
        Ok(png) => builder
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/png")
            .body(png.as_ref().clone()),
        Err((status, message)) => builder
            .status(status)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(message.into_bytes()),
    };
    response.unwrap_or_else(|_| Response::new(Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_page_requests() {
        assert_eq!(
            parse("/page/3/12", Some("scale=1.5")),
            Ok(PageRequest {
                doc: 3,
                page: 12,
                scale: 1.5
            })
        );
        assert_eq!(
            parse("/page/1/0/", Some("v=2&scale=0.25")).map(|r| r.scale),
            Ok(0.25)
        );
    }

    #[test]
    fn rejects_malformed_addresses() {
        for (path, query) in [
            ("/autre/1/0", Some("scale=1")),
            ("/page/1", Some("scale=1")),
            ("/page/x/0", Some("scale=1")),
            ("/page/1/-1", Some("scale=1")),
            ("/page/1/0", None),
            ("/page/1/0", Some("scale=abc")),
        ] {
            assert!(parse(path, query).is_err(), "{path} {query:?}");
        }
    }
}
