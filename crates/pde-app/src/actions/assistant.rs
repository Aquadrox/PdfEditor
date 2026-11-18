//! Actions de l'assistant (IA-15) : outils proposés au modèle, tirés du
//! catalogue, et propositions qu'il en tire, vérifiées avant tout aperçu
//! (outil permis, arguments au bon format, pages dans la portée choisie).
//! Le texte visé est cherché dans le document par la visionneuse : le
//! modèle ne donne jamais de coordonnées.

use pde_core::ai::{ToolCall, ToolSpec};
use pde_core::edit::MarkKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::catalog;

/// Actions que l'assistant peut proposer.
pub const ASSISTANT_ACTIONS: [&str; 5] = [
    "redact",
    "highlight",
    "add_note",
    "rotate_pages",
    "remove_pages",
];

/// Plus long texte visé ou note acceptés d'un modèle.
const MAX_TEXT: usize = 2_000;

/// Sens d'un quart de tour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Left,
    Right,
}

/// Action proposée par l'assistant ; pages numérotées à partir de 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Proposal {
    /// Caviarder chaque occurrence du texte (pages vides : toute la portée).
    Redact {
        text: String,
        #[serde(default)]
        pages: Vec<usize>,
    },
    Highlight {
        text: String,
        #[serde(default)]
        pages: Vec<usize>,
        #[serde(default)]
        style: Option<MarkKind>,
    },
    /// Note sur une page, près du texte `near` s'il est trouvé.
    AddNote {
        page: usize,
        text: String,
        #[serde(default)]
        near: Option<String>,
    },
    RotatePages {
        pages: Vec<usize>,
        direction: Direction,
    },
    RemovePages {
        pages: Vec<usize>,
    },
}

/// Outils offerts au modèle : les actions permises du catalogue, avec leur
/// explication et leurs paramètres.
pub fn tools(allowed: &[&str]) -> Vec<ToolSpec> {
    catalog()
        .into_iter()
        .filter(|a| allowed.contains(&a.id) && ASSISTANT_ACTIONS.contains(&a.id))
        .map(|a| ToolSpec {
            name: a.id.to_owned(),
            description: format!("{} : {}", a.title, a.help),
            parameters: a.params.to_string(),
        })
        .collect()
}

/// Proposition tirée d'un appel d'outil, ou la raison de son refus.
pub fn parse(call: &ToolCall, allowed: &[&str]) -> Result<Proposal, String> {
    let name = call.name.trim();
    if !allowed.contains(&name) || !ASSISTANT_ACTIONS.contains(&name) {
        return Err(format!("action « {name} » non permise"));
    }
    let arguments = if call.arguments.trim().is_empty() {
        Value::Object(Default::default())
    } else {
        serde_json::from_str(&call.arguments)
            .map_err(|_| format!("« {name} » : arguments illisibles"))?
    };
    let Value::Object(mut map) = arguments else {
        return Err(format!("« {name} » : arguments illisibles"));
    };
    map.insert("action".into(), Value::from(name));
    let proposal: Proposal = serde_json::from_value(Value::Object(map))
        .map_err(|e| format!("« {name} » : arguments invalides ({e})"))?;
    proposal.validate()?;
    Ok(proposal)
}

impl Proposal {
    fn validate(&self) -> Result<(), String> {
        let text_ok = |t: &str| !t.trim().is_empty() && t.chars().count() <= MAX_TEXT;
        let ok = match self {
            Proposal::Redact { text, .. } | Proposal::Highlight { text, .. } => text_ok(text),
            Proposal::AddNote { text, near, .. } => {
                text_ok(text)
                    && near
                        .as_deref()
                        .is_none_or(|n| n.chars().count() <= MAX_TEXT)
            }
            Proposal::RotatePages { pages, .. } | Proposal::RemovePages { pages } => {
                !pages.is_empty()
            }
        };
        if ok {
            Ok(())
        } else {
            Err(format!("{} : texte ou pages manquants", self.summary()))
        }
    }

    /// Pages nommées par la proposition (à partir de 1).
    fn named_pages(&self) -> Vec<usize> {
        match self {
            Proposal::Redact { pages, .. }
            | Proposal::Highlight { pages, .. }
            | Proposal::RotatePages { pages, .. }
            | Proposal::RemovePages { pages } => pages.clone(),
            Proposal::AddNote { page, .. } => vec![*page],
        }
    }

    /// Pages visées (rangs à partir de 0), toutes dans la portée `scope` ;
    /// une recherche sans pages couvre toute la portée.
    pub fn pages_in(&self, scope: &[usize]) -> Result<Vec<usize>, String> {
        let named = self.named_pages();
        if named.is_empty() {
            return Ok(scope.to_vec());
        }
        let mut pages = Vec::with_capacity(named.len());
        for page in named {
            let index = page.checked_sub(1).filter(|i| scope.contains(i));
            let Some(index) = index else {
                return Err(format!(
                    "{} : page {page} hors de la portée choisie",
                    self.summary()
                ));
            };
            if !pages.contains(&index) {
                pages.push(index);
            }
        }
        Ok(pages)
    }

    /// La proposition supprime réellement du contenu (confirmation à part).
    pub fn is_redaction(&self) -> bool {
        matches!(self, Proposal::Redact { .. })
    }

    /// En mots d'utilisateur, pour la liste des propositions.
    pub fn summary(&self) -> String {
        let on_pages = |pages: &[usize]| {
            if pages.is_empty() {
                String::new()
            } else {
                let list: Vec<String> = pages.iter().map(usize::to_string).collect();
                format!(" (p. {})", list.join(", "))
            }
        };
        match self {
            Proposal::Redact { text, pages } => {
                format!("Caviarder « {} »{}", short(text), on_pages(pages))
            }
            Proposal::Highlight { text, pages, style } => {
                let verb = match style {
                    Some(MarkKind::Underline) => "Souligner",
                    Some(MarkKind::StrikeOut) => "Barrer",
                    _ => "Surligner",
                };
                format!("{verb} « {} »{}", short(text), on_pages(pages))
            }
            Proposal::AddNote { page, text, .. } => format!("Note p. {page} : « {} »", short(text)),
            Proposal::RotatePages {
                pages: list,
                direction,
            } => {
                let side = match direction {
                    Direction::Left => "à gauche",
                    Direction::Right => "à droite",
                };
                format!("Pivoter{} {side}", on_pages(list))
            }
            Proposal::RemovePages { pages: list } => format!("Supprimer{}", on_pages(list)),
        }
    }
}

/// Début d'un texte, pour un résumé d'une ligne.
fn short(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= 60 {
        text.to_owned()
    } else {
        format!("{}…", text.chars().take(57).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str, arguments: &str) -> ToolCall {
        ToolCall {
            name: name.into(),
            arguments: arguments.into(),
        }
    }

    #[test]
    fn every_assistant_action_is_a_catalogued_tool() {
        let tools = tools(&ASSISTANT_ACTIONS);
        assert_eq!(tools.len(), ASSISTANT_ACTIONS.len());
        for tool in &tools {
            let schema: Value = serde_json::from_str(&tool.parameters).unwrap();
            assert_eq!(schema["type"], "object", "{}", tool.name);
            assert!(tool.description.len() > 20);
        }
        // Une recette limite les outils offerts.
        assert_eq!(tools_names(&["redact", "settings"]), ["redact"]);
    }

    fn tools_names(allowed: &[&str]) -> Vec<String> {
        tools(allowed).into_iter().map(|t| t.name).collect()
    }

    #[test]
    fn calls_become_checked_proposals() {
        let all = &ASSISTANT_ACTIONS;
        assert_eq!(
            parse(
                &call("redact", r#"{"text":"Jean Dupont","pages":[2]}"#),
                all
            )
            .unwrap(),
            Proposal::Redact {
                text: "Jean Dupont".into(),
                pages: vec![2]
            }
        );
        assert_eq!(
            parse(
                &call("rotate_pages", r#"{"pages":[1,3],"direction":"left"}"#),
                all
            )
            .unwrap()
            .summary(),
            "Pivoter (p. 1, 3) à gauche"
        );
        let note = parse(
            &call(
                "add_note",
                r#"{"page":1,"text":"À vérifier","near":"Total"}"#,
            ),
            all,
        );
        assert!(matches!(note, Ok(Proposal::AddNote { page: 1, .. })));
    }

    #[test]
    fn invalid_or_unallowed_calls_are_refused() {
        let all = &ASSISTANT_ACTIONS;
        for (name, arguments) in [
            ("settings", "{}"),
            ("delete_everything", "{}"),
            ("redact", "pas du JSON"),
            ("redact", "[1]"),
            ("redact", r#"{"text":"   "}"#),
            ("rotate_pages", r#"{"pages":[],"direction":"left"}"#),
            ("rotate_pages", r#"{"pages":[1],"direction":"haut"}"#),
            ("remove_pages", r#"{"pages":"tout"}"#),
        ] {
            assert!(
                parse(&call(name, arguments), all).is_err(),
                "{name} {arguments}"
            );
        }
        // Permis par l'assistant mais pas par la recette.
        assert!(parse(&call("remove_pages", r#"{"pages":[1]}"#), &["redact"]).is_err());
    }

    #[test]
    fn pages_must_stay_in_the_chosen_scope() {
        let remove = Proposal::RemovePages { pages: vec![1, 3] };
        assert_eq!(remove.pages_in(&[0, 1, 2]).unwrap(), [0, 2]);
        assert!(
            remove
                .pages_in(&[0, 1])
                .unwrap_err()
                .contains("page 3 hors de la portée")
        );
        assert!(
            Proposal::RemovePages { pages: vec![0] }
                .pages_in(&[0])
                .is_err()
        );
        let redact = Proposal::Redact {
            text: "x".into(),
            pages: vec![],
        };
        assert_eq!(redact.pages_in(&[4, 5]).unwrap(), [4, 5], "toute la portée");
    }
}
