//! Checking a note's frontmatter against the schema's field descriptions.
//!
//! Nothing here knows the name of a single field. That is the property worth
//! protecting: renaming `asserted` should be a data edit and a migration, and a
//! `match` on field names in here would silently make it a code change too.

use lab_note::Note;
use lab_schema::{Field, Kind, Severity};
use serde_yaml_ng::Value;

use crate::{Diagnostic, describe};

/// Rule id for a field-level violation. These are not rows in the schema's rule
/// table — they come from the field descriptions themselves — but a diagnostic
/// without an id cannot be explained or suppressed.
const FIELD_RULE: &str = "field";

pub fn check(schema: &lab_schema::Schema, note: &Note, out: &mut Vec<Diagnostic>) {
    for field in &schema.field {
        match note.get(&field.name) {
            None | Some(Value::Null) => {
                if field.required {
                    report(out, note, format!("`{}` is required", field.name));
                }
            }
            Some(value) => check_value(field, value, note, out),
        }
    }

    let known: Vec<&str> = schema.field.iter().map(|f| f.name.as_str()).collect();
    for key in note.fields.keys() {
        let Some(name) = key.as_str() else {
            report(out, note, "a frontmatter key is not a string".to_string());
            continue;
        };
        // `name` and `visibility` get a rule of their own, with an explanation.
        if !known.contains(&name) && !crate::DERIVABLE.contains(&name) {
            report(
                out,
                note,
                format!("`{name}` is not in the schema; add it there or remove it"),
            );
        }
    }
}

fn check_value(field: &Field, value: &Value, note: &Note, out: &mut Vec<Diagnostic>) {
    match field.kind {
        Kind::Text => {
            if value.as_str().is_none() {
                report(
                    out,
                    note,
                    format!("`{}` must be text, but is {}", field.name, describe(value)),
                );
            }
        }
        Kind::Enum => match value.as_str() {
            Some(actual) if field.values.iter().any(|v| v == actual) => {}
            Some(actual) => report(
                out,
                note,
                format!(
                    "`{}` is `{actual}`, which is not one of {}",
                    field.name,
                    field.values.join(", ")
                ),
            ),
            None => report(
                out,
                note,
                format!("`{}` must be text, but is {}", field.name, describe(value)),
            ),
        },
        Kind::Pattern => match value.as_str() {
            Some(actual) if matches_pattern(field, actual) => {}
            Some(actual) => report(
                out,
                note,
                format!(
                    "`{}` is `{actual}`, which does not match `{}`",
                    field.name,
                    field.pattern.as_deref().unwrap_or_default()
                ),
            ),
            None => report(
                out,
                note,
                format!("`{}` must be text, but is {}", field.name, describe(value)),
            ),
        },
        Kind::Date => match value.as_str() {
            // YAML parses an unquoted ISO date into its own date type, which is
            // not a string. Both spellings are fine; a number is not.
            Some(actual) if actual.parse::<jiff::civil::Date>().is_ok() => {}
            Some(actual) => report(
                out,
                note,
                format!("`{}` is `{actual}`, which is not an ISO date", field.name),
            ),
            None => report(
                out,
                note,
                format!(
                    "`{}` must be an ISO date, but is {}",
                    field.name,
                    describe(value)
                ),
            ),
        },
        Kind::List => match value.as_sequence() {
            Some(items) => {
                let min = field.min.unwrap_or(0);
                if items.len() < min {
                    report(
                        out,
                        note,
                        format!(
                            "`{}` has {} entries; the schema asks for at least {min}",
                            field.name,
                            items.len()
                        ),
                    );
                }
                if let Some(bad) = items.iter().find(|i| i.as_str().is_none()) {
                    report(
                        out,
                        note,
                        format!(
                            "`{}` contains {}, but every entry must be text",
                            field.name,
                            describe(bad)
                        ),
                    );
                }
            }
            None => report(
                out,
                note,
                format!(
                    "`{}` must be a list, but is {}",
                    field.name,
                    describe(value)
                ),
            ),
        },
    }
}

/// The pattern language is deliberately tiny: alternatives separated by `|`,
/// where `NAME` stands for a non-empty segment. Anything richer would be the
/// expression language P1 is specified not to have.
fn matches_pattern(field: &Field, actual: &str) -> bool {
    let Some(pattern) = field.pattern.as_deref() else {
        return true;
    };
    pattern.split('|').any(|alt| {
        let alt = alt.trim();
        match alt.split_once(":NAME") {
            Some((prefix, "")) => actual
                .strip_prefix(prefix)
                .and_then(|rest| rest.strip_prefix(':'))
                .is_some_and(|name| !name.is_empty()),
            _ => alt == actual,
        }
    })
}

fn report(out: &mut Vec<Diagnostic>, note: &Note, message: String) {
    out.push(Diagnostic {
        path: note.path.clone(),
        line: None,
        rule: FIELD_RULE.to_string(),
        severity: Severity::Error,
        message,
    });
}
