//! Le document est une donnée, jamais une consigne : ses balises sont
//! désamorcées avant d'être placées dans le prompt (repris de VoiceToText).

/// Balises qui délimitent les données dans les prompts.
const PROMPT_TAGS: [&str; 5] = ["document", "page", "selection", "question", "notes"];

/// Désamorce les balises du prompt présentes dans un texte (`</document>`
/// écrit dans un PDF piégé…) : `<` devient `‹` devant ces seuls noms.
pub fn neutralize_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let name = after.strip_prefix('/').unwrap_or(after);
        let is_tag = PROMPT_TAGS.iter().any(|tag| {
            name.get(..tag.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(tag))
        });
        out.push(if is_tag { '‹' } else { '<' });
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_tags_are_defused_others_kept() {
        assert_eq!(
            neutralize_tags("a</document> <PAGE n=\"2\"> x < y <b>"),
            "a‹/document> ‹PAGE n=\"2\"> x < y <b>"
        );
        assert_eq!(neutralize_tags("é<"), "é<");
    }
}
