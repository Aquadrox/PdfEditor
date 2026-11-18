//! Les actions de l'éditeur, rubrique par rubrique.

use pde_core::settings::Action as Shortcut;
use serde_json::{Value, json};

use super::{ActionInfo, Mode};

const FILE: &str = "Fichier";
const VIEW: &str = "Affichage";
const EDIT: &str = "Éditer";
const PAGES: &str = "Pages";
const SIGN: &str = "Signer";
const AI: &str = "IA";

/// Action sans paramètres, valable partout ; les champs restants se
/// précisent avec les méthodes ci-dessous.
fn action(
    id: &'static str,
    group: &'static str,
    title: &'static str,
    help: &'static str,
    tool: &'static str,
) -> ActionInfo {
    ActionInfo {
        id,
        group,
        title,
        help,
        keywords: &[],
        mode: None,
        tool,
        needs_document: true,
        shortcut: None,
        params: object(json!({}), &[]),
    }
}

impl ActionInfo {
    fn keywords(mut self, keywords: &'static [&'static str]) -> Self {
        self.keywords = keywords;
        self
    }
    fn mode(mut self, mode: Mode) -> Self {
        self.mode = Some(mode);
        self
    }
    fn shortcut(mut self, shortcut: Shortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }
    fn anytime(mut self) -> Self {
        self.needs_document = false;
        self
    }
    fn params(mut self, params: Value) -> Self {
        self.params = params;
        self
    }
}

/// Schéma d'un objet aux propriétés données, sans propriétés en plus.
fn object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn pages_param(what: &str) -> Value {
    json!({
        "type": "array",
        "items": { "type": "integer", "minimum": 1 },
        "minItems": 1,
        "description": format!("Numéros des pages {what}, à partir de 1"),
    })
}

/// Texte visé, cherché dans le document : jamais de coordonnées.
fn text_param(what: &str) -> Value {
    json!({
        "type": "string",
        "minLength": 1,
        "description": format!("Texte exact {what}, recopié tel qu'il figure dans le document"),
    })
}

fn page_param() -> Value {
    json!({ "type": "integer", "minimum": 1, "description": "Numéro de la page, à partir de 1" })
}

/// Zone d'une page en points PDF (1/72 de pouce), origine en haut à gauche.
fn rect_param(what: &str) -> Value {
    let number = json!({ "type": "number" });
    json!({
        "type": "object",
        "properties": { "x": number, "y": number, "width": number, "height": number },
        "required": ["x", "y", "width", "height"],
        "additionalProperties": false,
        "description": format!("{what}, en points depuis le coin haut gauche de la page"),
    })
}

pub(super) fn all() -> Vec<ActionInfo> {
    let mut actions = file();
    actions.extend(view());
    actions.extend(edit());
    actions.extend(pages());
    actions.extend(sign());
    actions.extend(ai());
    actions
}

fn file() -> Vec<ActionInfo> {
    vec![
        action(
            "open",
            FILE,
            "Ouvrir un PDF",
            "Ouvrir un fichier PDF depuis le disque ; vous pouvez aussi le déposer sur la fenêtre.",
            "#menu-open",
        )
        .keywords(&["fichier", "charger", "document"])
        .shortcut(Shortcut::Open)
        .anytime(),
        action(
            "save",
            FILE,
            "Enregistrer",
            "Enregistrer les modifications dans le fichier ouvert.",
            "#menu-save",
        )
        .keywords(&["sauvegarder", "sauver"])
        .shortcut(Shortcut::Save),
        action(
            "save_as",
            FILE,
            "Enregistrer sous",
            "Enregistrer le document modifié dans un nouveau fichier ; l'original reste intact.",
            "#menu-save-as",
        )
        .keywords(&["copie", "exporter", "nouveau fichier"])
        .shortcut(Shortcut::SaveAs),
        action(
            "close",
            FILE,
            "Fermer le document",
            "Fermer le document affiché ; PdfEditor propose d'enregistrer s'il a changé.",
            "#menu-close",
        )
        .shortcut(Shortcut::Close),
        action(
            "properties",
            FILE,
            "Propriétés du document",
            "Voir et modifier le titre, l'auteur, le sujet et les mots-clés du document.",
            "#menu-properties",
        )
        .keywords(&["titre", "auteur", "métadonnées", "informations"]),
        action(
            "flatten_form",
            FILE,
            "Aplatir le formulaire",
            "Figer les champs remplis dans la page : le formulaire n'est plus modifiable.",
            "#menu-flatten",
        )
        .keywords(&["formulaire", "champs", "figer"]),
        action(
            "settings",
            FILE,
            "Réglages",
            "Ouvrir les réglages : thème, zoom, raccourcis, modifications, signature, IA.",
            "#menu-settings",
        )
        .keywords(&[
            "préférences",
            "options",
            "paramètres",
            "raccourcis",
            "thème",
        ])
        .shortcut(Shortcut::Settings)
        .anytime(),
    ]
}

fn view() -> Vec<ActionInfo> {
    vec![
        action("mode_read", VIEW, "Mode Lire", "Lire le document et remplir ses formulaires.", "#mode-read")
            .keywords(&["lecture", "formulaire", "remplir"]),
        action("search", VIEW, "Rechercher", "Chercher un mot ou une expression dans le document.", "#search-button")
            .keywords(&["trouver", "chercher", "texte"])
            .shortcut(Shortcut::Search)
            .params(object(
                json!({ "query": { "type": "string", "minLength": 1, "description": "Texte à chercher" } }),
                &["query"],
            )),
        action("go_to_page", VIEW, "Aller à la page", "Afficher une page donnée : tapez son numéro dans la barre d'état.", "#page-input")
            .keywords(&["page", "naviguer", "numéro"])
            .params(object(json!({ "page": page_param() }), &["page"])),
        action("zoom_in", VIEW, "Zoom avant", "Agrandir l'affichage des pages.", "#zoom-in")
            .keywords(&["agrandir", "grossir"])
            .shortcut(Shortcut::ZoomIn),
        action("zoom_out", VIEW, "Zoom arrière", "Réduire l'affichage des pages.", "#zoom-out")
            .keywords(&["réduire", "rapetisser"])
            .shortcut(Shortcut::ZoomOut),
        action("zoom_reset", VIEW, "Taille réelle", "Afficher les pages à 100 %.", "#zoom-value")
            .keywords(&["100", "zoom"])
            .shortcut(Shortcut::ZoomReset),
        action("fit_page", VIEW, "Page entière", "Ajuster le zoom pour voir la page entière.", "#fit-page")
            .keywords(&["ajuster", "zoom"])
            .shortcut(Shortcut::FitPage),
        action("fit_width", VIEW, "Pleine largeur", "Ajuster le zoom à la largeur de la fenêtre.", "#fit-width")
            .keywords(&["ajuster", "zoom", "largeur"])
            .shortcut(Shortcut::FitWidth),
        action("show_thumbnails", VIEW, "Miniatures des pages", "Afficher les miniatures des pages dans le volet de gauche.", "#tab-pages")
            .keywords(&["vignettes", "aperçu", "volet"]),
        action("show_outline", VIEW, "Plan du document", "Afficher le plan (signets) du document dans le volet de gauche.", "#tab-outline")
            .keywords(&["signets", "sommaire", "table des matières"]),
    ]
}

fn edit() -> Vec<ActionInfo> {
    vec![
        action("mode_edit", EDIT, "Mode Éditer", "Ajouter du texte, des images, des annotations, ou caviarder.", "#mode-edit")
            .keywords(&["modifier", "éditer"]),
        action("undo", EDIT, "Annuler", "Annuler la dernière modification.", "#edit-undo")
            .mode(Mode::Edit)
            .keywords(&["défaire", "revenir"])
            .shortcut(Shortcut::Undo),
        action("redo", EDIT, "Rétablir", "Refaire la modification annulée.", "#edit-redo")
            .mode(Mode::Edit)
            .keywords(&["refaire"])
            .shortcut(Shortcut::Redo),
        action("add_text", EDIT, "Ajouter du texte", "Écrire un nouveau texte sur la page : cliquez où il doit commencer.", "#tool-text")
            .mode(Mode::Edit)
            .keywords(&["écrire", "taper", "zone de texte"]),
        action("edit_text", EDIT, "Modifier le texte du document", "Corriger une ligne de texte existante : cliquez dessus et tapez.", "#tool-retext")
            .mode(Mode::Edit)
            .keywords(&["corriger", "remplacer", "faute", "coquille"]),
        action("add_image", EDIT, "Ajouter une image", "Poser une image (PNG, JPEG, WebP) sur la page ; vous pouvez aussi la coller ou la déposer.", "#tool-image")
            .mode(Mode::Edit)
            .keywords(&["photo", "logo", "tampon", "insérer"]),
        action("highlight", EDIT, "Surligner, souligner, barrer", "Marquer du texte : sélectionnez-le avec l'outil Surligner, puis choisissez surligner, souligner ou barrer.", "#tool-markup")
            .mode(Mode::Edit)
            .keywords(&["surligneur", "fluo", "souligner", "barrer", "marquer"])
            .params(object(
                json!({
                    "text": text_param("à marquer"),
                    "pages": pages_param("où chercher (toutes celles de la portée si absent)"),
                    "style": { "type": "string", "enum": ["highlight", "underline", "strike_out"], "description": "Surligner, souligner ou barrer" },
                }),
                &["text"],
            )),
        action("draw", EDIT, "Dessiner", "Dessiner à main levée sur la page.", "#tool-ink")
            .mode(Mode::Edit)
            .keywords(&["crayon", "stylo", "main levée", "griffonner"]),
        action("add_shape", EDIT, "Rectangle, ellipse, flèche", "Tracer une forme ; un rectangle plein masque une zone, mais le contenu reste dans le fichier : pour le supprimer, caviardez.", "#tool-shape")
            .mode(Mode::Edit)
            .keywords(&["forme", "cadre", "flèche", "trait", "masquer", "cacher", "entourer"])
            .params(object(
                json!({
                    "page": page_param(),
                    "area": rect_param("Zone de la forme"),
                    "shape": { "type": "string", "enum": ["rectangle", "ellipse", "line", "arrow"], "description": "Forme à tracer" },
                    "filled": { "type": "boolean", "description": "Forme pleine (masque ce qu'elle couvre, sans le supprimer)" },
                }),
                &["page", "area", "shape"],
            )),
        action("add_note", EDIT, "Ajouter une note", "Poser un commentaire sur la page, que tous les lecteurs PDF affichent.", "#tool-note")
            .mode(Mode::Edit)
            .keywords(&["commentaire", "annotation", "post-it", "remarque"])
            .params(object(
                json!({
                    "page": page_param(),
                    "text": { "type": "string", "minLength": 1, "description": "Texte de la note" },
                    "near": { "type": "string", "description": "Texte de la page près duquel poser la note (en haut de la page sinon)" },
                }),
                &["page", "text"],
            )),
        action("redact", EDIT, "Caviarder", "Supprimer réellement du texte ou une zone du document : contrairement à un masque, rien ne reste dans le fichier.", "#tool-redact")
            .mode(Mode::Edit)
            .keywords(&["anonymiser", "masquer", "cacher", "effacer", "noircir", "confidentiel", "rgpd"])
            .params(object(
                json!({
                    "text": text_param("à supprimer de chaque page"),
                    "pages": pages_param("où chercher (toutes celles de la portée si absent)"),
                }),
                &["text"],
            )),
    ]
}

fn pages() -> Vec<ActionInfo> {
    vec![
        action("mode_pages", PAGES, "Mode Pages", "Réorganiser les pages : déplacer, pivoter, supprimer, insérer, extraire.", "#mode-pages")
            .keywords(&["organiser", "ordre", "grille"]),
        action("insert_blank_page", PAGES, "Insérer une page blanche", "Ajouter une page blanche après les pages choisies.", "#pages-insert")
            .mode(Mode::Pages)
            .keywords(&["ajouter", "page vide"])
            .params(object(
                json!({ "after": { "type": "integer", "minimum": 0, "description": "Numéro de la page après laquelle insérer (0 : au début)" } }),
                &["after"],
            )),
        action("insert_pdf", PAGES, "Insérer un PDF", "Ajouter les pages d'autres PDF après les pages choisies ; vous pouvez aussi les glisser sur la grille.", "#pages-insert-pdf")
            .mode(Mode::Pages)
            .keywords(&["fusionner", "combiner", "assembler", "joindre", "ajouter"]),
        action("duplicate_pages", PAGES, "Dupliquer des pages", "Copier les pages choisies, avec ce que vous y avez ajouté.", "#pages-duplicate")
            .mode(Mode::Pages)
            .keywords(&["copier"])
            .params(object(json!({ "pages": pages_param("à copier") }), &["pages"])),
        action("rotate_pages", PAGES, "Pivoter des pages", "Tourner les pages choisies d'un quart de tour, à gauche ou à droite.", "#pages-rotate-right")
            .mode(Mode::Pages)
            .keywords(&["tourner", "rotation", "orientation", "paysage", "portrait"])
            .params(object(
                json!({
                    "pages": pages_param("à tourner"),
                    "direction": { "type": "string", "enum": ["left", "right"], "description": "Sens du quart de tour" },
                }),
                &["pages", "direction"],
            )),
        action("extract_pages", PAGES, "Extraire des pages", "Enregistrer les pages choisies dans un nouveau PDF.", "#pages-extract")
            .mode(Mode::Pages)
            .keywords(&["séparer", "diviser", "découper", "scinder", "exporter"])
            .params(object(json!({ "pages": pages_param("à extraire") }), &["pages"])),
        action("remove_pages", PAGES, "Supprimer des pages", "Retirer les pages choisies du document (Suppr dans la grille).", "#pages-remove")
            .mode(Mode::Pages)
            .keywords(&["effacer", "enlever", "retirer"])
            .params(object(json!({ "pages": pages_param("à supprimer") }), &["pages"])),
    ]
}

fn sign() -> Vec<ActionInfo> {
    vec![
        action(
            "mode_sign",
            SIGN,
            "Mode Signer",
            "Signer le document et vérifier ses signatures.",
            "#mode-sign",
        )
        .keywords(&["signature", "vérifier"]),
        action(
            "sign",
            SIGN,
            "Signer le document",
            "Apposer une signature électronique, avec ou sans apparence sur la page.",
            "#sign-start",
        )
        .mode(Mode::Sign)
        .keywords(&["signature", "parapher", "certificat", "pades"]),
        action(
            "place_signature",
            SIGN,
            "Placer la signature",
            "Tracer sur la page le cadre où la signature apparaîtra.",
            "#sign-place",
        )
        .mode(Mode::Sign)
        .keywords(&["cadre", "champ de signature", "emplacement"]),
        action(
            "identities",
            SIGN,
            "Identités de signature",
            "Créer ou importer l'identité (certificat) qui signe vos documents.",
            "#sign-identities",
        )
        .mode(Mode::Sign)
        .keywords(&["certificat", "p12", "pfx", "clé"]),
    ]
}

fn ai() -> Vec<ActionInfo> {
    let task = |id, title, help, tool, keywords| {
        action(id, AI, title, help, tool)
            .mode(Mode::Ai)
            .keywords(keywords)
    };
    vec![
        action(
            "mode_ai",
            AI,
            "Mode IA",
            "Analyser le document ou lui poser des questions avec l'assistant IA.",
            "#mode-ai",
        )
        .keywords(&["assistant", "intelligence artificielle"]),
        task(
            "ai_summary",
            "Résumer",
            "Résumer la sélection, des pages ou le document avec l'IA.",
            "[data-task=\"summary\"]",
            &["résumé", "synthèse"],
        ),
        task(
            "ai_key_points",
            "Points clés",
            "Lister les points clés de la sélection, des pages ou du document.",
            "[data-task=\"key_points\"]",
            &["essentiel", "idées"],
        ),
        task(
            "ai_entities",
            "Entités",
            "Relever personnes, organisations, dates et montants cités.",
            "[data-task=\"entities\"]",
            &["noms", "dates", "montants"],
        ),
        task(
            "ai_risks",
            "Risques et clauses",
            "Repérer les risques et les clauses à surveiller, par exemple dans un contrat.",
            "[data-task=\"risks\"]",
            &["contrat", "clauses", "juridique"],
        ),
        task(
            "ai_question",
            "Poser une question",
            "Poser une question sur le document ; la réponse cite ses pages.",
            "#ai-question",
            &["question", "demander"],
        ),
    ]
}
