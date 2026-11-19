//! Scripts standard d'Acrobat dans les actions d'un champ (`/AA` : `/F`
//! mise en forme, `/K` frappe, `/V` validation, `/C` calcul) : écrits pour
//! les autres lecteurs, reconnus à la lecture. Aucun script n'est exécuté :
//! un appel inconnu ou mal formé est simplement ignoré.

use super::calc::{CalcOp, FieldCalc};
use super::{FieldFormat, Separator};

/// Scripts des actions d'un champ.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FieldActions {
    pub format: Option<String>,
    pub keystroke: Option<String>,
    pub validate: Option<String>,
    pub calculate: Option<String>,
}

/// Script de validation d'une adresse e-mail (repère reconnu à la lecture).
const EMAIL_CHECK: &str = "if (event.value && !/^[^\\s@]+@[^\\s@]+\\.[^\\s@]{2,}$/.test(event.value)) { app.alert(\"Adresse e-mail invalide.\"); event.rc = false; }";

/// Chaîne JavaScript entre guillemets (les formats sont déjà vérifiés ;
/// l'échappement protège quand même).
fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Scripts qui décrivent `format` et `calc`.
pub fn write_actions(format: Option<&FieldFormat>, calc: Option<&FieldCalc>) -> FieldActions {
    let mut actions = FieldActions::default();
    match format {
        Some(FieldFormat::Date { pattern }) => {
            actions.format = Some(format!("AFDate_FormatEx({});", quoted(pattern)));
            actions.keystroke = Some(format!("AFDate_KeystrokeEx({});", quoted(pattern)));
        }
        Some(FieldFormat::Time { pattern }) => {
            actions.format = Some(format!("AFTime_FormatEx({});", quoted(pattern)));
            actions.keystroke = Some(format!("AFTime_KeystrokeEx({});", quoted(pattern)));
        }
        Some(FieldFormat::Number {
            decimals,
            separator,
            currency,
            currency_before,
        }) => {
            let args = format!(
                "{decimals}, {}, 0, 0, {}, {currency_before}",
                separator.code(),
                quoted(currency)
            );
            actions.format = Some(format!("AFNumber_Format({args});"));
            actions.keystroke = Some(format!("AFNumber_Keystroke({args});"));
        }
        Some(FieldFormat::Percent {
            decimals,
            separator,
        }) => {
            let args = format!("{decimals}, {}", separator.code());
            actions.format = Some(format!("AFPercent_Format({args});"));
            actions.keystroke = Some(format!("AFPercent_Keystroke({args});"));
        }
        Some(FieldFormat::Mask { mask }) => {
            actions.keystroke = Some(format!("AFSpecial_KeystrokeEx({});", quoted(mask)));
        }
        Some(FieldFormat::Email) => actions.validate = Some(EMAIL_CHECK.into()),
        None => {}
    }
    if let Some(calc) = calc {
        let names: Vec<String> = calc.fields.iter().map(|f| quoted(f)).collect();
        actions.calculate = Some(format!(
            "AFSimple_Calculate({}, new Array({}));",
            quoted(calc.op.js_name()),
            names.join(", ")
        ));
    }
    actions
}

/// Format et calcul reconnus dans les scripts d'un champ.
pub fn read_actions(actions: &FieldActions) -> (Option<FieldFormat>, Option<FieldCalc>) {
    let scripts = [&actions.format, &actions.keystroke, &actions.validate];
    let format = scripts
        .iter()
        .filter_map(|s| s.as_deref())
        .find_map(read_format)
        .filter(|f| f.validate().is_ok());
    let calc = actions.calculate.as_deref().and_then(read_calc);
    (format, calc)
}

fn read_format(script: &str) -> Option<FieldFormat> {
    if script.contains("@]+\\.") || script.contains("@]+\\\\.") {
        return Some(FieldFormat::Email);
    }
    if let Some(args) =
        call(script, "AFDate_FormatEx").or_else(|| call(script, "AFDate_KeystrokeEx"))
    {
        return Some(FieldFormat::Date {
            pattern: args.first()?.text()?.to_string(),
        });
    }
    if let Some(args) = call(script, "AFDate_Format").or_else(|| call(script, "AFDate_Keystroke")) {
        return Some(FieldFormat::Date {
            pattern: legacy_date(args.first()?.number()?)?.into(),
        });
    }
    if let Some(args) =
        call(script, "AFTime_FormatEx").or_else(|| call(script, "AFTime_KeystrokeEx"))
    {
        return Some(FieldFormat::Time {
            pattern: args.first()?.text()?.to_string(),
        });
    }
    if let Some(args) = call(script, "AFTime_Format").or_else(|| call(script, "AFTime_Keystroke")) {
        let pattern = ["HH:MM", "h:MM tt", "HH:MM:ss", "h:MM:ss tt"]
            .get(usize::try_from(args.first()?.number()? as i64).ok()?)?;
        return Some(FieldFormat::Time {
            pattern: (*pattern).into(),
        });
    }
    if let Some(args) =
        call(script, "AFNumber_Format").or_else(|| call(script, "AFNumber_Keystroke"))
    {
        return Some(FieldFormat::Number {
            decimals: decimals(args.first())?,
            separator: separator(args.get(1)),
            currency: args.get(4).and_then(Arg::text).unwrap_or_default().into(),
            currency_before: args.get(5).and_then(Arg::boolean).unwrap_or(false),
        });
    }
    if let Some(args) =
        call(script, "AFPercent_Format").or_else(|| call(script, "AFPercent_Keystroke"))
    {
        return Some(FieldFormat::Percent {
            decimals: decimals(args.first())?,
            separator: separator(args.get(1)),
        });
    }
    if let Some(args) =
        call(script, "AFSpecial_KeystrokeEx").or_else(|| call(script, "AFSpecial_FormatEx"))
    {
        return Some(FieldFormat::Mask {
            mask: args.first()?.text()?.to_string(),
        });
    }
    if let Some(args) =
        call(script, "AFSpecial_Format").or_else(|| call(script, "AFSpecial_Keystroke"))
    {
        let mask = ["99999", "99999-9999", "(999) 999-9999", "999-99-9999"]
            .get(usize::try_from(args.first()?.number()? as i64).ok()?)?;
        return Some(FieldFormat::Mask {
            mask: (*mask).into(),
        });
    }
    None
}

fn decimals(arg: Option<&Arg>) -> Option<u8> {
    u8::try_from(arg?.number()? as i64).ok()
}

fn separator(arg: Option<&Arg>) -> Separator {
    arg.and_then(Arg::number)
        .and_then(|n| Separator::from_code(n as i64))
        .unwrap_or(Separator::CommaDot)
}

/// Formats numérotés de `AFDate_Format` (anciens formulaires).
fn legacy_date(index: f64) -> Option<&'static str> {
    const PATTERNS: [&str; 14] = [
        "m/d",
        "m/d/yy",
        "mm/dd/yy",
        "mm/yy",
        "d-mmm",
        "d-mmm-yy",
        "dd-mmm-yy",
        "yy-mm-dd",
        "mmm-yy",
        "mmmm-yy",
        "mmm d, yyyy",
        "mmmm d, yyyy",
        "m/d/yy h:MM tt",
        "m/d/yy HH:MM",
    ];
    PATTERNS.get(usize::try_from(index as i64).ok()?).copied()
}

fn read_calc(script: &str) -> Option<FieldCalc> {
    let args = call(script, "AFSimple_Calculate")?;
    let op = CalcOp::from_js_name(args.first()?.text()?)?;
    let fields = match args.get(1)? {
        Arg::List(names) => names.clone(),
        // Liste dans une chaîne : « a, b, c ».
        Arg::Text(names) => names
            .split(',')
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect(),
        _ => return None,
    };
    let calc = FieldCalc { op, fields };
    calc.validate("").is_ok().then_some(calc)
}

/// Argument d'un appel.
#[derive(Debug, Clone, PartialEq)]
enum Arg {
    Text(String),
    Number(f64),
    Bool(bool),
    List(Vec<String>),
}

impl Arg {
    fn text(&self) -> Option<&str> {
        match self {
            Arg::Text(t) => Some(t),
            _ => None,
        }
    }

    fn number(&self) -> Option<f64> {
        match self {
            Arg::Number(n) => Some(*n),
            _ => None,
        }
    }

    fn boolean(&self) -> Option<bool> {
        match self {
            Arg::Bool(b) => Some(*b),
            Arg::Number(n) => Some(*n != 0.0),
            _ => None,
        }
    }
}

/// Arguments du premier appel à `name` dans `script` ; `None` s'il n'y en
/// a pas ou s'ils ne sont pas tous des littéraux.
fn call(script: &str, name: &str) -> Option<Vec<Arg>> {
    let mut search = script;
    loop {
        let start = search.find(name)?;
        let before = search[..start].chars().next_back();
        let rest = &search[start + name.len()..];
        let is_word = before.is_some_and(|c| c.is_alphanumeric() || c == '_');
        let rest_trim = rest.trim_start();
        if !is_word && let Some(args) = rest_trim.strip_prefix('(') {
            let mut parser = Parser { rest: args };
            return parser.arguments(')');
        }
        search = rest;
    }
}

struct Parser<'a> {
    rest: &'a str,
}

impl Parser<'_> {
    fn skip_space(&mut self) {
        self.rest = self.rest.trim_start();
    }

    fn eat(&mut self, token: &str) -> bool {
        self.skip_space();
        match self.rest.strip_prefix(token) {
            Some(rest) => {
                self.rest = rest;
                true
            }
            None => false,
        }
    }

    fn arguments(&mut self, close: char) -> Option<Vec<Arg>> {
        let mut args = Vec::new();
        if self.eat(&close.to_string()) {
            return Some(args);
        }
        loop {
            args.push(self.argument()?);
            if self.eat(&close.to_string()) {
                return Some(args);
            }
            if !self.eat(",") || args.len() > 1000 {
                return None;
            }
        }
    }

    fn argument(&mut self) -> Option<Arg> {
        self.skip_space();
        if self.eat("new") {
            if !self.eat("Array") || !self.eat("(") {
                return None;
            }
            return self.list(')');
        }
        if self.eat("[") {
            return self.list(']');
        }
        if self.eat("true") {
            return Some(Arg::Bool(true));
        }
        if self.eat("false") {
            return Some(Arg::Bool(false));
        }
        let first = self.rest.chars().next()?;
        if first == '"' || first == '\'' {
            return self.string(first).map(Arg::Text);
        }
        let end = self
            .rest
            .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+')))
            .unwrap_or(self.rest.len());
        let number = self.rest[..end].parse().ok()?;
        self.rest = &self.rest[end..];
        Some(Arg::Number(number))
    }

    fn list(&mut self, close: char) -> Option<Arg> {
        let items = self.arguments(close)?;
        items
            .into_iter()
            .map(|a| match a {
                Arg::Text(t) => Some(t),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .map(Arg::List)
    }

    fn string(&mut self, quote: char) -> Option<String> {
        let mut out = String::new();
        let mut chars = self.rest.char_indices().skip(1);
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => out.push(chars.next()?.1),
                c if c == quote => {
                    self.rest = &self.rest[i + c.len_utf8()..];
                    return Some(out);
                }
                c => out.push(c),
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(format: &str) -> Option<FieldFormat> {
        read_actions(&FieldActions {
            format: Some(format.into()),
            ..FieldActions::default()
        })
        .0
    }

    #[test]
    fn written_scripts_are_read_back() {
        let formats = [
            FieldFormat::Date {
                pattern: "dd/mm/yyyy".into(),
            },
            FieldFormat::Time {
                pattern: "HH:MM".into(),
            },
            FieldFormat::Number {
                decimals: 2,
                separator: Separator::Comma,
                currency: " €".into(),
                currency_before: false,
            },
            FieldFormat::Percent {
                decimals: 1,
                separator: Separator::DotComma,
            },
            FieldFormat::Mask {
                mask: "99 99 99 99 99".into(),
            },
            FieldFormat::Email,
        ];
        let calc = FieldCalc {
            op: CalcOp::Sum,
            fields: vec!["ligne.1".into(), "ligne d'achat".into()],
        };
        for format in formats {
            let actions = write_actions(Some(&format), Some(&calc));
            assert_eq!(
                read_actions(&actions),
                (Some(format.clone()), Some(calc.clone())),
                "{actions:?}"
            );
        }
        let date = write_actions(
            Some(&FieldFormat::Date {
                pattern: "dd/mm/yyyy".into(),
            }),
            None,
        );
        assert_eq!(
            date.format.as_deref(),
            Some("AFDate_FormatEx(\"dd/mm/yyyy\");")
        );
        assert_eq!(date.calculate, None);
    }

    #[test]
    fn scripts_of_other_tools_are_recognized() {
        assert_eq!(
            read("AFNumber_Format(2, 0, 0, 0, \"$\", true);"),
            Some(FieldFormat::Number {
                decimals: 2,
                separator: Separator::CommaDot,
                currency: "$".into(),
                currency_before: true,
            })
        );
        assert_eq!(
            read("AFDate_Format(1);"),
            Some(FieldFormat::Date {
                pattern: "m/d/yy".into()
            })
        );
        assert_eq!(
            read("AFSpecial_Format(2);"),
            Some(FieldFormat::Mask {
                mask: "(999) 999-9999".into()
            })
        );
        assert_eq!(
            read("  AFTime_Format(1) ; "),
            Some(FieldFormat::Time {
                pattern: "h:MM tt".into()
            })
        );
        let calc = read_actions(&FieldActions {
            calculate: Some("AFSimple_Calculate('AVG', 'a, b');".into()),
            ..FieldActions::default()
        })
        .1;
        assert_eq!(
            calc.map(|c| (c.op, c.fields.len())),
            Some((CalcOp::Average, 2))
        );
    }

    #[test]
    fn anything_else_is_ignored() {
        for script in [
            "event.value = util.printd('dd', new Date());",
            "MyAFDate_FormatEx(\"dd\");",
            "AFDate_FormatEx(x);",
            "AFDate_FormatEx(\"zz\");",
            "AFNumber_Format(300, 0);",
            "AFDate_FormatEx(\"dd",
            "",
        ] {
            assert_eq!(read(script), None, "{script}");
        }
    }
}
