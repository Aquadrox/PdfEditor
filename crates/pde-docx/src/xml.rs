//! Lecteur XML réduit à ce qu'écrit Word : balises ouvrantes, fermantes et
//! vides avec leurs attributs, texte, entités standard et numériques.
//! Commentaires, instructions et déclarations sont sautés. Un document mal
//! formé s'arrête simplement là où il cesse d'être lisible.

/// Événement du document.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Balise ouvrante (`empty` : `<a/>`), nom complet avec préfixe.
    Start {
        name: String,
        attrs: Vec<(String, String)>,
        empty: bool,
    },
    End(String),
    Text(String),
}

impl Event {
    /// Valeur de l'attribut `key` d'une balise ouvrante.
    pub fn attr(&self, key: &str) -> Option<&str> {
        match self {
            Event::Start { attrs, .. } => attrs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.as_str()),
            _ => None,
        }
    }
}

/// Texte avec ses entités remplacées.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let Some(semi) = after.find(';').filter(|&s| s <= 10) else {
            out.push('&');
            rest = after;
            continue;
        };
        let entity = &after[..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &after[semi + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn attributes(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq].trim().to_string();
        let after = rest[eq + 1..].trim_start();
        let Some(quote) = after.chars().next().filter(|q| *q == '"' || *q == '\'') else {
            break;
        };
        let Some(close) = after[1..].find(quote) else {
            break;
        };
        out.push((key, unescape(&after[1..1 + close])));
        rest = &after[close + 2..];
    }
    out
}

/// Tous les événements de `xml`.
pub fn events(xml: &str) -> Vec<Event> {
    let mut out = Vec::new();
    let mut rest = xml;
    while !rest.is_empty() {
        let Some(open) = rest.find('<') else {
            out.push(Event::Text(unescape(rest)));
            break;
        };
        if open > 0 {
            out.push(Event::Text(unescape(&rest[..open])));
        }
        rest = &rest[open..];
        let close_mark = if rest.starts_with("<!--") {
            "-->"
        } else if rest.starts_with("<![CDATA[") {
            "]]>"
        } else {
            ">"
        };
        let Some(end) = rest.find(close_mark) else {
            break;
        };
        let tag = &rest[1..end];
        if let Some(cdata) = tag.strip_prefix("![CDATA[") {
            out.push(Event::Text(cdata.to_string()));
        } else if let Some(name) = tag.strip_prefix('/') {
            out.push(Event::End(name.trim().to_string()));
        } else if !tag.starts_with('?') && !tag.starts_with('!') {
            let empty = tag.ends_with('/');
            let tag = tag.trim_end_matches('/');
            let (name, attrs) = tag
                .split_once(char::is_whitespace)
                .map_or((tag, ""), |(n, a)| (n, a));
            out.push(Event::Start {
                name: name.to_string(),
                attrs: attributes(attrs),
                empty,
            });
        }
        rest = &rest[end + close_mark.len()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_attributes_text_and_entities() {
        let xml = r#"<?xml version="1.0"?><!-- c --><w:p a="1" b='x &amp; y'><w:t xml:space="preserve">A &lt;b&gt; &#233;t&#xE9;</w:t><w:br/></w:p>"#;
        let events = events(xml);
        assert_eq!(events.len(), 6);
        assert_eq!(events[0].attr("b"), Some("x & y"));
        assert_eq!(events[2], Event::Text("A <b> été".into()));
        assert_eq!(
            events[4],
            Event::Start {
                name: "w:br".into(),
                attrs: vec![],
                empty: true
            }
        );
        assert_eq!(events[5], Event::End("w:p".into()));
        assert_eq!(unescape("& &bogus; &;"), "& &bogus; &;");
        // Document coupé : on s'arrête sans paniquer.
        assert_eq!(super::events("<a><b").len(), 1);
    }
}
