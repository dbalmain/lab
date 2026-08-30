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

    for key in note.fields.keys() {
        let Some(name) = key.as_str() else {
            report(out, note, "a frontmatter key is not a string".to_string());
            continue;
        };
        // `name` and `visibility` get a rule of their own, with an explanation.
        let in_schema = schema.field.iter().any(|f| f.name == name);
        if !in_schema && !crate::DERIVABLE.contains(&name) {
            report(
                out,
                note,
                format!("`{name}` is not in the schema; add it there or remove it"),
            );
        }
    }
}

fn check_value(field: &Field, value: &Value, note: &Note, out: &mut Vec<Diagnostic>) {
    check_as(field, field.kind, value, note, out);
}

/// Checks `value` against `kind`, which is the field's own kind for a scalar and
/// its element kind for each entry of a list. Splitting these apart is what lets
/// the schema describe a list of enums without the linter growing a second copy
/// of the enum check.
fn check_as(field: &Field, kind: Kind, value: &Value, note: &Note, out: &mut Vec<Diagnostic>) {
    match kind {
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
                for item in items {
                    check_as(field, field.element_kind(), item, note, out);
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use lab_note::Note;
    use lab_schema::Schema;

    use crate::Diagnostic;

    /// A schema with a list of enums — the case the shipped note schema does not
    /// exercise, and the one a hardcoded list-of-text check gets wrong.
    fn schema() -> Schema {
        Schema::parse(
            r#"
[schema]
name = "t"
title = "T"
version = 1

[[field]]
name = "labels"
required = true
kind = "list"
of = "enum"
min = 1
values = ["bug", "chore"]

[[field]]
name = "seen"
required = false
kind = "list"
of = "date"
"#,
        )
        .expect("test schema")
    }

    fn diagnose(front: &str) -> Vec<String> {
        let text = format!("---\n{front}---\n\nBody.\n");
        let note = Note::parse(Path::new("x/a.md"), &text)
            .ok()
            .expect("parses");
        let mut out: Vec<Diagnostic> = Vec::new();
        super::check(&schema(), &note, &mut out);
        out.into_iter().map(|d| d.message).collect()
    }

    #[test]
    fn a_list_of_valid_enums_passes() {
        assert!(diagnose("labels:\n  - bug\n  - chore\n").is_empty());
    }

    /// The discriminating case: a list of text and a list of enums are
    /// indistinguishable until an entry is outside the enum.
    #[test]
    fn an_entry_outside_the_enum_is_reported() {
        let found = diagnose("labels:\n  - bug\n  - nonsense\n");
        assert!(found.iter().any(|m| m.contains("`nonsense`")), "{found:?}");
    }

    #[test]
    fn an_entry_of_the_wrong_element_type_is_reported() {
        let found = diagnose("labels:\n  - bug\nseen:\n  - not-a-date\n");
        assert!(
            found.iter().any(|m| m.contains("not an ISO date")),
            "{found:?}"
        );
    }

    #[test]
    fn a_list_shorter_than_min_is_reported() {
        let found = diagnose("labels: []\n");
        assert!(found.iter().any(|m| m.contains("at least 1")), "{found:?}");
    }

    #[test]
    fn a_scalar_where_a_list_belongs_is_reported() {
        let found = diagnose("labels: bug\n");
        assert!(
            found.iter().any(|m| m.contains("must be a list")),
            "{found:?}"
        );
    }

    #[test]
    fn a_field_not_in_the_schema_is_reported() {
        let found = diagnose("labels:\n  - bug\nmystery: yes\n");
        assert!(found.iter().any(|m| m.contains("`mystery`")), "{found:?}");
    }
}
